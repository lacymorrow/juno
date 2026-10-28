//! Juno's log on disk.
//!
//! Juno launched from the Dock or Finder has nowhere for stdout to go, so without this the log
//! (including the `[TurnTiming]` line) is lost. Every event is also appended to
//! `~/Library/Logs/Juno/juno-YYYY-MM-DD.log`: one file per local day, the last
//! [`KEEP_DAYS`] kept. Console.app lists that folder under Log Reports.
//!
//! If the folder cannot be created, Juno logs to stdout only, exactly as before.

use chrono::{Local, NaiveDate};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tracing_subscriber::fmt::MakeWriter;

/// Days of log files kept, today included.
const KEEP_DAYS: i64 = 7;
/// The most one day's file may hold. A week of files at this size is the worst
/// case on disk; a runaway logger (`RUST_LOG=debug`, a tight error loop) stops
/// here instead of filling the disk. Normal days are a few megabytes.
const MAX_DAY_BYTES: u64 = 64 * 1024 * 1024;
const CAP_NOTICE: &str =
    "[log] today's log reached its size cap; later lines are dropped until midnight\n";
const FILE_PREFIX: &str = "juno-";
const FILE_SUFFIX: &str = ".log";

/// Where the log files live.
pub fn log_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir().map(|home| home.join("Library/Logs/Juno"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        dirs::data_local_dir().map(|dir| dir.join("Juno/logs"))
    }
}

fn file_name(day: NaiveDate) -> String {
    format!("{FILE_PREFIX}{}{FILE_SUFFIX}", day.format("%Y-%m-%d"))
}

fn day_of(file_name: &str) -> Option<NaiveDate> {
    let date = file_name
        .strip_prefix(FILE_PREFIX)?
        .strip_suffix(FILE_SUFFIX)?;
    NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()
}

/// Remove Juno's own log files older than the retention window. Anything else in the folder
/// is left alone.
fn prune(dir: &Path, today: NaiveDate) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(day) = name.to_str().and_then(day_of) else {
            continue;
        };
        if (today - day).num_days() >= KEEP_DAYS {
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// A `tracing` writer that appends to today's file and moves to a new one at midnight.
pub struct DailyLog {
    dir: PathBuf,
    /// Bytes one day's file may reach. See [`MAX_DAY_BYTES`].
    cap: u64,
    current: Mutex<Option<OpenDay>>,
}

/// Today's file, and how full it is.
struct OpenDay {
    day: NaiveDate,
    file: File,
    written: u64,
    /// The cap notice has been written; nothing more goes to the file today.
    capped: bool,
}

impl DailyLog {
    /// Create the log folder and drop expired files. `None` if the folder cannot be created.
    pub fn open() -> Option<Self> {
        Self::open_in(log_dir()?, MAX_DAY_BYTES)
    }

    fn open_in(dir: PathBuf, cap: u64) -> Option<Self> {
        fs::create_dir_all(&dir).ok()?;
        prune(&dir, Local::now().date_naive());
        Some(Self {
            dir,
            cap,
            current: Mutex::new(None),
        })
    }

    fn write_on(&self, today: NaiveDate, buf: &[u8]) -> io::Result<usize> {
        let mut current = self.current.lock().unwrap_or_else(|e| e.into_inner());
        if !matches!(current.as_ref(), Some(open) if open.day == today) {
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(self.dir.join(file_name(today)))?;
            if current.is_some() {
                prune(&self.dir, today);
            }
            // A relaunch appends to today's file, so start from what is there.
            let written = file.metadata().map(|m| m.len()).unwrap_or(0);
            *current = Some(OpenDay {
                day: today,
                file,
                written,
                capped: written >= self.cap,
            });
        }
        let Some(open) = current.as_mut() else {
            return Ok(buf.len());
        };
        if open.written >= self.cap {
            // Past the cap the line is dropped and reported as written, so the
            // logger carries on rather than retrying every line.
            if !open.capped {
                open.capped = true;
                let _ = open.file.write_all(CAP_NOTICE.as_bytes());
            }
            return Ok(buf.len());
        }
        let n = open.file.write(buf)?;
        open.written += n as u64;
        Ok(n)
    }
}

pub struct DailyLogWriter<'a>(&'a DailyLog);

impl Write for DailyLogWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write_on(Local::now().date_naive(), buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut current = self.0.current.lock().unwrap_or_else(|e| e.into_inner());
        match current.as_mut() {
            Some(open) => open.file.flush(),
            None => Ok(()),
        }
    }
}

impl<'a> MakeWriter<'a> for DailyLog {
    type Writer = DailyLogWriter<'a>;

    fn make_writer(&'a self) -> Self::Writer {
        DailyLogWriter(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("juno-file-log-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn day(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn file_name_round_trips() {
        let d = day("2026-10-05");
        assert_eq!(file_name(d), "juno-2026-10-05.log");
        assert_eq!(day_of(&file_name(d)), Some(d));
        assert_eq!(day_of("juno-latest.log"), None);
        assert_eq!(day_of("other-2026-10-05.log"), None);
    }

    #[test]
    fn appends_and_rolls_over_at_midnight() {
        let dir = scratch_dir("roll");
        let log = DailyLog::open_in(dir.clone(), MAX_DAY_BYTES).unwrap();
        log.write_on(day("2026-10-05"), b"one\n").unwrap();
        log.write_on(day("2026-10-05"), b"two\n").unwrap();
        log.write_on(day("2026-10-06"), b"three\n").unwrap();
        assert_eq!(
            fs::read_to_string(dir.join("juno-2026-10-05.log")).unwrap(),
            "one\ntwo\n"
        );
        assert_eq!(
            fs::read_to_string(dir.join("juno-2026-10-06.log")).unwrap(),
            "three\n"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_day_stops_growing_at_the_cap_and_the_next_day_starts_fresh() {
        let dir = scratch_dir("cap");
        let log = DailyLog::open_in(dir.clone(), 10).unwrap();
        let d = day("2026-10-05");
        // The line that crosses the cap is kept whole; what follows is dropped.
        assert_eq!(log.write_on(d, b"0123456789ab\n").unwrap(), 13);
        assert_eq!(log.write_on(d, b"dropped\n").unwrap(), 8);
        assert_eq!(log.write_on(d, b"dropped again\n").unwrap(), 14);
        let text = fs::read_to_string(dir.join("juno-2026-10-05.log")).unwrap();
        assert!(text.starts_with("0123456789ab\n"));
        assert!(!text.contains("dropped\n"));
        assert_eq!(
            text.matches("size cap").count(),
            1,
            "one notice, not one per line"
        );

        log.write_on(day("2026-10-06"), b"fresh\n").unwrap();
        assert_eq!(
            fs::read_to_string(dir.join("juno-2026-10-06.log")).unwrap(),
            "fresh\n"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_relaunch_counts_what_the_file_already_holds() {
        let dir = scratch_dir("relaunch");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("juno-2026-10-05.log"), "0123456789ab\n").unwrap();
        let log = DailyLog::open_in(dir.clone(), 10).unwrap();
        log.write_on(day("2026-10-05"), b"after relaunch\n")
            .unwrap();
        let text = fs::read_to_string(dir.join("juno-2026-10-05.log")).unwrap();
        assert_eq!(
            text, "0123456789ab\n",
            "already over the cap: nothing appended"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn prune_keeps_a_week_and_ignores_other_files() {
        let dir = scratch_dir("prune");
        fs::create_dir_all(&dir).unwrap();
        for name in [
            "juno-2026-09-28.log",
            "juno-2026-09-29.log",
            "juno-2026-10-05.log",
            "notes.txt",
        ] {
            fs::write(dir.join(name), "x").unwrap();
        }
        prune(&dir, day("2026-10-05"));
        assert!(!dir.join("juno-2026-09-28.log").exists());
        assert!(dir.join("juno-2026-09-29.log").exists());
        assert!(dir.join("juno-2026-10-05.log").exists());
        assert!(dir.join("notes.txt").exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
