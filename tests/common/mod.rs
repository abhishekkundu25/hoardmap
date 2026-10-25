//! Shared by the integration tests: the binary as a child process, raw HTTP over a
//! socket, CPU accounting, and a stub `open` on PATH so no test opens a browser.
#![allow(dead_code)]

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

/// CPU time (user + system) a process has used so far, in seconds.
#[cfg(target_os = "linux")]
pub fn cpu_seconds(pid: u32) -> f64 {
    // Fields 14 and 15 after the parenthesised name, in clock ticks of 1/100 s.
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).expect("read /proc stat");
    let after_name = &stat[stat.rfind(')').expect("stat format") + 2..];
    let fields: Vec<&str> = after_name.split(' ').collect();
    let ticks: f64 = fields[11].parse::<f64>().unwrap() + fields[12].parse::<f64>().unwrap();
    ticks / 100.0
}

/// CPU time (user + system) a process has used so far, in seconds.
#[cfg(not(target_os = "linux"))]
pub fn cpu_seconds(pid: u32) -> f64 {
    let out = Command::new("ps")
        .args(["-o", "time=", "-p", &pid.to_string()])
        .output()
        .expect("run ps");
    parse_ps_time(String::from_utf8_lossy(&out.stdout).trim())
}

/// `[[dd-]hh:]mm:ss.cc`, as `ps -o time=` prints it.
pub fn parse_ps_time(text: &str) -> f64 {
    let (days, rest) = match text.split_once('-') {
        Some((d, r)) => (d.parse::<f64>().unwrap_or(0.0), r),
        None => (0.0, text),
    };
    rest.split(':')
        .map(|x| x.parse::<f64>().unwrap_or(0.0))
        .fold(0.0, |acc, x| acc * 60.0 + x)
        + days * 86400.0
}

/// CPU the server uses over 2 s of doing nothing; under 0.1 s passes. Returns the
/// seconds used.
pub fn assert_idle(server: &Server, what: &str) -> f64 {
    thread::sleep(Duration::from_millis(300));
    let before = cpu_seconds(server.pid());
    thread::sleep(Duration::from_secs(2));
    let used = cpu_seconds(server.pid()) - before;
    println!("# idle CPU, {what}: {used:.2} s in 2 s");
    assert!(
        used < 0.1,
        "idle server ({what}) used {used:.2} s of CPU in 2 s"
    );
    used
}

static NEXT_STUB: AtomicUsize = AtomicUsize::new(0);

/// A temp folder holding `open` and `xdg-open` scripts that write their arguments,
/// one per line, to `marker`. Removed on drop.
pub struct OpenStub {
    pub dir: PathBuf,
    pub marker: PathBuf,
}

impl OpenStub {
    pub fn new() -> OpenStub {
        let dir = std::env::temp_dir().join(format!(
            "macdirtree-test-{}-{}",
            std::process::id(),
            NEXT_STUB.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).expect("create stub dir");
        let marker = dir.join("open-args");
        let script = format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"{}\"\n",
            marker.display()
        );
        for name in ["open", "xdg-open"] {
            let path = dir.join(name);
            fs::write(&path, &script).expect("write stub");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
            }
        }
        OpenStub { dir, marker }
    }

    /// The PATH value that puts the stub first.
    pub fn path_env(&self) -> String {
        let rest = std::env::var("PATH").unwrap_or_default();
        format!("{}:{rest}", self.dir.display())
    }

    /// What the stub was called with, if it ran: its arguments, one per line.
    pub fn open_args(&self) -> Option<String> {
        fs::read_to_string(&self.marker).ok()
    }
}

impl Drop for OpenStub {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.dir).ok();
    }
}

/// The binary as a child process, killed on drop.
pub struct Server {
    child: Child,
    stdout: BufReader<ChildStdout>,
    pub port: u16,
    /// The first stdout line, without the newline.
    pub first_line: String,
    pub stub: OpenStub,
}

impl Server {
    /// Spawns `macdirtree <stub dir> <args>` with the stub `open` first on PATH and
    /// waits for the listening line.
    pub fn start(args: &[&str]) -> Server {
        let stub = OpenStub::new();
        let path = stub.dir.clone();
        Server::start_in(&path, args, stub)
    }

    pub fn start_in(path: &Path, args: &[&str], stub: OpenStub) -> Server {
        let mut child = Command::new(env!("CARGO_BIN_EXE_macdirtree"))
            .arg(path)
            .args(args)
            .env("PATH", stub.path_env())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn macdirtree");
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let mut first_line = String::new();
        stdout
            .read_line(&mut first_line)
            .expect("read the port line");
        let first_line = first_line.trim_end().to_string();
        let port = first_line
            .strip_prefix("Listening on http://127.0.0.1:")
            .and_then(|rest| rest.strip_suffix('/'))
            .and_then(|port| port.parse().ok())
            .unwrap_or_else(|| {
                child.kill().ok();
                panic!("unexpected first line: {first_line:?}")
            });
        Server {
            child,
            stdout,
            port,
            first_line,
            stub,
        }
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/", self.port)
    }

    /// A fresh connection with a 10 s read timeout.
    pub fn connect(&self) -> TcpStream {
        let stream = TcpStream::connect(("127.0.0.1", self.port)).expect("connect");
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        stream
    }

    /// One GET with `Connection: close`: status, headers and body.
    pub fn get(&self, path: &str) -> (u16, Vec<(String, String)>, String) {
        let mut stream = self.connect();
        write!(
            stream,
            "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
            self.port
        )
        .unwrap();
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).expect("read the response");
        parse_response(&raw)
    }

    /// Opens `/api/events` and returns the socket once the first `event: status`
    /// block has arrived. Dropping the socket is the tab closing.
    pub fn open_events(&self) -> TcpStream {
        let mut stream = self.connect();
        write!(
            stream,
            "GET /api/events HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAccept: text/event-stream\r\n\r\n",
            self.port
        )
        .unwrap();
        let mut got = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            let n = stream.read(&mut buf).expect("read the event stream");
            assert!(n > 0, "the event stream closed before the first status");
            got.extend_from_slice(&buf[..n]);
            let text = String::from_utf8_lossy(&got);
            if let Some(i) = text.find("event: status") {
                if text[i..].contains("\n\n") {
                    return stream;
                }
            }
        }
    }

    pub fn is_running(&mut self) -> bool {
        self.child.try_wait().expect("try_wait").is_none()
    }

    /// Polls for the process to exit, up to `timeout`.
    pub fn wait_exit(&mut self, timeout: Duration) -> Option<ExitStatus> {
        let end = Instant::now() + timeout;
        loop {
            if let Some(status) = self.child.try_wait().expect("try_wait") {
                return Some(status);
            }
            if Instant::now() >= end {
                return None;
            }
            thread::sleep(Duration::from_millis(50));
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.child.kill().ok();
        self.child.wait().ok();
    }
}

/// Splits a raw HTTP/1.1 response into status, headers and body.
pub fn parse_response(raw: &[u8]) -> (u16, Vec<(String, String)>, String) {
    let text = String::from_utf8_lossy(raw);
    let (head, body) = text.split_once("\r\n\r\n").expect("response head");
    let mut lines = head.split("\r\n");
    let status_line = lines.next().unwrap();
    let status = status_line
        .split(' ')
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| panic!("bad status line: {status_line:?}"));
    let headers = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect();
    (status, headers, body.to_string())
}

/// The first header with this name, compared case-insensitively.
pub fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

#[test]
fn ps_time_parses_every_shape() {
    assert_eq!(parse_ps_time("0:00.03"), 0.03);
    assert_eq!(parse_ps_time("1:02.50"), 62.5);
    assert_eq!(parse_ps_time("01:02:03.00"), 3723.0);
    assert_eq!(parse_ps_time("2-01:02:03.00"), 2.0 * 86400.0 + 3723.0);
}
