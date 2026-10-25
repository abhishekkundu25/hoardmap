//! The HTTP server on 127.0.0.1: serves the page, routes `/api/*` to [`crate::api`],
//! counts connected browser tabs and exits the process once the last tab has been
//! gone for the linger period.
//!
//! HTTP/1.1 is spoken directly over `std::net`: one thread per connection, one
//! response per connection (`Connection: close`). An event stream owns its socket, so a
//! blocking read on a clone of it notices the tab closing at once; nothing polls.

use std::convert::Infallible;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, Shutdown, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::json;

use crate::api;
use crate::cli::Options;
use crate::tree::Tree;

const INDEX_HTML: &str = include_str!("../web/index.html");
const APP_JS: &str = include_str!("../web/app.js");
const STYLE_CSS: &str = include_str!("../web/style.css");

/// A request line plus headers longer than this is refused with 431.
const MAX_HEAD_BYTES: usize = 16 * 1024;
/// A request body longer than this is refused with 413.
pub const MAX_BODY_BYTES: usize = 64 * 1024;
const KEEPALIVE: Duration = Duration::from_secs(15);

/// What the server knows about the scan.
#[derive(Debug)]
pub enum State {
    Idle,
    Scanning {
        path: PathBuf,
        started: Instant,
    },
    Done {
        path: PathBuf,
        tree: Arc<Tree>,
        duration: Duration,
    },
    Failed {
        path: PathBuf,
        error: String,
    },
}

/// Shared by every connection thread.
pub struct App {
    pub state: Mutex<State>,
    pub presence: Arc<Presence>,
    pub events: Broadcaster,
}

impl App {
    pub fn new(linger: Duration) -> App {
        App {
            state: Mutex::new(State::Idle),
            presence: Arc::new(Presence::new(linger)),
            events: Broadcaster::default(),
        }
    }

    /// The `/api/status` document for the current state.
    pub fn status(&self) -> serde_json::Value {
        let lossy = |p: &PathBuf| p.to_string_lossy().into_owned();
        match &*self.state.lock().unwrap() {
            State::Idle => status_json("idle", None, (0, 0, 0), 0, 0, None),
            State::Scanning { path, started } => status_json(
                "scanning",
                Some(lossy(path)),
                (0, 0, 0),
                started.elapsed().as_millis(),
                0,
                None,
            ),
            State::Done {
                path,
                tree,
                duration,
            } => {
                let root = tree.root();
                status_json(
                    "done",
                    Some(lossy(path)),
                    (root.dirs, root.files, root.size),
                    duration.as_millis(),
                    tree.error_count,
                    None,
                )
            }
            State::Failed { path, error } => {
                status_json("failed", Some(lossy(path)), (0, 0, 0), 0, 0, Some(error))
            }
        }
    }

    /// Sends the current status to every open event stream.
    pub fn notify_status(&self) {
        self.events.send("status", &self.status().to_string());
    }
}

fn status_json(
    state: &str,
    path: Option<String>,
    (dirs, files, bytes): (u32, u32, u64),
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
        "elapsed_ms": elapsed_ms as u64,
        "error_count": error_count,
        "error": error,
    })
}

/// Connected tabs, and the exit rule: linger after the last one leaves.
pub struct Presence {
    tabs: Mutex<Tabs>,
    linger: Duration,
}

struct Tabs {
    connected: usize,
    /// Bumped on every connect, so a timer armed before a reconnect does nothing.
    generation: u64,
}

/// One connected tab; dropping it is the disconnect.
pub struct Tab {
    presence: Arc<Presence>,
}

impl Presence {
    pub fn new(linger: Duration) -> Presence {
        Presence {
            tabs: Mutex::new(Tabs {
                connected: 0,
                generation: 0,
            }),
            linger,
        }
    }

    pub fn connect(self: &Arc<Self>) -> Tab {
        let mut tabs = self.tabs.lock().unwrap();
        tabs.connected += 1;
        tabs.generation += 1;
        Tab {
            presence: self.clone(),
        }
    }

    pub fn connected(&self) -> usize {
        self.tabs.lock().unwrap().connected
    }
}

impl Drop for Tab {
    fn drop(&mut self) {
        let generation = {
            let mut tabs = self.presence.tabs.lock().unwrap();
            tabs.connected -= 1;
            if tabs.connected > 0 {
                return;
            }
            tabs.generation
        };
        let presence = self.presence.clone();
        thread::spawn(move || {
            thread::sleep(presence.linger);
            let tabs = presence.tabs.lock().unwrap();
            if tabs.connected == 0 && tabs.generation == generation {
                std::process::exit(0);
            }
        });
    }
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

/// Binds, prints the listening line, launches the browser and serves until the linger
/// timer exits the process. Returns only when binding fails.
pub fn run(options: Options) -> io::Result<Infallible> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, options.port))?;
    let url = format!("http://127.0.0.1:{}/", listener.local_addr()?.port());
    println!("Listening on {url}");
    io::stdout().flush()?;
    if options.open {
        launch_browser(&url);
    }
    let app = Arc::new(App::new(options.linger));
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                let app = app.clone();
                thread::spawn(move || serve(&app, stream).ok());
            }
            Err(error) => {
                eprintln!("macdirtree: accept failed: {error}");
                thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

fn launch_browser(url: &str) {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    match Command::new(program).arg(url).spawn() {
        Ok(mut child) => {
            thread::spawn(move || child.wait().ok());
        }
        Err(error) => eprintln!("macdirtree: could not run {program}: {error}"),
    }
}

fn serve(app: &Arc<App>, mut stream: TcpStream) -> io::Result<()> {
    let request = match read_request(&stream) {
        Ok(Some(request)) => request,
        Ok(None) => return Ok(()),
        Err(status) => return write_response(&mut stream, &Response::text(status, reason(status))),
    };
    let response = route(app, &request);
    match response.body {
        Body::EventStream => serve_events(app, stream, &response),
        Body::Bytes(_) => write_response(&mut stream, &response),
    }
}

fn route(app: &App, request: &Request) -> Response {
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
        _ => Response::text(404, "not found\n"),
    }
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
    let _tab = app.presence.connect();
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
        400 => "Bad Request",
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
