//! # Live CDP Browser Integration Tests
//!
//! Exercises `BrowserController` against a **real Chrome** over the DevTools
//! Protocol. This is the runtime QA that pure `cargo check` cannot give us —
//! the chromiumoxide migration compiles fine but every CDP round-trip is
//! unverified until something actually drives it.
//!
//! These tests are `#[ignore]`d so `cargo test` stays hermetic. Run them with a
//! Chrome listening on the remote debugging port:
//!
//! ```bash
//! # Launch (or already have) Chrome with:
//! #   /Applications/Google\ Chrome.app/Contents/MacOS/Google\ Chrome --remote-debugging-port=9222
//! cargo test --manifest-path src-tauri/Cargo.toml \
//!     --test browser_cdp_live -- --ignored --test-threads=1 --nocapture
//! ```
//!
//! `--test-threads=1` matters: the tests share one browser, and Chrome does not
//! appreciate several suites racing to open and close tabs.
//!
//! The page under test is served by a throwaway HTTP server bound to an
//! ephemeral localhost port, so the suite needs no network access and the HTML
//! never drifts out from under an assertion.
//!
//! ## These tests drive a tab you own
//!
//! The CDP attach strategy adopts an already-open tab rather than opening its
//! own, which is the whole point of "drive the browser the user is already
//! using". That means this suite navigates a real tab away from whatever was in
//! it. Every test restores the original URL when it finishes, but a scratch
//! Chrome window is still the polite place to run them.

use juno_lib::agent::tools::browser_controller::BrowserController;
use serde_json::json;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Markup with one of everything the controller claims to support.
const FIXTURE_HTML: &str = r#"<!doctype html>
<html>
<head><title>Juno CDP Fixture</title></head>
<body style="margin:0">
  <h1 id="heading">Hello from the fixture</h1>
  <p class="item">alpha</p>
  <p class="item">beta</p>
  <p class="item">gamma</p>
  <a id="link" href="https://example.com/target">A link</a>
  <!-- `#mirror` exists because `extract_content` can only read HTML attributes,
       never live DOM properties, so there is no way to read an input's current
       value back directly. Mirroring on `input` also proves the controller
       dispatches the events that reactive frameworks bind to. -->
  <input id="text-field" type="text" value=""
         oninput="document.getElementById('mirror').textContent = this.value" />
  <button id="btn" onclick="document.getElementById('sink').textContent='clicked'">Press me</button>
  <div id="sink">unclicked</div>
  <div id="mirror"></div>
  <div id="tall" style="height:3000px"></div>
</body>
</html>"#;

/// Serve `FIXTURE_HTML` on an ephemeral port until the test process exits.
///
/// Deliberately minimal: it answers every request with the same page and never
/// parses the request line beyond draining it, which is all the controller
/// needs and keeps the fixture free of an HTTP-server dependency.
async fn serve_fixture() -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local addr");

    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(async move {
                // Drain whatever the browser sent; we serve one page regardless.
                let mut buf = [0u8; 2048];
                let _ = socket.read(&mut buf).await;

                let response = format!(
                    "HTTP/1.1 200 OK\r\n\
                     Content-Type: text/html; charset=utf-8\r\n\
                     Content-Length: {}\r\n\
                     Connection: close\r\n\r\n{}",
                    FIXTURE_HTML.len(),
                    FIXTURE_HTML
                );
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.shutdown().await;
            });
        }
    });

    format!("http://{}/", addr)
}

/// Connect to the Chrome the developer already has running.
///
/// `BrowserController::new()` walks its four strategies in order and will
/// happily *launch* a browser if CDP attach fails. That would still pass the
/// assertions below while silently testing a different code path, so we assert
/// on the connection method and bail loudly instead.
///
/// Returns the controller alongside the URL of the tab it adopted, so the test
/// can hand that tab back in the state it borrowed it.
async fn connect() -> (BrowserController, String) {
    // `run()` normally does this at startup; tests never call `run()`, and
    // without it chromiumoxide's reqwest client panics as it is built.
    juno_lib::install_crypto_provider();

    let controller = BrowserController::new()
        .await
        .expect("BrowserController::new() should connect to Chrome on :9222");

    let method = controller.get_connection_method().to_string();
    assert!(
        method.starts_with("CDP"),
        "expected an attach to the existing Chrome, got connection method {:?}. \
         Start Chrome with --remote-debugging-port=9222 before running this suite.",
        method
    );

    let original = controller
        .get_current_url(&json!({}))
        .await
        .ok()
        .and_then(|r| r.output["url"].as_str().map(str::to_owned))
        .unwrap_or_else(|| "about:blank".to_string());

    (controller, original)
}

/// Put the borrowed tab back where it was, then release the connection.
async fn restore(controller: BrowserController, original_url: &str) {
    if !original_url.is_empty() && original_url != "about:blank" {
        let _ = controller.navigate(&json!({ "url": original_url })).await;
    }
    controller.cleanup().await.expect("cleanup should succeed");
}

#[tokio::test]
#[ignore = "requires a Chrome listening on :9222"]
async fn cdp_attach_reports_existing_browser() {
    let (controller, original) = connect().await;
    println!("connected via: {}", controller.get_connection_method());
    restore(controller, &original).await;
}

/// Count open page targets straight from the CDP HTTP endpoint, independent of
/// anything the controller reports about itself.
///
/// Hand-rolled GET rather than reqwest: integration tests are their own crate
/// and cannot see the lib's `[dependencies]`, and this endpoint is plain HTTP on
/// loopback, so a dev-dependency would buy nothing.
async fn page_count() -> usize {
    // Chrome's DevTools HTTP server does not honour `Connection: close`, so
    // reading to EOF hangs forever. Read the headers, then exactly the number of
    // body bytes it advertised. The outer timeout means a protocol surprise
    // fails the test instead of wedging the whole suite.
    let raw = tokio::time::timeout(Duration::from_secs(10), async {
        let mut socket = tokio::net::TcpStream::connect("127.0.0.1:9222")
            .await
            .expect("connect to CDP port");
        socket
            .write_all(b"GET /json/list HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .await
            .expect("send CDP request");

        let mut buf = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            let n = socket.read(&mut chunk).await.expect("read CDP response");
            if n == 0 {
                break; // Server closed early; parse whatever arrived.
            }
            buf.extend_from_slice(&chunk[..n]);

            let text = String::from_utf8_lossy(&buf);
            let Some((headers, body)) = text.split_once("\r\n\r\n") else {
                continue; // Headers still incomplete.
            };
            let content_length = headers
                .lines()
                .find_map(|l| {
                    let (name, value) = l.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())?
                })
                .expect("CDP response should advertise Content-Length");

            if body.len() >= content_length {
                return body.to_string();
            }
        }
        String::from_utf8_lossy(&buf)
            .split_once("\r\n\r\n")
            .map(|(_, body)| body.to_string())
            .unwrap_or_default()
    })
    .await
    .expect("CDP target list should arrive within 10s");

    let targets: serde_json::Value = serde_json::from_str(&raw).expect("parse CDP target list");
    targets
        .as_array()
        .map(|ts| ts.iter().filter(|t| t["type"] == "page").count())
        .unwrap_or(0)
}

/// A full connect/navigate/cleanup cycle must leave the tab count where it
/// found it.
///
/// Both directions have bitten us. Closing indiscriminately took down tabs the
/// user owned (and `Browser.close` over an attach killed all of Chrome); never
/// closing leaked a tab per agent session. Cleanup closes only pages it opened.
#[tokio::test]
#[ignore = "requires a Chrome listening on :9222"]
async fn session_leaves_no_tabs_behind() {
    let url = serve_fixture().await;
    let before = page_count().await;

    {
        let (controller, original) = connect().await;
        controller
            .navigate(&json!({ "url": url }))
            .await
            .expect("navigate");
        restore(controller, &original).await;
    }

    // Chrome tears targets down asynchronously; give it a moment to settle.
    tokio::time::sleep(Duration::from_millis(750)).await;
    let after = page_count().await;

    assert_eq!(
        before, after,
        "tab count changed across a session ({} -> {}): cleanup either leaked a \
         tab it opened or closed one it merely borrowed",
        before, after
    );
}

#[tokio::test]
#[ignore = "requires a Chrome listening on :9222"]
async fn navigate_reports_title_and_url() {
    let url = serve_fixture().await;
    let (controller, original) = connect().await;

    let nav = controller
        .navigate(&json!({ "url": url }))
        .await
        .expect("navigation should succeed");

    assert_eq!(nav.output["status"], "success");
    assert_eq!(
        nav.output["title"], "Juno CDP Fixture",
        "page title should come back from the real document"
    );

    let current = controller
        .get_current_url(&json!({}))
        .await
        .expect("get_current_url should succeed");
    assert!(
        current.output["url"]
            .as_str()
            .unwrap_or_default()
            .starts_with("http://127.0.0.1:"),
        "current URL should be the fixture, got {:?}",
        current.output["url"]
    );

    restore(controller, &original).await;
}

#[tokio::test]
#[ignore = "requires a Chrome listening on :9222"]
async fn navigate_honours_its_timeout() {
    // Bind a port and never accept, so the load hangs and the timeout we layer
    // over chromiumoxide (which has no per-call timeout of its own) has to fire.
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let dead_url = format!("http://{}/", addr);

    let (controller, original) = connect().await;
    let started = std::time::Instant::now();
    let result = controller
        .navigate(&json!({ "url": dead_url, "timeout": 3000 }))
        .await;
    let elapsed = started.elapsed();

    assert!(result.is_err(), "navigation to a black hole should fail");
    assert!(
        elapsed < Duration::from_secs(20),
        "timeout should bound the call; took {:?}",
        elapsed
    );

    restore(controller, &original).await;
}

#[tokio::test]
#[ignore = "requires a Chrome listening on :9222"]
async fn extract_content_reads_text_attributes_and_lists() {
    let url = serve_fixture().await;
    let (controller, original) = connect().await;
    controller
        .navigate(&json!({ "url": url }))
        .await
        .expect("navigate");

    let single = controller
        .extract_content(&json!({ "selector": "#heading" }))
        .await
        .expect("extract single");
    assert_eq!(single.output["content"], "Hello from the fixture");

    let attr = controller
        .extract_content(&json!({ "selector": "#link", "attribute": "href" }))
        .await
        .expect("extract attribute");
    assert_eq!(attr.output["content"], "https://example.com/target");

    let many = controller
        .extract_content(&json!({ "selector": ".item", "multiple": true }))
        .await
        .expect("extract multiple");
    let items = many.output["content"]
        .as_array()
        .expect("multiple extraction should yield an array");
    assert_eq!(
        items.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>(),
        vec!["alpha", "beta", "gamma"]
    );

    // A selector that matches nothing must come back null, not error.
    let missing = controller
        .extract_content(&json!({ "selector": "#does-not-exist" }))
        .await
        .expect("missing selector should still return a result");
    assert!(missing.output["content"].is_null());

    restore(controller, &original).await;
}

#[tokio::test]
#[ignore = "requires a Chrome listening on :9222"]
async fn interact_types_and_clicks() {
    let url = serve_fixture().await;
    let (controller, original) = connect().await;
    controller
        .navigate(&json!({ "url": url }))
        .await
        .expect("navigate");

    controller
        .interact(&json!({
            "action": "type",
            "selector": "#text-field",
            "value": "juno was here"
        }))
        .await
        .expect("type should succeed");

    // Read the mirror, not `getAttribute("value")` — the latter returns the
    // markup's initial attribute forever, no matter what the property holds.
    let typed = controller
        .extract_content(&json!({ "selector": "#mirror" }))
        .await
        .expect("read back the mirrored value");
    assert_eq!(
        typed.output["content"], "juno was here",
        "typing should set the property and fire an `input` event"
    );

    controller
        .interact(&json!({ "action": "click", "selector": "#btn" }))
        .await
        .expect("click should succeed");

    let sink = controller
        .extract_content(&json!({ "selector": "#sink" }))
        .await
        .expect("read the click sink");
    assert_eq!(
        sink.output["content"], "clicked",
        "the click handler should have run in the page"
    );

    // Clicking a selector that matches nothing is an error, not a silent no-op.
    let missing = controller
        .interact(&json!({ "action": "click", "selector": "#nope" }))
        .await;
    assert!(missing.is_err(), "clicking a missing element should error");

    restore(controller, &original).await;
}

#[tokio::test]
#[ignore = "requires a Chrome listening on :9222"]
async fn screenshot_returns_decodable_png() {
    use base64::Engine;

    let url = serve_fixture().await;
    let (controller, original) = connect().await;
    controller
        .navigate(&json!({ "url": url }))
        .await
        .expect("navigate");

    let shot = controller
        .screenshot(&json!({ "full_page": false }))
        .await
        .expect("viewport screenshot");
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(shot.output["base64"].as_str().expect("base64 field"))
        .expect("screenshot should be valid base64");
    assert!(
        bytes.starts_with(b"\x89PNG"),
        "screenshot should decode to a PNG"
    );

    let element = controller
        .screenshot(&json!({ "selector": "#heading" }))
        .await
        .expect("element screenshot");
    let element_bytes = base64::engine::general_purpose::STANDARD
        .decode(element.output["base64"].as_str().expect("base64 field"))
        .expect("element screenshot should be valid base64");
    assert!(element_bytes.starts_with(b"\x89PNG"));
    assert!(
        element_bytes.len() < bytes.len(),
        "a clipped element ({}B) should be smaller than the viewport ({}B)",
        element_bytes.len(),
        bytes.len()
    );

    // Screenshotting something that isn't there is an error, not a blank image.
    let missing = controller
        .screenshot(&json!({ "selector": "#nope" }))
        .await;
    assert!(missing.is_err(), "missing element should error");

    restore(controller, &original).await;
}
