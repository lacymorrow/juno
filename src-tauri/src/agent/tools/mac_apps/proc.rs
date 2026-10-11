//! Run a program with arguments, never through a shell, with a deadline.
//!
//! Music, Maps and Shortcuts all end in one external command. They share this
//! one runner so every call has the same two guarantees: what the person said
//! only ever travels as an argument or on stdin, and a command that never
//! answers is killed rather than left holding a blocking thread.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// How often a running child is checked.
const POLL: Duration = Duration::from_millis(40);

/// What a finished command left behind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ran {
    /// Exit status was zero.
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
    /// The deadline passed and the command was killed.
    pub timed_out: bool,
}

/// Run `program` with `args`. `stdin` is written and closed; `None` closes
/// stdin at once, so a command that waits for input sees the end of it
/// instead of stalling.
pub fn run(
    program: &str,
    args: &[String],
    stdin: Option<&str>,
    deadline: Duration,
) -> Result<Ran, String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not start {program}: {e}"))?;

    if let (Some(text), Some(mut pipe)) = (stdin, child.stdin.take()) {
        // A command that exits without reading closes the pipe; that is not an error here.
        let _ = pipe.write_all(text.as_bytes());
    }

    let out = child.stdout.take().map(drain);
    let err = child.stderr.take().map(drain);

    let started = Instant::now();
    let (status, timed_out) = loop {
        match child.try_wait() {
            Ok(Some(status)) => break (Some(status), false),
            Ok(None) if started.elapsed() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                break (None, true);
            }
            Ok(None) => thread::sleep(POLL),
            Err(e) => return Err(format!("Lost track of {program}: {e}")),
        }
    };

    // A killed command may leave a grandchild holding its pipes open; do not
    // wait on readers that may never finish.
    let collect = |handle: Option<thread::JoinHandle<String>>| {
        handle
            .filter(|_| !timed_out)
            .and_then(|h| h.join().ok())
            .unwrap_or_default()
            .trim()
            .to_string()
    };
    Ok(Ran {
        ok: status.is_some_and(|s| s.success()),
        stdout: collect(out),
        stderr: collect(err),
        timed_out,
    })
}

fn drain<R: Read + Send + 'static>(mut reader: R) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = reader.read_to_end(&mut bytes);
        String::from_utf8_lossy(&bytes).into_owned()
    })
}

/// Run a constant AppleScript with the variable input as `argv`.
///
/// The script must be a `on run argv` handler: nothing the person said is
/// ever part of the script text.
pub fn osascript(script: &str, argv: &[String], deadline: Duration) -> Result<Ran, String> {
    let mut args = vec!["-e".to_string(), script.to_string()];
    args.extend(argv.iter().cloned());
    run("osascript", &args, None, deadline)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn arguments_are_not_interpreted_by_a_shell() {
        let ran = run(
            "echo",
            &["a; echo b $(whoami)".to_string()],
            None,
            Duration::from_secs(5),
        );
        assert_eq!(ran.map(|r| r.stdout), Ok("a; echo b $(whoami)".to_string()));
    }

    #[test]
    fn stdin_reaches_the_command() {
        let ran = run("cat", &[], Some("hello"), Duration::from_secs(5));
        assert_eq!(ran.map(|r| r.stdout), Ok("hello".to_string()));
    }

    #[test]
    fn a_command_that_never_answers_is_killed() {
        let ran = run(
            "sleep",
            &["30".to_string()],
            None,
            Duration::from_millis(200),
        );
        assert!(matches!(
            ran,
            Ok(Ran {
                timed_out: true,
                ok: false,
                ..
            })
        ));
    }
}
