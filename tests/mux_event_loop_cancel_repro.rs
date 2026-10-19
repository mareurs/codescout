//! OPEN BUG repro, ignored on purpose: the mux event loop drops a half-read server frame.
//!
//! `lsp/mux/process.rs` races `read_message(server_reader)` against accepting a client, the 10 s
//! watchdog tick and signals in one `select!`. `read_message` (`lsp/transport.rs`) keeps the header
//! and the body it has read in locals, so when another arm wins while a frame is half read, the
//! bytes consumed so far are dropped with the future and the next read starts mid-frame. The mux
//! then reads garbage, reports "LSP server disconnected" and shuts down, taking the server with it.
//!
//! These tests run the real `codescout mux` against a fake LSP server that answers `initialize`,
//! waits `pre` seconds, writes a header and part of a frame, pauses `gap` seconds, finishes the
//! frame and sends a second whole message. The control (nothing races the read) must stay up; a
//! client connecting during the pause (the accept arm), or the watchdog tick landing in it, must
//! not be able to kill the mux either. Run the ignored ones with `--ignored`:
//! `scripts/with-slot.sh cargo test --test mux_event_loop_cancel_repro -- --ignored`
//! (nothing under `scripts/` or `.github/` passes `--ignored` or `--include-ignored`.)
//! docs/issues/2026-10-18-read-message-is-not-cancel-safe-and-the-mux-event-loop-cancels-it.md
#![cfg(unix)]

use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixStream;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// How long a mux gets to print `ready`; a mux that neither prints it nor exits fails the test
/// instead of hanging the gate.
const READY_DEADLINE: Duration = Duration::from_secs(30);

/// `fake_lsp.py <pre> <gap>`. Exits on its own after 20 s so a mux that was killed leaves nothing
/// behind for long.
const FAKE_LSP: &str = r#"
import sys, json, time

def read():
    head = b""
    while not head.endswith(b"\r\n\r\n"):
        c = sys.stdin.buffer.read(1)
        if not c:
            sys.exit(0)
        head += c
    n = int([l for l in head.split(b"\r\n") if l.lower().startswith(b"content-length")][0].split(b":")[1])
    return json.loads(sys.stdin.buffer.read(n))

def raw(b):
    sys.stdout.buffer.write(b)
    sys.stdout.buffer.flush()

def frame(o):
    b = json.dumps(o).encode()
    return b"Content-Length: %d\r\n\r\n" % len(b) + b

pre, gap = float(sys.argv[1]), float(sys.argv[2])
init = read()
raw(frame({"jsonrpc": "2.0", "id": init["id"], "result": {"capabilities": {}}}))
read()
time.sleep(pre)
first = frame({"jsonrpc": "2.0", "method": "window/logMessage", "params": {"type": 3, "message": "x" * 200}})
raw(first[:60])
time.sleep(gap)
raw(first[60:])
raw(frame({"jsonrpc": "2.0", "method": "window/logMessage", "params": {"type": 3, "message": "second"}}))
time.sleep(20)
"#;

/// A spawned child that is stopped on EVERY exit path of the test, panics included: SIGTERM, a
/// grace of 2 s to let the mux kill its server, then SIGKILL.
struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(Some(_))) {
            return;
        }
        // SAFETY: `kill(2)` on the pid of a child this guard owns and has not reaped (the
        // `try_wait` above found it still running).
        unsafe {
            libc::kill(self.0.id() as libc::pid_t, libc::SIGTERM);
        }
        let until = Instant::now() + Duration::from_secs(2);
        while Instant::now() < until {
            if matches!(self.0.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Waits up to `deadline` for the first line of the child's stdout. The read runs on a thread so
/// a child that prints nothing cannot block the caller.
fn first_stdout_line(child: &mut Child, deadline: Duration) -> Result<String, String> {
    let stdout = child.stdout.take().expect("stdout is piped");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let _ = BufReader::new(stdout).read_line(&mut line);
        let _ = tx.send(line);
    });
    rx.recv_timeout(deadline)
        .map_err(|_| format!("no output line within {deadline:?}"))
}

/// Runs the real mux against the fake server. A client connects `client_at` after `ready`, if
/// given. Returns when and how the mux exited within `watch` of `ready`, if it did.
fn run_mux_against_a_split_frame(
    pre: u32,
    gap: u32,
    client_at: Option<Duration>,
    watch: Duration,
) -> Option<(Duration, std::process::ExitStatus)> {
    let dir = tempfile::tempdir().unwrap();
    let server = dir.path().join("fake_lsp.py");
    std::fs::write(&server, FAKE_LSP).unwrap();
    let sock = dir.path().join("m.sock");
    let lock = dir.path().join("m.lock");

    let mut mux = ChildGuard(
        Command::new(env!("CARGO_BIN_EXE_codescout"))
            .arg("mux")
            .arg("--socket")
            .arg(&sock)
            .arg("--lock")
            .arg(&lock)
            .arg("--cwd")
            .arg(dir.path())
            .args(["--idle-timeout", "60", "--", "python3"])
            .arg(&server)
            .arg(pre.to_string())
            .arg(gap.to_string())
            .stdout(Stdio::piped())
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn the mux"),
    );
    let ready = first_stdout_line(&mut mux.0, READY_DEADLINE).expect("the mux must print ready");
    assert!(
        ready.starts_with("ready"),
        "the mux must come up: {ready:?}"
    );

    let started = Instant::now();
    let mut client = None;
    let mut exited = None;
    while started.elapsed() < watch {
        if client.is_none() && client_at.is_some_and(|at| started.elapsed() >= at) {
            client = Some(UnixStream::connect(&sock).expect("connect a client"));
        }
        if let Some(status) = mux.0.try_wait().unwrap() {
            exited = Some((started.elapsed(), status));
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    exited
}

/// The deadline works: a program that prints nothing and does not exit fails the wait instead of
/// hanging it, and the guard stops it. (The first version read `ready` with a bare `read_line`.)
#[test]
fn a_child_that_never_prints_ready_fails_the_wait_and_is_stopped() {
    let mut child = ChildGuard(
        Command::new("sleep")
            .arg("60")
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn sleep"),
    );
    let pid = child.0.id();
    let started = Instant::now();
    let result = first_stdout_line(&mut child.0, Duration::from_millis(500));
    assert!(result.is_err(), "sleep prints nothing: {result:?}");
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "the wait must end at its deadline, took {:?}",
        started.elapsed()
    );
    drop(child);
    assert!(
        !std::path::Path::new(&format!("/proc/{pid}")).exists(),
        "the guard must have stopped and reaped the child"
    );
}

/// Control: the frame is split 1 s after `ready` and finished 1 s later, nothing races the read
/// (the watchdog's next tick is at 10 s), and the mux stays up through the second message and a
/// second more, so the exits below are the cancelled read and not the split frame. 3.5 s watched
/// (it was 7 s; the frame is finished at about 2 s).
#[test]
fn a_split_frame_alone_does_not_kill_the_mux() {
    let exited = run_mux_against_a_split_frame(1, 1, None, Duration::from_millis(3500));
    assert!(
        exited.is_none(),
        "the mux exited with nothing racing the read: {exited:?}"
    );
}

/// A client connects 2 s in, while the server is 1 s into a 3 s pause inside a frame.
#[test]
#[ignore = "open bug: docs/issues/2026-10-18-read-message-is-not-cancel-safe-and-the-mux-event-loop-cancels-it.md"]
fn a_client_connecting_while_the_server_is_mid_frame_does_not_kill_the_mux() {
    let exited =
        run_mux_against_a_split_frame(1, 3, Some(Duration::from_secs(2)), Duration::from_secs(7));
    assert!(
        exited.is_none(),
        "the mux exited {exited:?} after a client connected mid-frame, although its server was alive",
    );
}

/// No client at all: the frame straddles the watchdog's 10 s tick (split at 9 s, finished at 12 s).
#[test]
#[ignore = "open bug: docs/issues/2026-10-18-read-message-is-not-cancel-safe-and-the-mux-event-loop-cancels-it.md"]
fn the_watchdog_tick_during_a_split_frame_does_not_kill_the_mux() {
    let exited = run_mux_against_a_split_frame(9, 3, None, Duration::from_secs(15));
    assert!(
        exited.is_none(),
        "the mux exited {exited:?} after the watchdog tick fell inside a frame, although its server was alive",
    );
}
