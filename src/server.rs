//! The HTTP server on 127.0.0.1 behind the app's window: serves the page and routes
//! `/api/*` to [`crate::api`]. Only the page the app loads holds the launch token.
//!
//! HTTP/1.1 is spoken directly over `std::net`: one thread per connection, one
//! response per connection (`Connection: close`). An event stream owns its socket, so a
//! blocking read on a clone of it notices the tab closing at once; nothing polls.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering as AtomicOrdering;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::json;

use crate::api::{self, lossy};
use crate::scan::{self, Progress};
use crate::tree::Tree;
use crate::treemap::Colours;

const INDEX_HTML: &str = include_str!("../web/index.html");
const APP_JS: &str = include_str!("../web/app.js");
const STYLE_CSS: &str = include_str!("../web/style.css");
const LOGO_SVG: &str = include_str!("../assets/hoardmap.svg");

/// A request line plus headers longer than this is refused with 431.
const MAX_HEAD_BYTES: usize = 16 * 1024;
/// The answer to a request without the launch token.
const NO_TOKEN: &str =
    "This address is from another launch. Open the one hoardmap printed, or reopen the app.\n";

/// A request body longer than this is refused with 413.
pub const MAX_BODY_BYTES: usize = 64 * 1024;
const KEEPALIVE: Duration = Duration::from_secs(15);
/// How often the status goes out on the event streams while a scan runs.
const TICK: Duration = Duration::from_millis(250);

/// Ticks between rebuilds of a running scan's preview tree.
const LIVE_EVERY: u32 = 4;

/// A running scan's preview: a tree of what it has finished so far.
#[derive(Debug)]
pub struct Live {
    pub tree: Arc<Tree>,
    pub colours: Colours,
    /// Finished entries it was built from.
    pub entries: usize,
}

/// The `start_scan` error while a scan runs; the API answers it with 409.
pub const SCAN_RUNNING: &str = "a scan is running";

/// The `stop_scan` error when none runs; the API answers it with 409.
pub const NO_SCAN: &str = "no scan is running";

/// How long a stopped scan may take to wind down before its preview replaces it.
const STOP_WAIT: Duration = Duration::from_millis(1500);

/// The state of a scan whose folder disappeared, such as a drive unplugged mid-scan.
fn gone(path: PathBuf) -> State {
    let error = format!(
        "{} went away during the scan; was the drive disconnected?",
        path.display()
    );
    State::Failed { path, error }
}

/// What the server knows about the scan.
#[derive(Debug)]
pub enum State {
    Idle,
    Scanning {
        path: PathBuf,
        progress: Arc<Progress>,
        started: Instant,
        /// What the scan has finished so far, rebuilt every [`LIVE_EVERY`] ticks; the
        /// tree endpoints answer from it until the scan is done.
        live: Option<Live>,
    },
    Done {
        path: PathBuf,
        tree: Arc<Tree>,
        duration: Duration,
        colours: Colours,
        /// Stopped before the end: the folders it had not read count as empty.
        stopped: bool,
        /// The running scan's preview stood in for a scan that did not wind down: only
        /// the top levels, and no unreadable folders.
        preview: bool,
    },
    Failed {
        path: PathBuf,
        error: String,
    },
}

/// Shared by every connection thread.
pub struct App {
    pub state: Mutex<State>,
    pub events: Broadcaster,
    /// The bound port, which a request's `Host` header must name.
    pub port: u16,
    /// A secret made at launch. Only the page's own address carries it; a request
    /// without it in the [`cookie_name`] cookie is refused, so other programs on the
    /// Mac cannot use the server.
    pub token: String,
    /// Served to the app's window; false only for the debug test server.
    pub native: bool,
    /// The folder to scan first; the idle status names it so the page can start it.
    pub start: Mutex<Option<PathBuf>>,
    /// `(total, free)` of the scanned folder's volume, read when a scan starts and ends
    /// rather than on each status, since a network drive that stopped answering blocks
    /// the read.
    disk: Mutex<Option<(u64, u64)>>,
}

impl App {
    pub fn new(port: u16, native: bool) -> App {
        App {
            state: Mutex::new(State::Idle),
            events: Broadcaster::default(),
            port,
            token: new_token(),
            native,
            start: Mutex::new(None),
            disk: Mutex::new(None),
        }
    }

    fn read_disk(&self, path: &Path) {
        *self.disk.lock().unwrap() = disk(path);
    }

    /// The `/api/status` document for the current state, with the size and free space
    /// of the volume the folder is on.
    pub fn status(&self) -> serde_json::Value {
        self.status_of(&self.state.lock().unwrap())
    }

    fn status_of(&self, state: &State) -> serde_json::Value {
        let mut status = self.state_json(state);
        let disk = *self.disk.lock().unwrap();
        status["disk"] = json!(disk.map(|(total, free)| json!({ "total": total, "free": free })));
        status
    }

    fn state_json(&self, state: &State) -> serde_json::Value {
        match state {
            State::Idle => status_json(
                "idle",
                self.start.lock().unwrap().as_deref().map(lossy),
                (0, 0, 0, 0),
                (false, 0),
                0,
                0,
                None,
            ),
            State::Scanning {
                path,
                progress,
                started,
                live,
            } => {
                let mut status = status_json(
                    "scanning",
                    Some(lossy(path)),
                    progress.snapshot(),
                    (progress.clone_aware.load(AtomicOrdering::Relaxed), 0),
                    started.elapsed().as_millis(),
                    0,
                    None,
                );
                // Changes whenever the preview does, so the page knows to reload it.
                status["live"] = json!(live.as_ref().map(|l| l.entries));
                status
            }
            State::Done {
                path,
                tree,
                duration,
                stopped,
                preview,
                ..
            } => {
                let root = tree.root();
                let mut status = status_json(
                    "done",
                    Some(lossy(path)),
                    (
                        u64::from(root.dirs),
                        u64::from(root.files),
                        root.size,
                        root.physical,
                    ),
                    (tree.clone_aware, tree.clone_counted_once),
                    duration.as_millis(),
                    tree.error_count,
                    None,
                );
                status["stopped"] = json!(stopped);
                status["preview"] = json!(preview);
                status
            }
            State::Failed { path, error } => status_json(
                "failed",
                Some(lossy(path)),
                (0, 0, 0, 0),
                (false, 0),
                0,
                0,
                Some(error),
            ),
        }
    }

    /// Sends the current status to every open event stream.
    /// Sent under the state lock, so the events go out in the order the state changed.
    pub fn notify_status(&self) {
        let state = self.state.lock().unwrap();
        self.events
            .send("status", &self.status_of(&state).to_string());
    }

    /// Starts scanning `path` on its own thread, with a ticker that sends the status
    /// every 250 ms until the scan ends. Refused while a scan runs or when `path` is not
    /// a directory.
    pub fn start_scan(self: &Arc<App>, path: PathBuf) -> Result<(), String> {
        let progress = Arc::new(Progress::default());
        let started = Instant::now();
        {
            let mut state = self.state.lock().unwrap();
            if matches!(*state, State::Scanning { .. }) {
                return Err(SCAN_RUNNING.to_string());
            }
            if !path.is_dir() {
                return Err(format!("{} is not a directory", path.display()));
            }
            *state = State::Scanning {
                path: path.clone(),
                progress: progress.clone(),
                started,
                live: None,
            };
        }
        self.read_disk(&path);
        self.notify_status();
        let app = self.clone();
        let ticker = progress.clone();
        thread::spawn(move || app.tick(&ticker));
        let app = self.clone();
        thread::spawn(move || app.finish_scan(path, &progress, started));
        Ok(())
    }

    fn finish_scan(&self, path: PathBuf, progress: &Arc<Progress>, started: Instant) {
        let path_for_disk = path.clone();
        let next = match scan::scan(&path, progress) {
            Ok(_) if !path.exists() => gone(path),
            Ok(tree) => State::Done {
                path,
                colours: Colours::new(&tree),
                tree: Arc::new(tree),
                duration: started.elapsed(),
                stopped: progress.stop.load(AtomicOrdering::Relaxed),
                preview: false,
            },
            Err(error) => State::Failed {
                path,
                error: error.to_string(),
            },
        };
        self.read_disk(&path_for_disk);
        self.replace_scan(progress, next);
    }

    /// Puts `next` in place of the scan `progress` belongs to; false when that scan is
    /// no longer the current state, which is then left alone.
    fn replace_scan(&self, progress: &Arc<Progress>, next: State) -> bool {
        {
            let mut state = self.state.lock().unwrap();
            if !matches!(&*state, State::Scanning { progress: p, .. } if Arc::ptr_eq(p, progress)) {
                return false;
            }
            *state = next;
        }
        self.notify_status();
        true
    }

    /// A folder handed over by Finder or the `hoardmap` command. Before the first scan
    /// (the start screen is up) it becomes the folder that scan reads; otherwise it
    /// replaces the current scan, stopping a running one first. Blocks while a running
    /// scan winds down, so call it off the event loop.
    pub fn open_folder(self: &Arc<App>, path: PathBuf) -> Result<(), String> {
        if !path.is_dir() {
            return Err(format!("{} is not a directory", path.display()));
        }
        if matches!(*self.state.lock().unwrap(), State::Idle) {
            *self.start.lock().unwrap() = Some(path);
            self.notify_status();
            return Ok(());
        }
        if self.stop_scan().is_ok() {
            let end = Instant::now() + STOP_WAIT + Duration::from_secs(1);
            while matches!(*self.state.lock().unwrap(), State::Scanning { .. })
                && Instant::now() < end
            {
                thread::sleep(Duration::from_millis(20));
            }
        }
        self.start_scan(path)
    }

    /// Asks the running scan to stop. It ends with what it has read; if it is still
    /// stuck after [`STOP_WAIT`], for instance on a network drive that stopped
    /// answering, its preview takes its place at once.
    pub fn stop_scan(self: &Arc<App>) -> Result<(), String> {
        let (path, progress, started) = match &*self.state.lock().unwrap() {
            State::Scanning {
                path,
                progress,
                started,
                ..
            } => (path.clone(), progress.clone(), *started),
            _ => return Err(NO_SCAN.to_string()),
        };
        progress.stop.store(true, AtomicOrdering::Relaxed);
        let app = self.clone();
        thread::spawn(move || {
            thread::sleep(STOP_WAIT);
            let finished = progress.finished.lock().unwrap().clone();
            let mut tree = scan::partial(&lossy(&path), &finished);
            tree.clone_aware = progress.clone_aware.load(AtomicOrdering::Relaxed);
            app.replace_scan(
                &progress,
                State::Done {
                    path,
                    colours: Colours::new(&tree),
                    tree: Arc::new(tree),
                    duration: started.elapsed(),
                    stopped: true,
                    preview: true,
                },
            );
        });
        Ok(())
    }

    /// The scan's progress counters identify it, so a ticker outlives neither its own
    /// scan nor into the next one.
    fn tick(&self, progress: &Arc<Progress>) {
        let mut built = 0;
        for ticks in 1.. {
            thread::sleep(TICK);
            let (root, path) = match &*self.state.lock().unwrap() {
                State::Scanning {
                    progress: p, path, ..
                } if Arc::ptr_eq(p, progress) => (lossy(path), path.clone()),
                _ => return,
            };
            if ticks % LIVE_EVERY == 0 && !path.exists() {
                // An unplugged drive: end now rather than wait on reads that may never return.
                progress.stop.store(true, AtomicOrdering::Relaxed);
                self.replace_scan(progress, gone(path));
                return;
            }
            if ticks % LIVE_EVERY == 0 {
                let finished = {
                    let finished = progress.finished.lock().unwrap();
                    (finished.len() != built).then(|| finished.clone())
                };
                if let Some(finished) = finished {
                    built = finished.len();
                    let mut tree = scan::partial(&root, &finished);
                    tree.clone_aware = progress.clone_aware.load(AtomicOrdering::Relaxed);
                    let live = Live {
                        colours: Colours::new(&tree),
                        tree: Arc::new(tree),
                        entries: built,
                    };
                    if let State::Scanning {
                        progress: p,
                        live: slot,
                        ..
                    } = &mut *self.state.lock().unwrap()
                        && Arc::ptr_eq(p, progress)
                    {
                        *slot = Some(live);
                    }
                }
            }
            // Checked and sent under the state lock, so a scan that has finished in the
            // meantime gets neither a second `done` nor a late `scanning` from here.
            match &*self.state.lock().unwrap() {
                state @ State::Scanning { progress: p, .. } if Arc::ptr_eq(p, progress) => {
                    self.events
                        .send("status", &self.status_of(state).to_string());
                }
                _ => return,
            }
        }
    }
}

/// `(total, free)` bytes of the volume holding `path`; free is what an ordinary user
/// may still write.
pub fn disk(path: &Path) -> Option<(u64, u64)> {
    let cpath = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).ok()?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: a valid C string and a statvfs the call fills in.
    if unsafe { libc::statvfs(cpath.as_ptr(), &mut stat) } != 0 {
        return None;
    }
    let block = stat.f_frsize as u64;
    Some((stat.f_blocks as u64 * block, stat.f_bavail as u64 * block))
}

/// `clones` is the tree's `(clone_aware, clone_counted_once)`.
fn status_json(
    state: &str,
    path: Option<String>,
    (dirs, files, bytes, physical): (u64, u64, u64, u64),
    (clone_aware, clone_counted_once): (bool, u64),
    elapsed_ms: u128,
    error_count: u64,
    error: Option<&str>,
) -> serde_json::Value {
    json!({
        "state": state,
        "path": path,
        "dirs": dirs,
        "files": files,
        "bytes": bytes,
        "physical": physical,
        "clone_aware": clone_aware,
        "clone_counted_once": clone_counted_once,
        "elapsed_ms": elapsed_ms as u64,
        "error_count": error_count,
        "error": error,
    })
}

enum Signal {
    Event(String),
    /// The tab's socket closed.
    Gone,
}

/// Fan-out of SSE events to every open stream.
#[derive(Default)]
pub struct Broadcaster {
    subscribers: Mutex<Vec<Sender<Signal>>>,
}

impl Broadcaster {
    /// Queues `event: <event>\ndata: <data>\n\n` on every stream; `data` must not
    /// contain a newline.
    pub fn send(&self, event: &str, data: &str) {
        let text = format!("event: {event}\ndata: {data}\n\n");
        self.subscribers
            .lock()
            .unwrap()
            .retain(|tx| tx.send(Signal::Event(text.clone())).is_ok());
    }

    fn subscribe(&self) -> (Sender<Signal>, Receiver<Signal>) {
        let (tx, rx) = mpsc::channel();
        self.subscribers.lock().unwrap().push(tx.clone());
        (tx, rx)
    }
}

/// A parsed request. `path` and `query` are the two halves of the request target.
#[derive(Debug)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub query: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    /// The first header with this name, compared case-insensitively.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

#[derive(Debug)]
pub enum Body {
    Bytes(Vec<u8>),
    /// The server keeps the connection and streams events on it.
    EventStream,
}

#[derive(Debug)]
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Body,
}

impl Response {
    pub fn new(status: u16, content_type: &str, body: impl Into<Vec<u8>>) -> Response {
        Response {
            status,
            headers: vec![
                ("Content-Type".into(), content_type.into()),
                ("Cache-Control".into(), "no-store".into()),
            ],
            body: Body::Bytes(body.into()),
        }
    }

    pub fn text(status: u16, body: &str) -> Response {
        Response::new(status, "text/plain; charset=utf-8", body)
    }

    pub fn json(status: u16, value: &impl Serialize) -> Response {
        let body = serde_json::to_vec(value).expect("JSON value");
        Response::new(status, "application/json", body)
    }

    pub fn event_stream() -> Response {
        Response {
            status: 200,
            headers: vec![
                ("Content-Type".into(), "text/event-stream".into()),
                ("Cache-Control".into(), "no-store".into()),
            ],
            body: Body::EventStream,
        }
    }

    pub fn with_header(mut self, name: &str, value: &str) -> Response {
        self.headers.push((name.into(), value.into()));
        self
    }
}

/// How [`spawn`] starts the server.
#[derive(Clone, Debug)]
pub struct Config {
    pub path: PathBuf,
    /// Tried first; when it is taken, or 0, the OS picks one.
    pub port: u16,
    pub native: bool,
    /// Scan `path` at once; otherwise wait for the page to ask.
    pub scan_at_start: bool,
}

/// A running server: the bound port, the page's address with the token, and the
/// state, for handing it folders.
pub struct Spawned {
    pub port: u16,
    pub url: String,
    pub app: Arc<App>,
}

/// Binds 127.0.0.1, starts the scan of `config.path` and serves on a background
/// thread.
pub fn spawn(config: Config) -> io::Result<Spawned> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, config.port))
        .or_else(|_| TcpListener::bind((Ipv4Addr::LOCALHOST, 0)))?;
    let port = listener.local_addr()?.port();
    let app = App::new(port, config.native);
    *app.start.lock().unwrap() = Some(config.path.clone());
    app.read_disk(&config.path);
    let url = page_url(port, &app.token);
    let app = Arc::new(app);
    if config.scan_at_start
        && let Err(error) = app.start_scan(config.path.clone())
    {
        eprintln!("hoardmap: {error}");
        *app.state.lock().unwrap() = State::Failed {
            path: config.path,
            error,
        };
    }
    let served = app.clone();
    thread::spawn(move || {
        loop {
            match listener.accept() {
                Ok((stream, _)) => {
                    let app = app.clone();
                    thread::spawn(move || serve(&app, stream).ok());
                }
                Err(error) => {
                    eprintln!("hoardmap: accept failed: {error}");
                    thread::sleep(Duration::from_millis(100));
                }
            }
        }
    });
    Ok(Spawned {
        port,
        url,
        app: served,
    })
}

/// The page's address with the launch token; the first visit trades it for a cookie.
pub fn page_url(port: u16, token: &str) -> String {
    format!("http://127.0.0.1:{port}/?token={token}")
}

/// The token's cookie. Cookies ignore the port, so the name carries it: two copies of
/// the app on different ports keep theirs apart.
pub fn cookie_name(port: u16) -> String {
    format!("hoardmap-{port}")
}

/// 32 random bytes, hex.
fn new_token() -> String {
    let mut bytes = [0u8; 32];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .expect("read /dev/urandom");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Compares in time that does not depend on where the strings differ.
fn same_secret(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0, |d, (x, y)| d | (x ^ y)) == 0
}

/// The request carries the launch token in its cookie.
fn has_token(app: &App, request: &Request) -> bool {
    let name = cookie_name(app.port);
    request.header("Cookie").is_some_and(|cookies| {
        cookies
            .split(';')
            .filter_map(|c| c.trim().split_once('='))
            .any(|(k, v)| k == name && same_secret(v, &app.token))
    })
}

fn serve(app: &Arc<App>, mut stream: TcpStream) -> io::Result<()> {
    let request = match read_request(&stream) {
        Ok(Some(request)) => request,
        Ok(None) => return Ok(()),
        Err(status) => {
            write_response(&mut stream, &Response::text(status, reason(status)))?;
            return drain(&mut stream);
        }
    };
    let response = route(app, &request);
    match response.body {
        Body::EventStream => serve_events(app, stream, &response),
        Body::Bytes(_) => write_response(&mut stream, &response),
    }
}

fn route(app: &Arc<App>, request: &Request) -> Response {
    // A page on another origin that resolved its name to 127.0.0.1 (DNS rebinding)
    // would otherwise be same-origin here.
    if !host_allowed(request.header("Host"), app.port) {
        return Response::text(403, "forbidden\n");
    }
    // The address hoardmap opened: keep the token as a cookie and drop it from the
    // address bar and history.
    if request.path == "/"
        && let Some(given) = request
            .query
            .split('&')
            .find_map(|p| p.strip_prefix("token="))
    {
        if !same_secret(given, &app.token) {
            return Response::text(403, NO_TOKEN);
        }
        let cookie = format!(
            "{}={}; Path=/; HttpOnly; SameSite=Strict",
            cookie_name(app.port),
            app.token
        );
        return Response::text(303, "")
            .with_header("Location", "/")
            .with_header("Set-Cookie", &cookie);
    }
    if !has_token(app, request) {
        return Response::text(403, NO_TOKEN);
    }
    if request.path.starts_with("/api/") {
        return api::handle(app, request);
    }
    if request.method != "GET" {
        return Response::text(405, "method not allowed\n").with_header("Allow", "GET");
    }
    match request.path.as_str() {
        "/" => Response::new(200, "text/html; charset=utf-8", INDEX_HTML),
        "/app.js" => Response::new(200, "application/javascript; charset=utf-8", APP_JS),
        "/style.css" => Response::new(200, "text/css; charset=utf-8", STYLE_CSS),
        "/logo.svg" => Response::new(200, "image/svg+xml", LOGO_SVG),
        _ => Response::text(404, "not found\n"),
    }
}

/// `127.0.0.1` or `localhost`, with no port or with the bound one.
fn host_allowed(host: Option<&str>, port: u16) -> bool {
    let Some(host) = host else {
        return false;
    };
    let (name, given) = match host.rsplit_once(':') {
        Some((name, port)) => (name, Some(port)),
        None => (host, None),
    };
    (name == "127.0.0.1" || name.eq_ignore_ascii_case("localhost"))
        && given.is_none_or(|p| p.parse::<u16>() == Ok(port))
}

/// Reads one request. `Ok(None)` when the client closed without sending one; `Err` is
/// the status code to answer with.
fn read_request(stream: &TcpStream) -> Result<Option<Request>, u16> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let mut total = reader.read_line(&mut line).map_err(|_| 400u16)?;
    if total == 0 {
        return Ok(None);
    }
    let mut parts = line.trim_end().split(' ');
    let (method, target) = match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some(m), Some(t), Some(v), None) if v.starts_with("HTTP/1.") && !m.is_empty() => {
            (m.to_string(), t.to_string())
        }
        _ => return Err(400),
    };
    let mut headers = Vec::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line).map_err(|_| 400u16)?;
        if n == 0 {
            return Err(400);
        }
        total += n;
        if total > MAX_HEAD_BYTES {
            return Err(431);
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        let (name, value) = trimmed.split_once(':').ok_or(400u16)?;
        headers.push((name.trim().to_string(), value.trim().to_string()));
    }
    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (target, String::new()),
    };
    let mut request = Request {
        method,
        path,
        query,
        headers,
        body: Vec::new(),
    };
    if request.header("Transfer-Encoding").is_some() {
        return Err(411);
    }
    let length: usize = match request.header("Content-Length") {
        Some(v) => v.trim().parse().map_err(|_| 400u16)?,
        None => 0,
    };
    if length > MAX_BODY_BYTES {
        return Err(413);
    }
    request.body = vec![0; length];
    reader.read_exact(&mut request.body).map_err(|_| 400u16)?;
    Ok(Some(request))
}

/// After an error answered before the body was read: closing at once would reset the
/// connection while the client is still sending, and it would never see the status.
/// Half-close, then discard what arrives until the client closes, within limits.
fn drain(stream: &mut TcpStream) -> io::Result<()> {
    stream.shutdown(Shutdown::Write)?;
    stream.set_read_timeout(Some(Duration::from_secs(1)))?;
    let mut sink = [0u8; 8192];
    let mut left = 16 * MAX_BODY_BYTES;
    while left > 0 {
        match stream.read(&mut sink) {
            Ok(0) | Err(_) => break,
            Ok(n) => left = left.saturating_sub(n),
        }
    }
    Ok(())
}

fn write_response(stream: &mut TcpStream, response: &Response) -> io::Result<()> {
    let Body::Bytes(body) = &response.body else {
        unreachable!("event streams are written by serve_events");
    };
    let mut out = head(response, &format!("Content-Length: {}", body.len()));
    out.extend_from_slice(body);
    stream.write_all(&out)?;
    stream.flush()
}

/// Status line and headers, `framing` added, ending with the blank line.
fn head(response: &Response, framing: &str) -> Vec<u8> {
    let mut out = format!(
        "HTTP/1.1 {} {}\r\n",
        response.status,
        reason(response.status)
    );
    for (name, value) in &response.headers {
        out.push_str(&format!("{name}: {value}\r\n"));
    }
    out.push_str(framing);
    out.push_str("\r\nConnection: close\r\n\r\n");
    out.into_bytes()
}

fn serve_events(app: &Arc<App>, mut stream: TcpStream, response: &Response) -> io::Result<()> {
    let (sender, events) = app.events.subscribe();
    let mut watcher = stream.try_clone()?;
    thread::spawn(move || {
        let mut sink = [0u8; 1024];
        while matches!(watcher.read(&mut sink), Ok(n) if n > 0) {}
        sender.send(Signal::Gone).ok();
    });
    let result = stream_events(app, &mut stream, response, events);
    stream.shutdown(Shutdown::Both).ok();
    result
}

fn stream_events(
    app: &App,
    stream: &mut TcpStream,
    response: &Response,
    events: Receiver<Signal>,
) -> io::Result<()> {
    stream.write_all(&head(response, "Transfer-Encoding: chunked"))?;
    let status = format!("event: status\ndata: {}\n\n", app.status());
    write_chunk(stream, &status)?;
    loop {
        match events.recv_timeout(KEEPALIVE) {
            Ok(Signal::Event(text)) => write_chunk(stream, &text)?,
            Ok(Signal::Gone) | Err(RecvTimeoutError::Disconnected) => return Ok(()),
            Err(RecvTimeoutError::Timeout) => write_chunk(stream, ": keepalive\n\n")?,
        }
    }
}

fn write_chunk(stream: &mut TcpStream, text: &str) -> io::Result<()> {
    let mut out = format!("{:x}\r\n", text.len()).into_bytes();
    out.extend_from_slice(text.as_bytes());
    out.extend_from_slice(b"\r\n");
    stream.write_all(&out)?;
    stream.flush()
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        202 => "Accepted",
        204 => "No Content",
        303 => "See Other",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        411 => "Length Required",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        431 => "Request Header Fields Too Large",
        500 => "Internal Server Error",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_long_random_and_compared_exactly() {
        let (a, b) = (new_token(), new_token());
        assert_eq!(a.len(), 64);
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
        assert!(same_secret(&a, &a.clone()));
        assert!(!same_secret(&a, &b));
        assert!(!same_secret(&a, &a[..63]));
        assert!(!same_secret(&a, ""));
    }

    #[test]
    fn the_token_is_read_from_this_ports_cookie_only() {
        let app = App::new(8080, false);
        let with = |cookie: &str| Request {
            method: "GET".into(),
            path: "/api/status".into(),
            query: String::new(),
            headers: vec![("Cookie".into(), cookie.into())],
            body: Vec::new(),
        };
        let token = &app.token;
        assert!(has_token(&app, &with(&format!("hoardmap-8080={token}"))));
        assert!(has_token(
            &app,
            &with(&format!("a=1; hoardmap-8080={token}; b=2"))
        ));
        assert!(!has_token(&app, &with(&format!("hoardmap-8081={token}"))));
        assert!(!has_token(&app, &with("hoardmap-8080=nope")));
        assert!(!has_token(&app, &with("")));
    }

    #[test]
    fn host_must_be_loopback_on_the_bound_port() {
        assert!(host_allowed(Some("127.0.0.1:8080"), 8080));
        assert!(host_allowed(Some("localhost:8080"), 8080));
        assert!(host_allowed(Some("127.0.0.1"), 8080));
        assert!(host_allowed(Some("localhost"), 8080));
        assert!(!host_allowed(Some("127.0.0.1:8081"), 8080));
        assert!(!host_allowed(Some("example.com:8080"), 8080));
        assert!(!host_allowed(Some("127.0.0.1.example.com:8080"), 8080));
        assert!(!host_allowed(None, 8080));
    }
}
