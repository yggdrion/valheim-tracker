//! tiny_http glue: one thread per request, handler gets a plain `Request`.
use crate::hall::{Request, Response};
use crate::store::MAX_BODY;
use std::io::Read;
use std::net::TcpListener;
use std::sync::Arc;
use tiny_http::{Header, Server};

pub type Handler = Arc<dyn Fn(&Request) -> Response + Send + Sync>;

/// Blocking: serves until the listener fails.
pub fn serve(listener: TcpListener, handler: Handler) {
    let Ok(server) = Server::from_listener(listener, None) else { return };
    for mut rq in server.incoming_requests() {
        let h = handler.clone();
        std::thread::spawn(move || {
            let method = rq.method().as_str().to_string();
            let url = rq.url().to_string();
            let (path, query) = url.split_once('?').unwrap_or((&url, ""));
            let token = rq.headers().iter().find(|h| h.field.equiv("X-Token")).map(|h| h.value.as_str().to_string());
            let host = rq.headers().iter().find(|h| h.field.equiv("Host")).map(|h| h.value.as_str().to_string());
            let mut body = Vec::new();
            let _ = rq.as_reader().take(MAX_BODY as u64 + 1).read_to_end(&mut body);
            let req = Request { method: &method, path, query, token: token.as_deref(), host: host.as_deref(), body: &body };
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| h(&req))).unwrap_or_else(|_| Response {
                status: 500,
                content_type: "application/json",
                body: br#"{"error":"internal error"}"#.to_vec(),
                cache: false,
            });
            let header = |k: &str, v: &str| Header::from_bytes(k.as_bytes(), v.as_bytes()).unwrap();
            let mut out = tiny_http::Response::from_data(res.body)
                .with_status_code(res.status)
                .with_header(header("Content-Type", res.content_type));
            if res.cache {
                out.add_header(header("Cache-Control", "public, max-age=86400"));
            }
            let _ = rq.respond(out);
        });
    }
}
