//! JSON endpoints over the scan state: status and events, nodes, treemap rectangles,
//! extension statistics, largest files, and opening a node in Finder.
//!
//! Every error body is `{"error": "..."}`. Endpoints that read the tree answer 409
//! until a scan has finished; a POST needs `Content-Type: application/json` (415) and a
//! valid JSON body (400).

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::thread;

use serde_json::{Value, json};

use crate::access;
use crate::icicle;
use crate::server::{App, Request, Response, SCAN_RUNNING, State};
use crate::tree::{Kind, Node, NodeId, Tree};
use crate::treemap::{self, Colours, Mode};
use crate::volumes;

const DEFAULT_LARGEST: usize = 100;
const MAX_LARGEST: usize = 1000;

/// Answers a request whose path starts with `/api/`.
pub fn handle(app: &Arc<App>, request: &Request) -> Response {
    let method = request.method.as_str();
    if let Some(id) = request.path.strip_prefix("/api/node/") {
        return match method {
            "GET" => node(app, id),
            _ => method_not_allowed("GET"),
        };
    }
    match (method, request.path.as_str()) {
        ("GET", "/api/status") => Response::json(200, &app.status()),
        ("GET", "/api/events") => Response::event_stream(),
        ("GET", "/api/treemap") => treemap(app, &Query::parse(&request.query)),
        ("GET", "/api/icicle") => icicle(app, &Query::parse(&request.query)),
        ("GET", "/api/extensions") => extensions(app, &Query::parse(&request.query)),
        ("GET", "/api/largest") => largest(app, &Query::parse(&request.query)),
        ("GET", "/api/errors") => errors(app),
        ("POST", "/api/scan") => json_body(request).map_or_else(|r| r, |body| scan(app, &body)),
        ("POST", "/api/scan/stop") => json_body(request).map_or_else(
            |r| r,
            |_| match app.stop_scan() {
                Ok(()) => Response::json(202, &app.status()),
                Err(e) => error(409, &e),
            },
        ),
        ("POST", "/api/open") => json_body(request).map_or_else(|r| r, |body| open(app, &body)),
        ("GET", "/api/access") => access_status(app),
        ("GET", "/api/volumes") => volumes(),
        ("POST", "/api/choose") => json_body(request).map_or_else(
            |r| r,
            |_| match volumes::choose_folder() {
                Some(path) => Response::json(200, &json!({ "path": lossy(&path) })),
                None => no_content(),
            },
        ),
        ("POST", "/api/access/settings") => {
            json_body(request).map_or_else(|r| r, |_| ran(access::open_settings()))
        }
        ("POST", "/api/access/reveal") if app.native => {
            json_body(request).map_or_else(|r| r, |_| ran(access::reveal_app()))
        }
        ("POST", "/api/relaunch") if app.native => {
            json_body(request).map_or_else(|r| r, |_| ran(access::relaunch()))
        }
        (
            _,
            "/api/status" | "/api/events" | "/api/treemap" | "/api/icicle" | "/api/extensions"
            | "/api/largest" | "/api/errors",
        ) => method_not_allowed("GET"),
        (_, "/api/access" | "/api/volumes") => method_not_allowed("GET"),
        (_, "/api/choose") => method_not_allowed("POST"),
        (_, "/api/scan" | "/api/scan/stop" | "/api/open" | "/api/access/settings") => {
            method_not_allowed("POST")
        }
        _ => error(404, "not found"),
    }
}

fn error(status: u16, message: &str) -> Response {
    Response::json(status, &json!({ "error": message }))
}

fn method_not_allowed(allow: &str) -> Response {
    error(405, "method not allowed").with_header("Allow", allow)
}

/// The finished tree and its colour table, or a running scan's preview of them, or
/// the 409 to answer with.
fn tree(app: &App) -> Result<(Arc<Tree>, Colours), Response> {
    match &*app.state.lock().unwrap() {
        State::Done { tree, colours, .. } => Ok((tree.clone(), colours.clone())),
        State::Scanning {
            live: Some(live), ..
        } => Ok((live.tree.clone(), live.colours.clone())),
        _ => Err(error(409, "no tree loaded")),
    }
}

/// The parsed JSON object of a POST, or the 415 or 400 to answer with.
fn json_body(request: &Request) -> Result<Value, Response> {
    let is_json = request
        .header("Content-Type")
        .is_some_and(|t| t.trim_start().starts_with("application/json"));
    if !is_json {
        return Err(error(415, "Content-Type must be application/json"));
    }
    if request.body.is_empty() {
        return Err(error(400, "a JSON body is required"));
    }
    match serde_json::from_slice::<Value>(&request.body) {
        Ok(value) if value.is_object() => Ok(value),
        Ok(_) => Err(error(400, "the body must be a JSON object")),
        Err(e) => Err(error(400, &format!("invalid JSON: {e}"))),
    }
}

/// `name=value` pairs of a query string. Values carry only numbers and mode words, so
/// there is no percent-decoding.
struct Query<'a>(Vec<(&'a str, &'a str)>);

impl<'a> Query<'a> {
    fn parse(query: &'a str) -> Query<'a> {
        Query(
            query
                .split('&')
                .filter(|p| !p.is_empty())
                .map(|p| p.split_once('=').unwrap_or((p, "")))
                .collect(),
        )
    }

    fn get(&self, name: &str) -> Option<&'a str> {
        self.0.iter().find(|(k, _)| *k == name).map(|(_, v)| *v)
    }

    fn root(&self) -> Result<NodeId, Response> {
        match self.get("root") {
            None | Some("") => Ok(crate::tree::ROOT),
            Some(v) => v.parse().map_err(|_| error(400, "root must be a node id")),
        }
    }

    fn mode(&self) -> Result<Mode, Response> {
        match self.get("mode") {
            None | Some("") => Ok(Mode::Logical),
            Some(v) => Mode::parse(v).ok_or_else(|| error(400, "mode must be logical or physical")),
        }
    }

    /// A required finite number above zero.
    fn pixels(&self, name: &str) -> Result<f32, Response> {
        self.get(name)
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|v| v.is_finite() && *v > 0.0)
            .ok_or_else(|| error(400, &format!("{name} must be a positive number of pixels")))
    }
}

fn node_json(id: NodeId, node: &Node) -> Value {
    json!({
        "id": id,
        "name": node.name,
        "parent": node.parent,
        "kind": node.kind,
        "size": node.size,
        "physical": node.physical,
        "files": node.files,
        "dirs": node.dirs,
        "mtime": node.mtime,
        "hard_link": node.hard_link,
        "has_children": !node.children.is_empty(),
    })
}

pub(crate) fn lossy(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn node(app: &App, id: &str) -> Response {
    let (tree, _) = match tree(app) {
        Ok(t) => t,
        Err(r) => return r,
    };
    let Some((id, node)) = id
        .parse::<NodeId>()
        .ok()
        .and_then(|id| Some((id, tree.get(id)?)))
    else {
        return error(404, "no such node");
    };
    let mut json = node_json(id, node);
    json["path"] = Value::String(lossy(&tree.path(id)));
    json["children"] = node
        .children
        .iter()
        .map(|&c| node_json(c, &tree.nodes[c as usize]))
        .collect();
    Response::json(200, &json)
}

/// The viewport, minimum size, root and mode shared by the two layouts.
fn viewport(query: &Query) -> Result<(NodeId, f32, f32, f32, Mode), Response> {
    let w = query.pixels("w")?;
    let h = query.pixels("h")?;
    let min = match query.get("min") {
        None | Some("") => 1.0,
        Some(v) => v
            .parse::<f32>()
            .ok()
            .filter(|m| m.is_finite())
            .ok_or_else(|| error(400, "min must be a number of pixels"))?
            .clamp(0.5, 100.0),
    };
    Ok((query.root()?, w, h, min, query.mode()?))
}

fn treemap(app: &App, query: &Query) -> Response {
    let (tree, colours) = match tree(app) {
        Ok(t) => t,
        Err(r) => return r,
    };
    let (root, w, h, min, mode) = match viewport(query) {
        Ok(p) => p,
        Err(r) => return r,
    };
    if tree.get(root).is_none() {
        return error(404, "no such node");
    }
    Response::json(
        200,
        &treemap::layout(&tree, root, w, h, min, mode, &colours),
    )
}

fn icicle(app: &App, query: &Query) -> Response {
    let (tree, colours) = match tree(app) {
        Ok(t) => t,
        Err(r) => return r,
    };
    let (root, w, h, min, mode) = match viewport(query) {
        Ok(p) => p,
        Err(r) => return r,
    };
    if tree.get(root).is_none() {
        return error(404, "no such node");
    }
    Response::json(200, &icicle::icicle(&tree, root, w, h, min, mode, &colours))
}

fn extensions(app: &App, query: &Query) -> Response {
    let (tree, colours) = match tree(app) {
        Ok(t) => t,
        Err(r) => return r,
    };
    let (root, mode) = match query.root().and_then(|r| Ok((r, query.mode()?))) {
        Ok(p) => p,
        Err(r) => return r,
    };
    if tree.get(root).is_none() {
        return error(404, "no such node");
    }
    let items = treemap::extensions(&tree, root, mode, &colours);
    let total: u64 = items
        .iter()
        .map(|s| match mode {
            Mode::Logical => s.bytes,
            Mode::Physical => s.physical,
        })
        .sum();
    Response::json(200, &json!({ "total": total, "items": items }))
}

fn largest(app: &App, query: &Query) -> Response {
    let (tree, _) = match tree(app) {
        Ok(t) => t,
        Err(r) => return r,
    };
    let parsed = (|| {
        let n = match query.get("n") {
            None | Some("") => DEFAULT_LARGEST,
            Some(v) => v
                .parse::<usize>()
                .map_err(|_| error(400, "n must be a whole number"))?
                .min(MAX_LARGEST),
        };
        Ok((query.root()?, n, query.mode()?))
    })();
    let (root, n, mode) = match parsed {
        Ok(p) => p,
        Err(r) => return r,
    };
    if tree.get(root).is_none() {
        return error(404, "no such node");
    }
    let items: Vec<Value> = treemap::largest(&tree, root, n, mode)
        .into_iter()
        .map(|id| {
            let node = &tree.nodes[id as usize];
            json!({
                "id": id,
                "path": lossy(&tree.path(id)),
                "size": node.size,
                "physical": node.physical,
                "mtime": node.mtime,
            })
        })
        .collect();
    Response::json(200, &json!({ "items": items }))
}

fn errors(app: &App) -> Response {
    match tree(app) {
        Ok((tree, _)) => Response::json(
            200,
            &json!({ "count": tree.error_count, "items": tree.errors }),
        ),
        Err(r) => r,
    }
}

fn scan(app: &Arc<App>, body: &Value) -> Response {
    let Some(path) = body["path"].as_str().filter(|p| !p.is_empty()) else {
        return error(400, "path must be a non-empty string");
    };
    // "/Users/me/" and "/Users/me" are one folder; the tree names its root without the slash.
    let path = match path.trim_end_matches('/') {
        "" => "/",
        trimmed => trimmed,
    };
    match app.start_scan(PathBuf::from(path)) {
        Ok(()) => Response::json(202, &app.status()),
        Err(e) if e == SCAN_RUNNING => error(409, &e),
        Err(e) => error(400, &e),
    }
}

/// The disks to offer, and the home folder.
fn volumes() -> Response {
    let home = std::env::var_os("HOME").map(|h| lossy(Path::new(&h)));
    Response::json(
        200,
        &json!({ "volumes": volumes::list(), "home": home, "chooser": cfg!(target_os = "macos") }),
    )
}

fn access_status(app: &App) -> Response {
    Response::json(
        200,
        &json!({
            "granted": access::granted(),
            "native": app.native,
            "app": access::bundle().map(|b| lossy(&b)),
            "installed": access::installed(),
        }),
    )
}

/// 204 when the command started, 500 with its error otherwise.
fn ran(result: io::Result<()>) -> Response {
    match result {
        Ok(()) => no_content(),
        Err(e) => error(500, &format!("could not run open: {e}")),
    }
}

fn no_content() -> Response {
    Response {
        status: 204,
        headers: vec![("Cache-Control".into(), "no-store".into())],
        body: crate::server::Body::Bytes(Vec::new()),
    }
}

fn open(app: &App, body: &Value) -> Response {
    let (tree, _) = match tree(app) {
        Ok(t) => t,
        Err(r) => return r,
    };
    let Some(id) = body["id"].as_u64().and_then(|id| NodeId::try_from(id).ok()) else {
        return error(400, "id must be a node id");
    };
    let Some(node) = tree.get(id) else {
        return error(404, "no such node");
    };
    // The path comes from the tree alone; a request names a node, never a path.
    ran(reveal(&tree.path(id), node.kind == Kind::Dir))
}

/// `open -R <path>` for a file and `open <path>` for a directory on macOS; `xdg-open`
/// on the directory (or the file's parent) elsewhere. The child is reaped on a thread.
fn reveal(path: &Path, is_dir: bool) -> io::Result<()> {
    let mut command = if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        if !is_dir {
            c.arg("-R");
        }
        c.arg(path);
        c
    } else {
        let mut c = Command::new("xdg-open");
        c.arg(if is_dir {
            path
        } else {
            path.parent().unwrap_or(path)
        });
        c
    };
    let mut child = command.spawn()?;
    thread::spawn(move || child.wait().ok());
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::server::Body;

    fn app() -> Arc<App> {
        Arc::new(App::new(Some(Duration::from_secs(1)), 0, false))
    }

    fn get(path: &str) -> Request {
        let (path, query) = path.split_once('?').unwrap_or((path, ""));
        Request {
            method: "GET".into(),
            path: path.into(),
            query: query.into(),
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    fn post(path: &str, content_type: Option<&str>, body: &str) -> Request {
        Request {
            method: "POST".into(),
            path: path.into(),
            query: String::new(),
            headers: content_type
                .map(|t| ("Content-Type".to_string(), t.to_string()))
                .into_iter()
                .collect(),
            body: body.as_bytes().to_vec(),
        }
    }

    fn body(response: &Response) -> Value {
        match &response.body {
            Body::Bytes(bytes) => serde_json::from_slice(bytes).unwrap(),
            Body::EventStream => panic!("not a JSON body"),
        }
    }

    #[test]
    fn status_is_idle_at_start() {
        let response = handle(&app(), &get("/api/status"));
        assert_eq!(response.status, 200);
        let json = body(&response);
        assert_eq!(json["state"], "idle");
        assert_eq!(json["bytes"], 0);
        assert_eq!(json["physical"], 0);
        assert!(json["path"].is_null());
    }

    #[test]
    fn events_is_a_stream_and_the_rest_is_404() {
        let app = app();
        assert!(matches!(
            handle(&app, &get("/api/events")).body,
            Body::EventStream
        ));
        let response = handle(&app, &get("/api/nope"));
        assert_eq!(response.status, 404);
        assert_eq!(body(&response)["error"], "not found");
    }

    #[test]
    fn tree_endpoints_are_409_without_a_tree() {
        let app = app();
        for path in [
            "/api/node/0",
            "/api/treemap?w=10&h=10",
            "/api/extensions",
            "/api/largest",
            "/api/errors",
        ] {
            assert_eq!(handle(&app, &get(path)).status, 409, "{path}");
        }
        let response = handle(
            &app,
            &post("/api/open", Some("application/json"), r#"{"id":0}"#),
        );
        assert_eq!(response.status, 409);
    }

    #[test]
    fn posts_need_json() {
        let app = app();
        assert_eq!(handle(&app, &post("/api/scan", None, "{}")).status, 415);
        assert_eq!(
            handle(&app, &post("/api/scan", Some("text/plain"), "{}")).status,
            415
        );
        assert_eq!(
            handle(&app, &post("/api/scan", Some("application/json"), "")).status,
            400
        );
        assert_eq!(
            handle(&app, &post("/api/scan", Some("application/json"), "{nope")).status,
            400
        );
        assert_eq!(
            handle(&app, &post("/api/scan", Some("application/json"), "[1]")).status,
            400
        );
        assert_eq!(
            handle(&app, &post("/api/scan", Some("application/json"), "{}")).status,
            400
        );
        let response = handle(&app, &get("/api/scan"));
        assert_eq!(response.status, 405);
        assert_eq!(
            response
                .headers
                .iter()
                .find(|(k, _)| k == "Allow")
                .map(|(_, v)| v.as_str()),
            Some("POST")
        );
    }

    #[test]
    fn query_parsing() {
        let q = Query::parse("root=3&w=400&h=&mode=Physical&flag");
        assert_eq!(q.root().unwrap(), 3);
        assert_eq!(q.pixels("w").unwrap(), 400.0);
        assert!(q.pixels("h").is_err());
        assert_eq!(q.mode().unwrap(), Mode::Physical);
        assert_eq!(q.get("flag"), Some(""));
        assert!(Query::parse("w=inf").pixels("w").is_err());
        assert!(Query::parse("w=-1").pixels("w").is_err());
        assert!(Query::parse("mode=bytes").mode().is_err());
        assert!(Query::parse("root=x").root().is_err());
        assert_eq!(Query::parse("").root().unwrap(), 0);
        assert_eq!(Query::parse("").mode().unwrap(), Mode::Logical);
    }
}
