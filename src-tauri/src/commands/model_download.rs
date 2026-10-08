//! One file, fetched so that a lost connection costs a pause, not the download.
//!
//! Model weights are hundreds of megabytes, fetched over whatever Wi-Fi the
//! person happens to be on. What this guarantees:
//!
//! - Bytes land in a staging file the loader never reads. The caller moves it
//!   into place only after this returns `Ok`, so a dropped connection can never
//!   leave a half model where the engine looks.
//! - A stream that goes quiet is treated as dropped (`idle_timeout`), so a dead
//!   socket cannot hold the download at 43% forever.
//! - A transient failure (no route, DNS, reset, timeout, 408/429/5xx) waits and
//!   tries again with capped exponential backoff. Any attempt that moved bytes
//!   resets the backoff, and the download only gives up after `give_up_after`
//!   with no new byte. Turning Wi-Fi off and on again inside that window simply
//!   resumes.
//! - A retry resumes with an HTTP `Range` request. It only resumes bytes it can
//!   prove belong to the same file: either the size is pinned by a manifest, or
//!   the server's validator (ETag / Last-Modified) was remembered and is sent
//!   back as `If-Range`. A server that ignores the range gets a clean restart,
//!   never a spliced file.
//!
//! The pure decisions (`plan_response`, `parse_content_range`, the backoff) are
//! separate from the I/O so they are tested directly; the I/O is tested against
//! a tiny local HTTP server that drops, stalls and refuses on cue.

use futures_util::StreamExt;
use reqwest::header::{HeaderMap, CONTENT_RANGE, ETAG, IF_RANGE, LAST_MODIFIED, RANGE};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;
use tracing::{info, warn};

/// How hard to try before telling the person the download did not finish.
#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    /// First pause after a failure.
    pub base_delay: Duration,
    /// Longest pause between attempts.
    pub max_delay: Duration,
    /// Give up after this long without a single new byte.
    pub give_up_after: Duration,
    /// A response body that delivers nothing for this long counts as dropped.
    pub idle_timeout: Duration,
}

impl RetryPolicy {
    /// Model downloads: a laptop closing its lid, a train tunnel, a router
    /// reboot all fit inside fifteen minutes.
    pub const MODELS: RetryPolicy = RetryPolicy {
        base_delay: Duration::from_secs(1),
        max_delay: Duration::from_secs(30),
        give_up_after: Duration::from_secs(15 * 60),
        idle_timeout: Duration::from_secs(45),
    };

    /// Pause before attempt `attempt + 1` (0-based): base, 2x, 4x... capped.
    pub fn backoff(&self, attempt: u32) -> Duration {
        let factor = 1u32.checked_shl(attempt.min(20)).unwrap_or(u32::MAX);
        self.base_delay.saturating_mul(factor).min(self.max_delay)
    }
}

/// Why a fetch ended without a complete file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchError {
    /// The person cancelled.
    Cancelled,
    /// No connection, or the server stayed unwell, for longer than the policy
    /// allows. What is on disk is kept so the next try resumes.
    Network(String),
    /// The disk is full.
    NoSpace,
    /// Trying again will not help (a 404, a file that changed upstream, a
    /// disk error). What is on disk is discarded.
    Permanent(String),
}

impl FetchError {
    /// The one line a person may read. Never an HTTP code, a URL or an error
    /// chain: those go to the log.
    pub fn human_message(&self) -> &'static str {
        match self {
            FetchError::Cancelled => "Cancelled",
            FetchError::Network(_) => "No internet connection. Try again when you're back online.",
            FetchError::NoSpace => "There isn't enough free space for this model.",
            FetchError::Permanent(_) => "The download didn't finish. Try again in a little while.",
        }
    }

    /// Whether the bytes on disk are worth keeping for a later resume.
    pub fn keeps_partial(&self) -> bool {
        matches!(self, FetchError::Network(_))
    }

    /// Detail for the log.
    pub fn detail(&self) -> String {
        match self {
            FetchError::Cancelled => "cancelled".to_string(),
            FetchError::Network(d) => format!("network: {}", d),
            FetchError::NoSpace => "no space left on device".to_string(),
            FetchError::Permanent(d) => d.clone(),
        }
    }
}

/// What the fetch tells its caller while it runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchEvent {
    /// Bytes of this file on disk so far, and the file's full size when known.
    Progress {
        file_bytes: u64,
        file_total: Option<u64>,
    },
    /// The connection failed; waiting before trying again.
    Waiting,
}

/// One file to fetch.
pub struct FetchTarget<'a> {
    pub url: &'a str,
    /// Staging path. Never a path a loader reads.
    pub path: &'a Path,
    /// Exact size when a manifest pins it.
    pub expected_bytes: Option<u64>,
    /// Where the server's validator is remembered between attempts.
    pub validator_path: &'a Path,
}

// ---------------------------------------------------------------------------
// Pure decisions
// ---------------------------------------------------------------------------

/// A parsed `Content-Range: bytes start-end/total` (total may be `*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentRange {
    pub start: u64,
    pub end: u64,
    pub total: Option<u64>,
}

pub fn parse_content_range(value: &str) -> Option<ContentRange> {
    let rest = value.trim().strip_prefix("bytes")?.trim_start();
    let (range, total) = rest.split_once('/')?;
    let (start, end) = range.split_once('-')?;
    let start = start.trim().parse().ok()?;
    let end = end.trim().parse().ok()?;
    if end < start {
        return None;
    }
    let total = match total.trim() {
        "*" => None,
        t => Some(t.parse().ok()?),
    };
    Some(ContentRange { start, end, total })
}

/// The total from a 416's `Content-Range: bytes */total`.
pub fn parse_unsatisfied_total(value: &str) -> Option<u64> {
    value
        .trim()
        .strip_prefix("bytes")?
        .trim_start()
        .strip_prefix("*/")?
        .trim()
        .parse()
        .ok()
}

/// What to do with a response, given what is already on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponsePlan {
    /// Append the body to the bytes on disk. `total` is the whole file's size.
    Append { total: Option<u64> },
    /// The body is the whole file: start over from byte 0.
    Restart { total: Option<u64> },
    /// The file on disk is already complete.
    Complete,
    /// Throw away what is on disk and ask again.
    Discard,
    /// Worth asking again after a pause.
    Retry(String),
    /// Asking again will not help.
    Fail(String),
}

pub fn plan_response(
    status: u16,
    existing: u64,
    content_range: Option<&str>,
    content_length: Option<u64>,
    expected: Option<u64>,
) -> ResponsePlan {
    let changed_upstream = |offered: u64| {
        ResponsePlan::Fail(format!(
            "file changed upstream (expected {} bytes, server offers {})",
            expected.unwrap_or(0),
            offered
        ))
    };
    match status {
        206 => match content_range.and_then(parse_content_range) {
            Some(cr) if cr.start == existing => {
                if let (Some(exp), Some(total)) = (expected, cr.total) {
                    if exp != total {
                        return changed_upstream(total);
                    }
                }
                ResponsePlan::Append {
                    total: cr.total.or(expected),
                }
            }
            // A range we did not ask for: start clean rather than splice.
            _ => ResponsePlan::Discard,
        },
        200..=299 => {
            let length = content_length.filter(|l| *l > 0);
            if let (Some(exp), Some(len)) = (expected, length) {
                if exp != len {
                    return changed_upstream(len);
                }
            }
            ResponsePlan::Restart {
                total: length.or(expected),
            }
        }
        416 => {
            let total = content_range.and_then(parse_unsatisfied_total).or(expected);
            if existing > 0 && total == Some(existing) {
                ResponsePlan::Complete
            } else {
                ResponsePlan::Discard
            }
        }
        408 | 425 | 429 | 500..=599 => ResponsePlan::Retry(format!("server answered {}", status)),
        _ => ResponsePlan::Fail(format!("server answered {}", status)),
    }
}

/// The validator to send back as `If-Range`: a strong ETag, else Last-Modified.
/// Weak ETags are not allowed in `If-Range`.
pub fn validator_from(headers: &HeaderMap) -> Option<String> {
    let header = |name: reqwest::header::HeaderName| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    header(ETAG)
        .filter(|etag| !etag.starts_with("W/"))
        .or_else(|| header(LAST_MODIFIED))
}

// ---------------------------------------------------------------------------
// I/O
// ---------------------------------------------------------------------------

enum Attempt {
    /// Try again after a pause. `made_progress` resets the backoff.
    Transient { reason: String, made_progress: bool },
    /// Stop.
    Fatal(FetchError),
}

fn io_fatal(e: std::io::Error) -> Attempt {
    if e.kind() == std::io::ErrorKind::StorageFull || e.raw_os_error() == Some(28) {
        Attempt::Fatal(FetchError::NoSpace)
    } else {
        Attempt::Fatal(FetchError::Permanent(format!("disk error: {}", e)))
    }
}

async fn file_len(path: &Path) -> u64 {
    tokio::fs::metadata(path)
        .await
        .map(|m| m.len())
        .unwrap_or(0)
}

async fn discard(path: &Path) {
    if let Err(e) = tokio::fs::remove_file(path).await {
        if e.kind() != std::io::ErrorKind::NotFound {
            warn!(
                "[ModelDownload] Could not discard {}: {}",
                path.display(),
                e
            );
        }
    }
}

async fn read_validator(path: &Path) -> Option<String> {
    tokio::fs::read_to_string(path)
        .await
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

async fn write_validator(path: &Path, validator: Option<&str>) {
    match validator {
        Some(v) => {
            if let Some(parent) = path.parent() {
                let _ = tokio::fs::create_dir_all(parent).await;
            }
            if let Err(e) = tokio::fs::write(path, v).await {
                // Only costs a resume: the next attempt restarts instead.
                warn!("[ModelDownload] Could not remember validator: {}", e);
            }
        }
        None => discard(path).await,
    }
}

/// Sleep in short slices so a cancel lands within a fraction of a second.
async fn pause(delay: Duration, cancel: &AtomicBool) -> Result<(), FetchError> {
    let deadline = Instant::now() + delay;
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err(FetchError::Cancelled);
        }
        let now = Instant::now();
        if now >= deadline {
            return Ok(());
        }
        tokio::time::sleep((deadline - now).min(Duration::from_millis(200))).await;
    }
}

/// Fetch `target.url` into `target.path`, resuming and retrying as needed.
/// Returns the final size of the file on disk.
pub async fn fetch_resumable<F>(
    client: &reqwest::Client,
    target: &FetchTarget<'_>,
    policy: &RetryPolicy,
    cancel: &AtomicBool,
    on_event: &mut F,
) -> Result<u64, FetchError>
where
    F: FnMut(FetchEvent),
{
    let mut attempt: u32 = 0;
    let mut last_progress = Instant::now();
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err(FetchError::Cancelled);
        }
        match attempt_once(client, target, policy, cancel, on_event).await {
            Ok(size) => {
                discard(target.validator_path).await;
                return Ok(size);
            }
            Err(Attempt::Fatal(e)) => return Err(e),
            Err(Attempt::Transient {
                reason,
                made_progress,
            }) => {
                if made_progress {
                    attempt = 0;
                    last_progress = Instant::now();
                }
                if last_progress.elapsed() >= policy.give_up_after {
                    return Err(FetchError::Network(reason));
                }
                let delay = policy.backoff(attempt);
                attempt = attempt.saturating_add(1);
                info!(
                    "[ModelDownload] {} ({}); trying again in {:?}",
                    reason, target.url, delay
                );
                on_event(FetchEvent::Waiting);
                pause(delay, cancel).await?;
            }
        }
    }
}

async fn attempt_once<F>(
    client: &reqwest::Client,
    target: &FetchTarget<'_>,
    policy: &RetryPolicy,
    cancel: &AtomicBool,
    on_event: &mut F,
) -> Result<u64, Attempt>
where
    F: FnMut(FetchEvent),
{
    let mut existing = file_len(target.path).await;
    if let Some(exp) = target.expected_bytes {
        if existing == exp {
            return Ok(existing);
        }
        if existing > exp {
            discard(target.path).await;
            existing = 0;
        }
    }
    let validator = read_validator(target.validator_path).await;
    // Resume only bytes provably from the same file.
    if existing > 0 && target.expected_bytes.is_none() && validator.is_none() {
        discard(target.path).await;
        existing = 0;
    }

    let mut request = client.get(target.url);
    if existing > 0 {
        request = request.header(RANGE, format!("bytes={}-", existing));
        if let Some(v) = &validator {
            request = request.header(IF_RANGE, v.as_str());
        }
    }
    let response = request.send().await.map_err(|e| {
        if e.is_builder() {
            Attempt::Fatal(FetchError::Permanent(format!("bad request: {}", e)))
        } else {
            Attempt::Transient {
                reason: format!("request failed: {}", e),
                made_progress: false,
            }
        }
    })?;

    let status = response.status().as_u16();
    let content_range = response
        .headers()
        .get(CONTENT_RANGE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let new_validator = validator_from(response.headers());
    let plan = plan_response(
        status,
        existing,
        content_range.as_deref(),
        response.content_length(),
        target.expected_bytes,
    );

    let (append, total) = match plan {
        ResponsePlan::Append { total } => (true, total),
        ResponsePlan::Restart { total } => (false, total),
        ResponsePlan::Complete => return Ok(existing),
        ResponsePlan::Discard => {
            discard(target.path).await;
            discard(target.validator_path).await;
            return Err(Attempt::Transient {
                reason: format!("server could not resume (status {})", status),
                made_progress: false,
            });
        }
        ResponsePlan::Retry(reason) => {
            return Err(Attempt::Transient {
                reason,
                made_progress: false,
            })
        }
        ResponsePlan::Fail(reason) => return Err(Attempt::Fatal(FetchError::Permanent(reason))),
    };

    if !append || validator.is_none() {
        write_validator(target.validator_path, new_validator.as_deref()).await;
    }

    let mut out = if append {
        tokio::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(target.path)
            .await
            .map_err(io_fatal)?
    } else {
        tokio::fs::File::create(target.path)
            .await
            .map_err(io_fatal)?
    };

    let start = if append { existing } else { 0 };
    let mut written = start;
    on_event(FetchEvent::Progress {
        file_bytes: written,
        file_total: total,
    });

    let mut stream = response.bytes_stream();
    let ended_early = loop {
        if cancel.load(Ordering::SeqCst) {
            let _ = out.flush().await;
            return Err(Attempt::Fatal(FetchError::Cancelled));
        }
        let next = match tokio::time::timeout(policy.idle_timeout, stream.next()).await {
            Ok(next) => next,
            Err(_) => break Some(format!("no data for {:?}", policy.idle_timeout)),
        };
        match next {
            None => break None,
            Some(Err(e)) => break Some(format!("connection dropped: {}", e)),
            Some(Ok(chunk)) => {
                out.write_all(&chunk).await.map_err(io_fatal)?;
                written += chunk.len() as u64;
                if total.is_some_and(|t| written > t) {
                    drop(out);
                    discard(target.path).await;
                    discard(target.validator_path).await;
                    return Err(Attempt::Fatal(FetchError::Permanent(
                        "server sent more than it announced".to_string(),
                    )));
                }
                on_event(FetchEvent::Progress {
                    file_bytes: written,
                    file_total: total,
                });
            }
        }
    };

    // Whatever arrived stays on disk for the next attempt to resume from.
    out.flush().await.map_err(io_fatal)?;
    let made_progress = written > start;
    if let Some(reason) = ended_early {
        return Err(Attempt::Transient {
            reason,
            made_progress,
        });
    }
    if let Some(t) = total {
        if written < t {
            return Err(Attempt::Transient {
                reason: format!("connection closed at {} of {} bytes", written, t),
                made_progress,
            });
        }
    }
    out.sync_all().await.map_err(io_fatal)?;
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tokio::io::AsyncReadExt;
    use tokio::net::TcpListener;

    // -- pure ---------------------------------------------------------------

    #[test]
    fn content_range_parses_the_shapes_servers_send() {
        assert_eq!(
            parse_content_range("bytes 100-199/1000"),
            Some(ContentRange {
                start: 100,
                end: 199,
                total: Some(1000)
            })
        );
        assert_eq!(
            parse_content_range("bytes 0-9/*"),
            Some(ContentRange {
                start: 0,
                end: 9,
                total: None
            })
        );
        assert_eq!(parse_content_range("bytes 9-0/10"), None);
        assert_eq!(parse_content_range("items 0-9/10"), None);
        assert_eq!(parse_content_range("garbage"), None);
        assert_eq!(parse_unsatisfied_total("bytes */1000"), Some(1000));
        assert_eq!(parse_unsatisfied_total("bytes 0-1/1000"), None);
    }

    #[test]
    fn a_matching_partial_response_appends() {
        assert_eq!(
            plan_response(206, 100, Some("bytes 100-999/1000"), Some(900), None),
            ResponsePlan::Append { total: Some(1000) }
        );
    }

    #[test]
    fn a_partial_response_at_the_wrong_offset_is_never_spliced() {
        assert_eq!(
            plan_response(206, 100, Some("bytes 0-999/1000"), Some(1000), None),
            ResponsePlan::Discard
        );
        assert_eq!(
            plan_response(206, 100, None, Some(900), None),
            ResponsePlan::Discard
        );
    }

    #[test]
    fn a_server_that_ignores_the_range_restarts_from_zero() {
        assert_eq!(
            plan_response(200, 100, None, Some(1000), None),
            ResponsePlan::Restart { total: Some(1000) }
        );
    }

    #[test]
    fn a_pinned_size_that_disagrees_with_the_server_fails() {
        assert!(matches!(
            plan_response(200, 0, None, Some(999), Some(1000)),
            ResponsePlan::Fail(_)
        ));
        assert!(matches!(
            plan_response(206, 10, Some("bytes 10-998/999"), None, Some(1000)),
            ResponsePlan::Fail(_)
        ));
    }

    #[test]
    fn range_not_satisfiable_means_done_only_when_the_sizes_agree() {
        assert_eq!(
            plan_response(416, 1000, Some("bytes */1000"), None, None),
            ResponsePlan::Complete
        );
        assert_eq!(
            plan_response(416, 500, Some("bytes */1000"), None, None),
            ResponsePlan::Discard
        );
    }

    #[test]
    fn server_trouble_is_retried_and_missing_files_are_not() {
        for status in [408, 429, 500, 502, 503, 504] {
            assert!(
                matches!(
                    plan_response(status, 0, None, None, None),
                    ResponsePlan::Retry(_)
                ),
                "{status}"
            );
        }
        for status in [401, 403, 404, 410] {
            assert!(
                matches!(
                    plan_response(status, 0, None, None, None),
                    ResponsePlan::Fail(_)
                ),
                "{status}"
            );
        }
    }

    #[test]
    fn backoff_doubles_and_caps() {
        let p = RetryPolicy::MODELS;
        assert_eq!(p.backoff(0), Duration::from_secs(1));
        assert_eq!(p.backoff(1), Duration::from_secs(2));
        assert_eq!(p.backoff(3), Duration::from_secs(8));
        assert_eq!(p.backoff(10), Duration::from_secs(30));
        assert_eq!(p.backoff(u32::MAX), Duration::from_secs(30));
    }

    #[test]
    fn weak_etags_are_not_used_as_validators() {
        let mut h = HeaderMap::new();
        h.insert(ETAG, "W/\"abc\"".parse().unwrap());
        assert_eq!(validator_from(&h), None);
        h.insert(
            LAST_MODIFIED,
            "Wed, 01 Oct 2026 00:00:00 GMT".parse().unwrap(),
        );
        assert_eq!(
            validator_from(&h).as_deref(),
            Some("Wed, 01 Oct 2026 00:00:00 GMT")
        );
        h.insert(ETAG, "\"strong\"".parse().unwrap());
        assert_eq!(validator_from(&h).as_deref(), Some("\"strong\""));
    }

    #[test]
    fn nothing_a_person_reads_carries_a_code_or_an_error_chain() {
        for e in [
            FetchError::Network("request failed: error sending request for url".into()),
            FetchError::NoSpace,
            FetchError::Permanent("server answered 404".into()),
        ] {
            let msg = e.human_message();
            for bad in ["404", "HTTP", "http", "error sending", "url", "reqwest"] {
                assert!(!msg.contains(bad), "{msg:?} contains {bad:?}");
            }
        }
        assert!(FetchError::Network(String::new()).keeps_partial());
        assert!(!FetchError::Permanent(String::new()).keeps_partial());
        assert!(!FetchError::Cancelled.keeps_partial());
    }

    // -- I/O against a scripted local server --------------------------------

    /// What the server does for one connection, in order.
    enum Step {
        /// Serve the body (honouring `Range`), but stop after `cut` body bytes
        /// and close the socket.
        Drop { cut: usize },
        /// Serve the head and `cut` body bytes, then go silent.
        Stall { cut: usize },
        /// Serve properly, honouring `Range`.
        Serve,
        /// Serve the whole body with 200, ignoring `Range`.
        IgnoreRange,
        /// Answer with this status and no body.
        Status(u16),
    }

    struct Server {
        url: String,
        ranges: Arc<Mutex<Vec<Option<String>>>>,
    }

    fn body(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i % 251) as u8).collect()
    }

    async fn serve(script: Vec<Step>, data: Vec<u8>) -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let ranges = Arc::new(Mutex::new(Vec::new()));
        let seen = ranges.clone();
        let data = Arc::new(data);
        tokio::spawn(async move {
            let mut script = script.into_iter();
            loop {
                let Ok((sock, _)) = listener.accept().await else {
                    return;
                };
                let step = script.next().unwrap_or(Step::Serve);
                // One task per connection: a stalled one must not block the next.
                tokio::spawn(handle(sock, step, data.clone(), seen.clone()));
            }
        });
        Server {
            url: format!("http://{}/model.bin", addr),
            ranges,
        }
    }

    async fn handle(
        mut sock: tokio::net::TcpStream,
        step: Step,
        data: Arc<Vec<u8>>,
        seen: Arc<Mutex<Vec<Option<String>>>>,
    ) {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 1024];
        while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
            match sock.read(&mut chunk).await {
                Ok(0) | Err(_) => break,
                Ok(n) => buf.extend_from_slice(&chunk[..n]),
            }
        }
        let head = String::from_utf8_lossy(&buf).to_ascii_lowercase();
        let range_line = head
            .lines()
            .find(|l| l.starts_with("range:"))
            .map(|l| l.trim().to_string());
        let range_start = range_line.as_deref().and_then(|l| {
            l.strip_prefix("range: bytes=")
                .and_then(|r| r.trim_end_matches('-').parse::<usize>().ok())
        });
        seen.lock().unwrap().push(range_line);

        let total = data.len();
        let (status_line, start, extra) = match (&step, range_start) {
            (Step::Status(code), _) => {
                let resp = format!(
                    "HTTP/1.1 {} X\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    code
                );
                let _ = sock.write_all(resp.as_bytes()).await;
                return;
            }
            (Step::IgnoreRange, _) | (_, None) => ("200 OK", 0, String::new()),
            (_, Some(s)) => (
                "206 Partial Content",
                s,
                format!("Content-Range: bytes {}-{}/{}\r\n", s, total - 1, total),
            ),
        };
        let head = format!(
            "HTTP/1.1 {}\r\nContent-Length: {}\r\nETag: \"v1\"\r\n{}Connection: close\r\n\r\n",
            status_line,
            total - start,
            extra
        );
        let _ = sock.write_all(head.as_bytes()).await;
        match step {
            Step::Drop { cut } => {
                let _ = sock.write_all(&data[start..start + cut]).await;
            }
            Step::Stall { cut } => {
                let _ = sock.write_all(&data[start..start + cut]).await;
                let _ = sock.flush().await;
                tokio::time::sleep(Duration::from_secs(30)).await;
            }
            _ => {
                let _ = sock.write_all(&data[start..]).await;
            }
        }
    }

    fn fast_policy() -> RetryPolicy {
        RetryPolicy {
            base_delay: Duration::from_millis(10),
            max_delay: Duration::from_millis(50),
            give_up_after: Duration::from_secs(5),
            idle_timeout: Duration::from_millis(300),
        }
    }

    struct Run {
        result: Result<u64, FetchError>,
        on_disk: Vec<u8>,
        waits: usize,
    }

    async fn run(
        url: &str,
        expected: Option<u64>,
        policy: RetryPolicy,
        cancel: &AtomicBool,
    ) -> (tempfile_dir::Dir, Run) {
        let dir = tempfile_dir::Dir::new();
        let path = dir.path().join("model.bin");
        let validator_path = dir.path().join("model.bin.validator");
        let client = reqwest::Client::new();
        let target = FetchTarget {
            url,
            path: &path,
            expected_bytes: expected,
            validator_path: &validator_path,
        };
        let mut waits = 0;
        let mut on_event = |e: FetchEvent| {
            if e == FetchEvent::Waiting {
                waits += 1;
            }
        };
        let result = fetch_resumable(&client, &target, &policy, cancel, &mut on_event).await;
        let on_disk = std::fs::read(&path).unwrap_or_default();
        (
            dir,
            Run {
                result,
                on_disk,
                waits,
            },
        )
    }

    /// A self-cleaning temp directory, so the tests need no new crate.
    mod tempfile_dir {
        use std::path::{Path, PathBuf};
        use std::sync::atomic::{AtomicUsize, Ordering};

        static N: AtomicUsize = AtomicUsize::new(0);

        pub struct Dir(PathBuf);

        impl Dir {
            pub fn new() -> Self {
                let p = std::env::temp_dir().join(format!(
                    "juno-model-download-{}-{}",
                    std::process::id(),
                    N.fetch_add(1, Ordering::SeqCst)
                ));
                let _ = std::fs::remove_dir_all(&p);
                std::fs::create_dir_all(&p).unwrap();
                Dir(p)
            }
            pub fn path(&self) -> &Path {
                &self.0
            }
        }

        impl Drop for Dir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }

    #[tokio::test]
    async fn a_dropped_connection_resumes_where_it_stopped() {
        let data = body(10_000);
        let server = serve(vec![Step::Drop { cut: 4_000 }, Step::Serve], data.clone()).await;
        let cancel = AtomicBool::new(false);
        let (_dir, run) = run(&server.url, None, fast_policy(), &cancel).await;
        assert_eq!(run.result, Ok(10_000));
        assert_eq!(
            run.on_disk, data,
            "bytes are exactly the file, nothing spliced"
        );
        assert_eq!(run.waits, 1);
        let ranges = server.ranges.lock().unwrap().clone();
        assert_eq!(ranges[0], None);
        assert_eq!(ranges[1].as_deref(), Some("range: bytes=4000-"));
    }

    #[tokio::test]
    async fn a_stalled_stream_times_out_and_resumes() {
        let data = body(8_000);
        let server = serve(vec![Step::Stall { cut: 3_000 }, Step::Serve], data.clone()).await;
        let cancel = AtomicBool::new(false);
        let (_dir, run) = run(&server.url, Some(8_000), fast_policy(), &cancel).await;
        assert_eq!(run.result, Ok(8_000));
        assert_eq!(run.on_disk, data);
        let ranges = server.ranges.lock().unwrap().clone();
        assert_eq!(ranges[1].as_deref(), Some("range: bytes=3000-"));
    }

    #[tokio::test]
    async fn server_errors_are_retried_with_backoff() {
        let data = body(2_000);
        let server = serve(
            vec![Step::Status(503), Step::Status(502), Step::Serve],
            data.clone(),
        )
        .await;
        let cancel = AtomicBool::new(false);
        let (_dir, run) = run(&server.url, None, fast_policy(), &cancel).await;
        assert_eq!(run.result, Ok(2_000));
        assert_eq!(run.on_disk, data);
        assert_eq!(run.waits, 2);
    }

    #[tokio::test]
    async fn a_server_that_ignores_range_gets_a_clean_restart() {
        let data = body(5_000);
        let server = serve(
            vec![Step::Drop { cut: 1_000 }, Step::IgnoreRange],
            data.clone(),
        )
        .await;
        let cancel = AtomicBool::new(false);
        let (_dir, run) = run(&server.url, None, fast_policy(), &cancel).await;
        assert_eq!(run.result, Ok(5_000));
        assert_eq!(run.on_disk, data, "the first 1000 bytes are not duplicated");
    }

    #[tokio::test]
    async fn a_missing_file_fails_at_once() {
        let server = serve(vec![Step::Status(404)], body(10)).await;
        let cancel = AtomicBool::new(false);
        let (_dir, run) = run(&server.url, None, fast_policy(), &cancel).await;
        assert!(matches!(run.result, Err(FetchError::Permanent(_))));
        assert_eq!(run.waits, 0);
    }

    #[tokio::test]
    async fn no_network_gives_up_after_the_window_and_keeps_the_partial() {
        // Bind and drop: nothing listens, every connect is refused.
        let addr = {
            let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
            l.local_addr().unwrap()
        };
        let url = format!("http://{}/model.bin", addr);
        let policy = RetryPolicy {
            give_up_after: Duration::from_millis(200),
            ..fast_policy()
        };
        let cancel = AtomicBool::new(false);
        let (_dir, run) = run(&url, Some(100), policy, &cancel).await;
        match run.result {
            Err(e) => {
                assert!(matches!(e, FetchError::Network(_)), "{e:?}");
                assert!(e.keeps_partial());
            }
            Ok(n) => panic!("unexpected success: {n}"),
        }
        assert!(run.waits >= 1);
    }

    #[tokio::test]
    async fn cancel_stops_a_waiting_download() {
        let addr = {
            let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
            l.local_addr().unwrap()
        };
        let url = format!("http://{}/model.bin", addr);
        let cancel = AtomicBool::new(true);
        let (_dir, run) = run(&url, None, fast_policy(), &cancel).await;
        assert_eq!(run.result, Err(FetchError::Cancelled));
    }

    #[tokio::test]
    async fn a_complete_pinned_file_on_disk_needs_no_request() {
        let dir = tempfile_dir::Dir::new();
        let path = dir.path().join("f");
        let vpath = dir.path().join("f.validator");
        std::fs::write(&path, body(64)).unwrap();
        let target = FetchTarget {
            url: "http://127.0.0.1:9/never",
            path: &path,
            expected_bytes: Some(64),
            validator_path: &vpath,
        };
        let cancel = AtomicBool::new(false);
        let result = fetch_resumable(
            &reqwest::Client::new(),
            &target,
            &fast_policy(),
            &cancel,
            &mut |_e: FetchEvent| {},
        )
        .await;
        assert_eq!(result, Ok(64));
    }
}
