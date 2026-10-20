//! Regression tests: the mux event loop must not drop a half-read server frame.
//!
//! `lsp/mux/process.rs` races the server read against accepting a client, the 10 s watchdog tick
//! and signals in one `select!`. `read_message` (`lsp/transport.rs`) keeps the header and the body
//! it has read in locals, so a read dropped by that `select!` while a frame is half read loses the
//! bytes consumed so far and the next read starts mid-frame; the mux then reports "LSP server
//! disconnected" and shuts down, taking the server with it. The event loop now pins one read
//! future outside the loop and polls it by `&mut`, so no arm can drop it.
//!
//! These tests run the real `codescout mux` against a fake LSP server that answers `initialize`,
//! waits `pre` seconds, writes a header and part of a frame, pauses `gap` seconds, finishes the
//! frame and sends a second whole message. The control (nothing races the read) and the two
//! racing cases (a client connecting during the pause, the watchdog tick landing in it) must all
//! leave the mux running AND a connected client must still be served the whole conversation
//! (`init` and both notifications), so a mux that is up but deaf to its server fails too. When
//! they fail the racing cases end at about 4.6 s and 12.5 s; a green run of the whole file takes
//! about 16 s.
//! docs/issues/2026-10-18-read-message-is-not-cancel-safe-and-the-mux-event-loop-cancels-it.md
#![cfg(unix)]

use std::io::{BufRead, BufReader, Read};
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
/// given, and keeps reading until the mux exits or `watch` has passed since `ready`.
fn run_mux_against_a_split_frame(
    pre: u32,
    gap: u32,
    client_at: Option<Duration>,
    watch: Duration,
) -> Run {
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
    let mut client: Option<UnixStream> = None;
    let mut received = Vec::new();
    let mut exited = None;
    while started.elapsed() < watch {
        if client.is_none() && client_at.is_some_and(|at| started.elapsed() >= at) {
            let stream = UnixStream::connect(&sock).expect("connect a client");
            stream.set_nonblocking(true).expect("non-blocking client");
            client = Some(stream);
        }
        if let Some(stream) = client.as_mut() {
            let mut chunk = [0u8; 4096];
            loop {
                match stream.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => received.extend_from_slice(&chunk[..n]),
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(e) => panic!("read from the mux: {e}"),
                }
            }
        }
        if let Some(status) = mux.0.try_wait().unwrap() {
            exited = Some((started.elapsed(), status));
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Run { exited, received }
}

/// What a run of [`run_mux_against_a_split_frame`] observed.
struct Run {
    /// When and how the mux exited within the watch, if it did.
    exited: Option<(Duration, std::process::ExitStatus)>,
    /// Every byte the connected client read from the mux (empty when none connected).
    received: Vec<u8>,
}

impl Run {
    /// The mux stayed up AND the connected client was served the whole conversation: the cached
    /// `init`, then both notifications the fake server sends, the first being the one whose frame
    /// was split mid-way (a 200-`x` body). A mux that is alive but no longer reads its server
    /// (for example a server-read arm that never fires) passes the first half and fails the rest.
    fn assert_alive_and_client_served(&self, what: &str) {
        assert!(
            self.exited.is_none(),
            "the mux exited {:?} {what}, although its server was alive",
            self.exited
        );
        let frames = parse_frames(&self.received);
        let kinds: Vec<String> = frames
            .iter()
            .map(|f| {
                f["type"]
                    .as_str()
                    .or_else(|| f["method"].as_str())
                    .unwrap_or("?")
                    .to_string()
            })
            .collect();
        assert_eq!(
            kinds,
            ["init", "window/logMessage", "window/logMessage"],
            "the client must be served init and both notifications {what}; got {kinds:?} from {} bytes",
            self.received.len()
        );
        assert_eq!(
            frames[1]["params"]["message"].as_str(),
            Some("x".repeat(200).as_str()),
            "the split frame must reach the client whole {what}"
        );
        assert_eq!(
            frames[2]["params"]["message"].as_str(),
            Some("second"),
            "the frame after the split one must reach the client {what}"
        );
    }
}

/// Splits `bytes` into the `Content-Length` framed JSON messages they hold. Panics on a stream
/// that is not whole frames, which is what a desynchronised mux would send.
fn parse_frames(bytes: &[u8]) -> Vec<serde_json::Value> {
    let mut frames = Vec::new();
    let mut rest = bytes;
    while !rest.is_empty() {
        let head_end = rest
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .unwrap_or_else(|| panic!("a frame header never ends in {} bytes", rest.len()));
        let head = std::str::from_utf8(&rest[..head_end]).expect("header is utf-8");
        let len: usize = head
            .lines()
            .find_map(|l| l.strip_prefix("Content-Length:"))
            .unwrap_or_else(|| panic!("no Content-Length in header {head:?}"))
            .trim()
            .parse()
            .expect("numeric Content-Length");
        let body = &rest[head_end + 4..];
        assert!(body.len() >= len, "a frame body is cut short");
        frames.push(serde_json::from_slice(&body[..len]).expect("frame body is JSON"));
        rest = &body[len..];
    }
    frames
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
    // The client connects at once, a second before the frame is split, so its accept does not
    // race the read either.
    let run =
        run_mux_against_a_split_frame(1, 1, Some(Duration::ZERO), Duration::from_millis(3500));
    run.assert_alive_and_client_served("with nothing racing the read");
}

/// A client connects 2 s in, while the server is 1 s into a 3 s pause inside a frame.
#[test]
fn a_client_connecting_while_the_server_is_mid_frame_does_not_kill_the_mux() {
    let run =
        run_mux_against_a_split_frame(1, 3, Some(Duration::from_secs(2)), Duration::from_secs(7));
    run.assert_alive_and_client_served("after a client connected mid-frame");
}

/// No client at all: the frame straddles the watchdog's 10 s tick (split at 9 s, finished at 12 s).
#[test]
fn the_watchdog_tick_during_a_split_frame_does_not_kill_the_mux() {
    // The client connects at once, about 9 s before the frame is split, so the tick is the only
    // thing that can race the read.
    let run = run_mux_against_a_split_frame(9, 3, Some(Duration::ZERO), Duration::from_secs(15));
    run.assert_alive_and_client_served("after the watchdog tick fell inside a frame");
}
