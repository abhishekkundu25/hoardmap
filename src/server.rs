//! The HTTP server on 127.0.0.1 behind the app's window: serves the page and routes
//! `/api/*` to [`crate::api`]. Only the page the app loads holds the launch token.
//!
//! HTTP/1.1 is spoken directly over `std::net`: one thread per connection, one
//! response per connection (`Connection: close`). An event stream owns its socket, so a
//! blocking read on a clone of it notices the tab closing at once; nothing polls.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
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
/// General slots, including event streams; authenticated overflow is capped separately.
pub const MAX_CONNECTIONS: usize = 64;
/// Absolute budget for the request head and body together.
pub const READ_TIMEOUT: Duration = Duration::from_secs(2);
const DRAIN_TIMEOUT: Duration = Duration::from_secs(1);
const RESERVED_CONNECTIONS: usize = 8;
const ADMISSION_TIMEOUT: Duration = Duration::from_millis(50);
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
    /// (Last folder request, whether the first scan has been requested.)
    folder_request: Mutex<(usize, bool)>,
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
            folder_request: Mutex::new((0, false)),
        }
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

    fn subscribe_status(&self) -> (Sender<Signal>, Receiver<Signal>) {
        let state = self.state.lock().unwrap();
        self.events.subscribe(format!(
            "event: status\ndata: {}\n\n",
            self.status_of(&state)
        ))
    }

    /// Starts scanning `path` on its own thread, with a ticker that sends the status
    /// every 250 ms until the scan ends. Refused while a scan runs or when `path` is not
    /// a directory. Without a path, uses the current folder's raw path.
    pub fn start_scan(self: &Arc<App>, path: Option<PathBuf>) -> Result<(), String> {
        let request = self.next_folder_request(true);
        self.start_scan_for(path, request, false)
    }

    fn next_folder_request(&self, scan: bool) -> usize {
        let mut request = self.folder_request.lock().unwrap();
        request.0 += 1;
        request.1 |= scan;
        request.0
    }

    fn start_scan_for(
        self: &Arc<App>,
        path: Option<PathBuf>,
        request: usize,
        replace: bool,
    ) -> Result<(), String> {
        let path = path
            .or_else(|| match &*self.state.lock().unwrap() {
                State::Idle => self.start.lock().unwrap().clone(),
                State::Scanning { path, .. }
                | State::Done { path, .. }
                | State::Failed { path, .. } => Some(path.clone()),
            })
            .ok_or("no folder to scan")?;
        let progress = Arc::new(Progress::default());
        let started = Instant::now();
        if !path.is_dir() {
            return Err(format!("{} is not a directory", path.display()));
        }
        let disk = disk(&path);
        {
            let mut state = self.state.lock().unwrap();
            let current = self.folder_request.lock().unwrap();
            if request != current.0 {
                return Ok(());
            }
            if let State::Scanning { progress, .. } = &*state {
                if !replace {
                    return Err(SCAN_RUNNING.to_string());
                }
                progress.stop.store(true, AtomicOrdering::Relaxed);
            }
            *self.disk.lock().unwrap() = disk;
            *state = State::Scanning {
                path: path.clone(),
                progress: progress.clone(),
                started,
                live: None,
            };
        }
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
        let disk = disk(&path_for_disk);
        self.update_scan(progress, |state| {
            *self.disk.lock().unwrap() = disk;
            *state = next;
        });
    }

    /// Updates only the current scan, including any disk totals belonging to it.
    fn update_scan(&self, progress: &Arc<Progress>, update: impl FnOnce(&mut State)) -> bool {
        {
            let mut state = self.state.lock().unwrap();
            if !matches!(&*state, State::Scanning { progress: p, .. } if Arc::ptr_eq(p, progress)) {
                return false;
            }
            update(&mut state);
        }
        self.notify_status();
        true
    }

    /// A folder handed over by Finder or the `hoardmap` command. Before the first scan
    /// (the start screen is up) it becomes the folder that scan reads; otherwise it
    /// replaces the current scan, stopping a running one first. Call off the event
    /// loop because folder validation may block.
    pub fn open_folder(self: &Arc<App>, path: PathBuf) -> Result<(), String> {
        let request = self.next_folder_request(false);
        if !path.is_dir() {
            return Err(format!("{} is not a directory", path.display()));
        }
        {
            let state = self.state.lock().unwrap();
            let mut current = self.folder_request.lock().unwrap();
            if request != current.0 {
                return Ok(());
            }
            if matches!(*state, State::Idle) && !current.1 {
                *self.start.lock().unwrap() = Some(path);
                drop(current);
                drop(state);
                self.notify_status();
                return Ok(());
            }
            current.1 = true;
        }
        self.start_scan_for(Some(path), request, true)
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
            let next = State::Done {
                path,
                colours: Colours::new(&tree),
                tree: Arc::new(tree),
                duration: started.elapsed(),
                stopped: true,
                preview: true,
            };
            app.update_scan(&progress, |state| *state = next);
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
                self.update_scan(progress, |state| *state = gone(path));
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

    fn subscribe(&self, initial: String) -> (Sender<Signal>, Receiver<Signal>) {
        let (tx, rx) = mpsc::channel();
        tx.send(Signal::Event(initial)).unwrap();
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

struct Connection(Arc<AtomicUsize>);

struct Deadline<'a> {
    stream: &'a TcpStream,
    until: Instant,
}

impl Read for Deadline<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let remaining = self
            .until
            .checked_duration_since(Instant::now())
            .filter(|duration| !duration.is_zero())
            .ok_or_else(|| io::Error::from(io::ErrorKind::TimedOut))?;
        self.stream.set_read_timeout(Some(remaining))?;
        self.stream.read(buf)
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.0.fetch_sub(1, AtomicOrdering::Relaxed);
    }
}

/// Binds 127.0.0.1, starts the scan of `config.path` and serves on a background
/// thread.
pub fn spawn(config: Config) -> io::Result<Spawned> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, config.port))
        .or_else(|_| TcpListener::bind((Ipv4Addr::LOCALHOST, 0)))?;
    let port = listener.local_addr()?.port();
    let app = App::new(port, config.native);
    *app.start.lock().unwrap() = Some(config.path.clone());
    *app.disk.lock().unwrap() = disk(&config.path);
    let url = page_url(port, &app.token);
    let app = Arc::new(app);
    if config.scan_at_start
        && let Err(error) = app.start_scan(Some(config.path.clone()))
    {
        eprintln!("hoardmap: {error}");
        *app.state.lock().unwrap() = State::Failed {
            path: config.path,
            error,
        };
    }
    let served = app.clone();
    thread::Builder::new()
        .name("http-listener".into())
        .spawn(move || {
            let active = Arc::new(AtomicUsize::new(0));
            let reserved = Arc::new(AtomicUsize::new(0));
            loop {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let until = Instant::now() + READ_TIMEOUT;
                        let overflow = active.load(AtomicOrdering::Relaxed) >= MAX_CONNECTIONS;
                        let slots = if overflow { &reserved } else { &active };
                        if overflow
                            && reserved.load(AtomicOrdering::Relaxed) >= RESERVED_CONNECTIONS
                        {
                            continue;
                        }
                        slots.fetch_add(1, AtomicOrdering::Relaxed);
                        let connection = Connection(slots.clone());
                        let app = app.clone();
                        // The overflow check reads on this thread, so a silent client never
                        // holds up `accept`.
                        if let Err(error) = thread::Builder::new()
                            .name("http-request".into())
                            .spawn(move || {
                                let _connection = connection;
                                let prepared = if overflow {
                                    // shortcut: overflow requests have 50 ms; use readiness-based admission if local fragmentation needs longer.
                                    let Ok(Some(request)) = read_request(Deadline {
                                        stream: &stream,
                                        until: Instant::now() + ADMISSION_TIMEOUT,
                                    }) else {
                                        return;
                                    };
                                    if !has_token(&app, &request)
                                        && !launch_token(&request)
                                            .is_some_and(|token| same_secret(token, &app.token))
                                    {
                                        return;
                                    }
                                    Some(request)
                                } else {
                                    None
                                };
                                serve(&app, stream, until, prepared).ok();
                            })
                        {
                            eprintln!("hoardmap: connection thread failed: {error}");
                        }
                    }
                    Err(error) => {
                        eprintln!("hoardmap: accept failed: {error}");
                        thread::sleep(Duration::from_millis(100));
                    }
                }
            }
        })?;
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

fn launch_token(request: &Request) -> Option<&str> {
    if request.path != "/" {
        return None;
    }
    request
        .query
        .split('&')
        .find_map(|part| part.strip_prefix("token="))
}

fn serve(
    app: &Arc<App>,
    mut stream: TcpStream,
    until: Instant,
    prepared: Option<Request>,
) -> io::Result<()> {
    stream.set_write_timeout(Some(READ_TIMEOUT))?;
    let request = match prepared {
        Some(request) => Ok(Some(request)),
        None => read_request(Deadline {
            stream: &stream,
            until,
        }),
    };
    let request = match request {
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
    if let Some(given) = launch_token(request) {
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
fn read_request(stream: impl Read) -> Result<Option<Request>, u16> {
    // Limit the source before buffering, including BufReader's read-ahead.
    let mut reader = BufReader::new(stream.take(MAX_HEAD_BYTES as u64));
    let mut line = String::new();
    let mut total = reader.read_line(&mut line).map_err(|_| 400u16)?;
    if total == 0 {
        return Ok(None);
    }
    if !line.ends_with('\n') {
        return Err(if total == MAX_HEAD_BYTES { 431 } else { 400 });
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
        total += n;
        if n == 0 || !line.ends_with('\n') {
            return Err(if total == MAX_HEAD_BYTES { 431 } else { 400 });
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
    reader.get_mut().set_limit(length as u64);
    reader.read_exact(&mut request.body).map_err(|_| 400u16)?;
    Ok(Some(request))
}

/// After an error answered before the body was read: closing at once would reset the
/// connection while the client is still sending, and it would never see the status.
/// Half-close, then discard what arrives until the client closes, within limits.
fn drain(stream: &mut TcpStream) -> io::Result<()> {
    stream.shutdown(Shutdown::Write)?;
    let mut reader = Deadline {
        stream,
        until: Instant::now() + DRAIN_TIMEOUT,
    }
    .take((16 * MAX_BODY_BYTES) as u64);
    io::copy(&mut reader, &mut io::sink()).ok();
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
    let (sender, events) = app.subscribe_status();
    stream.set_read_timeout(None)?;
    let mut watcher = stream.try_clone()?;
    thread::Builder::new()
        .name("http-events".into())
        .spawn(move || {
            let mut sink = [0u8; 1024];
            while matches!(watcher.read(&mut sink), Ok(n) if n > 0) {}
            sender.send(Signal::Gone).ok();
        })?;
    let result = stream_events(&mut stream, response, events);
    stream.shutdown(Shutdown::Both).ok();
    result
}

fn stream_events(
    stream: &mut TcpStream,
    response: &Response,
    events: Receiver<Signal>,
) -> io::Result<()> {
    stream.write_all(&head(response, "Transfer-Encoding: chunked"))?;
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
    fn a_default_scan_uses_the_idle_start_folder_or_reports_none() {
        let app = Arc::new(App::new(0, false));
        assert_eq!(app.start_scan(None).unwrap_err(), "no folder to scan");
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        *app.start.lock().unwrap() = Some(path.clone());
        app.start_scan(None).unwrap();
        assert_eq!(app.status()["path"], lossy(&path));
    }

    #[test]
    fn a_later_folder_request_wins_while_the_first_scan_is_being_prepared() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        for finder in [true, false] {
            let app = Arc::new(App::new(0, false));
            *app.start.lock().unwrap() = Some(root.join("web"));
            // HTTP scan A has registered and is paused before filesystem validation.
            let first = app.next_folder_request(true);
            if finder {
                app.open_folder(root.join("assets")).unwrap();
            } else {
                app.start_scan(Some(root.join("assets"))).unwrap();
            }
            let end = Instant::now() + Duration::from_secs(5);
            while app.status()["state"] != "done" {
                assert!(Instant::now() < end);
                thread::sleep(Duration::from_millis(10));
            }
            app.start_scan_for(Some(root.join("web")), first, false)
                .unwrap();
            assert_eq!(app.status()["path"], lossy(&root.join("assets")));
        }
    }

    #[test]
    fn trickling_during_drain_cannot_extend_its_deadline() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let mut peer = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (mut stream, _) = listener.accept().unwrap();
        let started = Instant::now();
        let writer = thread::spawn(move || {
            while started.elapsed() < Duration::from_secs(2) {
                if peer.write_all(b"x").is_err() {
                    break;
                }
                thread::sleep(Duration::from_millis(50));
            }
        });
        drain(&mut stream).unwrap();
        let elapsed = started.elapsed();
        drop(stream);
        writer.join().unwrap();
        assert!(
            elapsed < Duration::from_millis(1500),
            "drained for {elapsed:?}"
        );
    }

    #[test]
    fn request_head_reads_stop_at_the_limit_before_buffering() {
        for prefix in ["GET /", "GET / HTTP/1.1\r\nX: "] {
            let mut input = io::Cursor::new(format!("{prefix}{}", "x".repeat(1024 * 1024)));
            assert!(matches!(read_request(&mut input), Err(431)));
            assert_eq!(input.position(), MAX_HEAD_BYTES as u64);
        }
        let mut input = io::Cursor::new(format!(
            "GET / HTTP/1.1\r\n{}",
            "X: short\r\n".repeat(MAX_HEAD_BYTES)
        ));
        assert!(matches!(read_request(&mut input), Err(431)));
        assert_eq!(input.position(), MAX_HEAD_BYTES as u64);
    }

    #[test]
    fn an_exact_limit_head_still_reads_its_body_and_truncated_heads_fail() {
        let prefix = "POST / HTTP/1.1\r\nContent-Length: 4\r\nX: ";
        let padding = "x".repeat(MAX_HEAD_BYTES - prefix.len() - 4);
        let request = format!("{prefix}{padding}\r\n\r\nbody");
        assert_eq!(
            read_request(request.as_bytes()).unwrap().unwrap().body,
            b"body"
        );
        let oversized = format!("{prefix}{padding}x\r\n\r\nbody");
        assert!(matches!(read_request(oversized.as_bytes()), Err(431)));
        for head in [
            "GET / HTTP/1.1",
            "GET / HTTP/1.1\r\n",
            "GET / HTTP/1.1\r\nX: a",
        ] {
            assert!(
                matches!(read_request(head.as_bytes()), Err(400)),
                "{head:?}"
            );
        }
    }

    #[test]
    fn a_stale_scan_finishes_without_changing_the_current_disk_or_state() {
        let app = App::new(0, false);
        let old = Arc::new(Progress::default());
        old.stop.store(true, AtomicOrdering::Relaxed);
        let current = Arc::new(Progress::default());
        *app.state.lock().unwrap() = State::Scanning {
            path: PathBuf::from("replacement"),
            progress: current.clone(),
            started: Instant::now(),
            live: None,
        };
        *app.disk.lock().unwrap() = Some((123, 45));
        app.finish_scan(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("assets"),
            &old,
            Instant::now(),
        );
        assert_eq!(*app.disk.lock().unwrap(), Some((123, 45)));
        assert!(
            matches!(&*app.state.lock().unwrap(), State::Scanning { progress, .. } if Arc::ptr_eq(progress, &current))
        );
    }

    #[test]
    fn folder_validation_does_not_wait_for_the_state_lock() {
        let app = Arc::new(App::new(0, false));
        let mut state = app.state.lock().unwrap();
        *state = State::Scanning {
            path: PathBuf::from("current"),
            progress: Arc::new(Progress::default()),
            started: Instant::now(),
            live: None,
        };
        let (tx, rx) = mpsc::channel();
        let starting = app.clone();
        let worker = thread::spawn(move || {
            tx.send(starting.start_scan(Some(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"),
            )))
            .unwrap();
        });
        let result = rx.recv_timeout(Duration::from_secs(2));
        drop(state);
        worker.join().unwrap();
        assert!(matches!(result, Ok(Err(error)) if error.ends_with("is not a directory")));
    }

    #[test]
    fn event_snapshots_precede_later_statuses_under_the_state_lock() {
        let app = Arc::new(App::new(0, false));
        let mut state = app.state.lock().unwrap();
        let (tx, rx) = mpsc::channel();
        let subscribing = app.clone();
        let subscriber = thread::spawn(move || tx.send(subscribing.subscribe_status()).unwrap());
        assert!(matches!(
            rx.recv_timeout(Duration::from_millis(50)),
            Err(RecvTimeoutError::Timeout)
        ));
        *state = State::Failed {
            path: PathBuf::from("first"),
            error: "first".into(),
        };
        drop(state);
        let (_sender, events) = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        subscriber.join().unwrap();
        *app.state.lock().unwrap() = State::Failed {
            path: PathBuf::from("second"),
            error: "second".into(),
        };
        app.notify_status();
        for expected in ["first", "second"] {
            let Signal::Event(text) = events.recv().unwrap() else {
                panic!("stream closed")
            };
            let data = text.strip_prefix("event: status\ndata: ").unwrap().trim();
            let status: serde_json::Value = serde_json::from_str(data).unwrap();
            assert_eq!(status["path"], expected);
        }
    }

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
