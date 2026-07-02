//! Shared harness for tesela-server integration tests that spawn the real
//! server binary.
//!
//! `pick_free_port` binds an ephemeral port, reads it back, then drops the
//! listener — a classic TOCTOU: under a parallel `cargo test` run, another
//! test's server can grab that same port before this test's child gets to
//! `bind()` it, and the child then sits there never listening while the
//! caller's `wait_for_port` burns its full 60s timeout. `spawn_with_retry`
//! closes that window by treating "didn't bind within a short per-attempt
//! timeout" as "someone else took the port" and retrying with a fresh one,
//! instead of a single long wait that can only ever time out.

#![allow(dead_code)]

use std::net::TcpStream;
use std::path::PathBuf;
use std::process::Child;
use std::time::{Duration, Instant};

pub fn binary_path() -> PathBuf {
    // CARGO_BIN_EXE_<name> is set by cargo for integration tests when the
    // package defines a [[bin]]. tesela-server's bin name is `tesela-server`.
    PathBuf::from(env!("CARGO_BIN_EXE_tesela-server"))
}

pub fn pick_free_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

/// Owns a spawned `tesela-server` child and SIGTERMs it on drop so the
/// server is reaped even if the test panics mid-flight.
pub struct ServerGuard(pub Option<Child>);

impl ServerGuard {
    pub fn stop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let pid = child.id() as i32;
            unsafe {
                libc::kill(pid, libc::SIGTERM);
            }
            let _ = child.wait();
        }
    }
}

impl Drop for ServerGuard {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Poll `addr` until it accepts a TCP connection, or give up after
/// `timeout`. Kept for call sites that already own their child and just
/// want a boolean (e.g. a second server spun up later in the same test,
/// where a retry loop would need to re-run setup too).
pub fn wait_for_port(addr: &str, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if TcpStream::connect(addr).is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

enum BindOutcome {
    Ready,
    ChildExited,
    TimedOut,
}

fn wait_for_bind(child: &mut Child, addr: &str, timeout: Duration) -> BindOutcome {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if TcpStream::connect(addr).is_ok() {
            return BindOutcome::Ready;
        }
        if let Ok(Some(_)) = child.try_wait() {
            return BindOutcome::ChildExited;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    BindOutcome::TimedOut
}

/// Pick a free port, spawn a server via `make_child(addr)`, and wait for it
/// to start accepting connections. Retries with a fresh port (up to 5
/// attempts) whenever the child exits early or fails to bind within
/// `per_attempt` — both symptoms of the ephemeral-port TOCTOU above.
///
/// Returns the live child plus its bound `addr` ("127.0.0.1:PORT") and
/// `base` ("http://127.0.0.1:PORT").
pub fn spawn_with_retry<F>(per_attempt: Duration, mut make_child: F) -> (Child, String, String)
where
    F: FnMut(&str) -> Child,
{
    const MAX_ATTEMPTS: u32 = 5;
    for attempt in 1..=MAX_ATTEMPTS {
        let port = pick_free_port();
        let addr = format!("127.0.0.1:{port}");
        let base = format!("http://{addr}");
        let mut child = make_child(&addr);
        match wait_for_bind(&mut child, &addr, per_attempt) {
            BindOutcome::Ready => return (child, addr, base),
            BindOutcome::ChildExited | BindOutcome::TimedOut if attempt < MAX_ATTEMPTS => {
                let _ = child.kill();
                let _ = child.wait();
            }
            _ => panic!("server never bound to {addr} after {MAX_ATTEMPTS} attempts"),
        }
    }
    unreachable!()
}
