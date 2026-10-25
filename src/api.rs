//! JSON endpoints over the scan state: status and events, nodes, treemap rectangles,
//! extension statistics, largest files, and opening a node in Finder.

use serde_json::json;

use crate::server::{App, Request, Response};

/// Answers a request whose path starts with `/api/`.
pub fn handle(app: &App, request: &Request) -> Response {
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/api/status") => Response::json(200, &app.status()),
        ("GET", "/api/events") => Response::event_stream(),
        _ => Response::json(404, &json!({ "error": "not found" })),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::server::Body;

    fn get(path: &str) -> Request {
        Request {
            method: "GET".into(),
            path: path.into(),
            query: String::new(),
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    fn body(response: &Response) -> serde_json::Value {
        match &response.body {
            Body::Bytes(bytes) => serde_json::from_slice(bytes).unwrap(),
            Body::EventStream => panic!("not a JSON body"),
        }
    }

    #[test]
    fn status_is_idle_at_start() {
        let app = App::new(Duration::from_secs(1));
        let response = handle(&app, &get("/api/status"));
        assert_eq!(response.status, 200);
        let json = body(&response);
        assert_eq!(json["state"], "idle");
        assert_eq!(json["bytes"], 0);
        assert!(json["path"].is_null());
    }

    #[test]
    fn events_is_a_stream_and_the_rest_is_404() {
        let app = App::new(Duration::from_secs(1));
        assert!(matches!(
            handle(&app, &get("/api/events")).body,
            Body::EventStream
        ));
        let response = handle(&app, &get("/api/nope"));
        assert_eq!(response.status, 404);
        assert_eq!(body(&response)["error"], "not found");
    }
}
