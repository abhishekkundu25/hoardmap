//! Shared by the integration tests: the binary as a child process, raw HTTP over a
//! socket, CPU accounting, a stub `open` on PATH so no test opens a browser, and the
//! fixture tree the scanner tests use.
#![allow(dead_code)]

use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

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
            "hoardmap-test-{}-{}",
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
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
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
    /// The launch token from the listening line.
    pub token: String,
}

impl Server {
    /// Spawns `hoardmap <stub dir> <args>` with the stub `open` first on PATH and
    /// waits for the listening line.
    pub fn start(args: &[&str]) -> Server {
        let stub = OpenStub::new();
        let path = stub.dir.clone();
        Server::start_in(&path, args, stub)
    }

    pub fn start_in(path: &Path, args: &[&str], stub: OpenStub) -> Server {
        let mut child = Command::new(env!("CARGO_BIN_EXE_hoardmap"))
            .arg(path)
            .args(args)
            .env("PATH", stub.path_env())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn hoardmap");
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let mut first_line = String::new();
        stdout
            .read_line(&mut first_line)
            .expect("read the port line");
        let first_line = first_line.trim_end().to_string();
        let (port, token) = first_line
            .strip_prefix("Listening on http://127.0.0.1:")
            .and_then(|rest| rest.split_once("/?token="))
            .and_then(|(port, token)| Some((port.parse().ok()?, token.to_string())))
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
            token,
        }
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// The page's address as printed, with the token.
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/?token={}", self.port, self.token)
    }

    /// The `Cookie` header line the page's browser sends.
    pub fn cookie(&self) -> String {
        format!("Cookie: hoardmap-{}={}\r\n", self.port, self.token)
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
        self.raw(&format!(
            "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n{}Connection: close\r\n\r\n",
            self.port,
            self.cookie()
        ))
    }

    /// One POST with `Connection: close`; no `Content-Type` header when `content_type`
    /// is `None`.
    pub fn post(
        &self,
        path: &str,
        content_type: Option<&str>,
        body: &str,
    ) -> (u16, Vec<(String, String)>, String) {
        let content_type = content_type
            .map(|t| format!("Content-Type: {t}\r\n"))
            .unwrap_or_default();
        self.raw(&format!(
            "POST {path} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n{}{content_type}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            self.port,
            self.cookie(),
            body.len()
        ))
    }

    /// Sends `request` verbatim on a fresh connection and reads to the close.
    pub fn raw(&self, request: &str) -> (u16, Vec<(String, String)>, String) {
        let mut stream = self.connect();
        stream.write_all(request.as_bytes()).unwrap();
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).expect("read the response");
        parse_response(&raw)
    }

    /// The parsed `/api/status` document.
    pub fn status(&self) -> serde_json::Value {
        let (status, _, body) = self.get("/api/status");
        assert_eq!(status, 200, "{body}");
        serde_json::from_str(&body).expect("status JSON")
    }

    /// Polls `/api/status` every 50 ms until `state` matches; panics after `timeout`.
    pub fn wait_for(&self, state: &str, timeout: Duration) -> serde_json::Value {
        let end = Instant::now() + timeout;
        loop {
            let status = self.status();
            if status["state"] == state {
                return status;
            }
            assert!(
                Instant::now() < end,
                "status did not reach {state:?} within {timeout:?}: {status}"
            );
            thread::sleep(Duration::from_millis(50));
        }
    }

    /// Opens `/api/events` and returns the socket once the first `event: status`
    /// block has arrived. Dropping the socket is the tab closing.
    pub fn open_events(&self) -> TcpStream {
        let mut stream = self.connect();
        write!(
            stream,
            "GET /api/events HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n{}Accept: text/event-stream\r\n\r\n",
            self.port,
            self.cookie()
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

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

/// The scanner tests' tree in a temp folder: `a/big.bin` (300000 B), `a/small.txt`
/// (10 B), `b/x.jpg` (2000 B), `b/c/` empty, `.hidden` (5 B), `link.txt` hard-linked
/// to `a/small.txt`, `sym -> a/big.bin`, and `locked/` (mode 000) holding a 100 B
/// file. Removed on drop, `locked/` made writable again first.
pub struct Fixture {
    pub root: PathBuf,
    /// False when the process can read a mode-000 directory (running as root).
    pub locked_unreadable: bool,
}

impl Fixture {
    /// The symlink's own size, as the scan counts it.
    pub fn sym_len(&self) -> u64 {
        fs::symlink_metadata(self.root.join("sym")).unwrap().len()
    }

    /// The root's logical bytes as the scan reports them.
    pub fn total_bytes(&self) -> u64 {
        let locked = if self.locked_unreadable { 0 } else { 100 };
        300_000 + 10 + 2000 + 5 + self.sym_len() + locked
    }

    /// Every path under the root with its size, sorted, the unreadable folder's
    /// contents excluded; equal before and after a run means nothing was touched.
    pub fn listing(&self) -> Vec<(PathBuf, u64)> {
        fn walk(dir: &Path, out: &mut Vec<(PathBuf, u64)>) {
            let Ok(entries) = fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let meta = fs::symlink_metadata(entry.path()).unwrap();
                out.push((entry.path(), meta.len()));
                if meta.is_dir() {
                    walk(&entry.path(), out);
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.root, &mut out);
        out.sort();
        out
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::set_permissions(self.root.join("locked"), fs::Permissions::from_mode(0o755));
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub fn fixture() -> Fixture {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "hoardmap-api-{}-{}-{}",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
        nanos
    ));
    let write = |rel: &str, len: usize| {
        File::create(root.join(rel))
            .unwrap()
            .write_all(&vec![b'x'; len])
            .unwrap();
    };
    fs::create_dir_all(root.join("a")).unwrap();
    fs::create_dir_all(root.join("b/c")).unwrap();
    fs::create_dir_all(root.join("locked")).unwrap();
    write("a/big.bin", 300_000);
    write("a/small.txt", 10);
    write("b/x.jpg", 2000);
    write(".hidden", 5);
    write("locked/secret.bin", 100);
    fs::hard_link(root.join("a/small.txt"), root.join("link.txt")).unwrap();
    symlink("a/big.bin", root.join("sym")).unwrap();
    fs::set_permissions(root.join("locked"), fs::Permissions::from_mode(0o000)).unwrap();
    let locked_unreadable = fs::read_dir(root.join("locked")).is_err();
    if !locked_unreadable {
        println!("locked/ is readable (running as root): permission checks skipped");
    }
    Fixture {
        root,
        locked_unreadable,
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
