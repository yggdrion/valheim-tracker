//! The Hall inside the app: a loopback server for the window's iframe. In `Local` mode it serves the
//! characters ingested from this machine; in `Group` mode it forwards GETs to the group's server.
//! It has no token, so every POST to the Hall itself is refused (403), and it answers only requests addressed
//! to its own loopback origin (a Host other than 127.0.0.1:port / localhost:port is refused: DNS rebinding).

use std::io;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tracker_core::hall::{Hall, Request, Response};
use tracker_core::serve::serve;
use tracker_core::store::Store;

#[derive(Clone, Debug, PartialEq)]
pub enum Mode {
    Local,
    Group { server: String },
}

#[derive(Clone)]
pub struct LocalHall {
    pub hall: Arc<Hall>,
    pub port: u16,
    pub mode: Arc<RwLock<Mode>>,
}

fn stale_secs() -> f64 {
    let minutes = std::env::var("STALE_MINUTES").ok().and_then(|v| v.trim().parse::<f64>().ok()).unwrap_or(10.0);
    minutes * 60.0
}

/// Binds `127.0.0.1:preferred` (the last run's port, so the iframe keeps its origin and the dashboard its
/// localStorage), or any free port when that fails or `preferred` is 0.
pub fn start(state_dir: PathBuf, preferred: u16) -> io::Result<LocalHall> {
    let store = Arc::new(Store::open(state_dir)?);
    let hall = Arc::new(Hall::new(store, None, None, stale_secs()));
    hall.set_world_only(true);
    let listener = match preferred {
        0 => TcpListener::bind("127.0.0.1:0")?,
        p => TcpListener::bind(("127.0.0.1", p)).or_else(|_| TcpListener::bind("127.0.0.1:0"))?,
    };
    let port = listener.local_addr()?.port();
    let lh = LocalHall { hall, port, mode: Arc::new(RwLock::new(Mode::Local)) };
    let h = lh.clone();
    std::thread::spawn(move || serve(listener, Arc::new(move |r: &Request| h.handle(r))));
    Ok(lh)
}

fn static_type(upstream: &str) -> &'static str {
    let t = upstream.split(';').next().unwrap_or("").trim().to_ascii_lowercase();
    match t.as_str() {
        "application/json" => "application/json",
        "text/html" => "text/html; charset=utf-8",
        "text/javascript" | "application/javascript" => "text/javascript; charset=utf-8",
        "font/woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}

fn unreachable_page(server: &str) -> Response {
    let server = server.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let body = format!(
        "<!doctype html><meta charset=utf-8><body style=\"font-family:sans-serif;padding:2em;color:#ccc;background:#1b1b1b\">\
         <p>Cannot reach the group's server ({server}). The Local view still works: switch to Local in the title bar.</p>"
    );
    Response { status: 502, content_type: "text/html; charset=utf-8", body: body.into_bytes(), cache: false }
}

impl LocalHall {
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/", self.port)
    }

    fn own_host(&self, host: Option<&str>) -> bool {
        let Some(h) = host else { return false };
        let h = h.to_ascii_lowercase();
        h == format!("127.0.0.1:{}", self.port) || h == format!("localhost:{}", self.port)
    }

    pub fn handle(&self, req: &Request) -> Response {
        if !self.own_host(req.host) {
            return Response::json(403, &serde_json::json!({ "error": "forbidden" }));
        }
        let mode = self.mode.read().map(|m| m.clone()).unwrap_or(Mode::Local);
        match mode {
            Mode::Local => self.hall.handle(req),
            Mode::Group { server } => proxy(&server, req),
        }
    }
}

fn proxy(server: &str, req: &Request) -> Response {
    if req.method != "GET" && req.method != "HEAD" {
        return Response::json(404, &serde_json::json!({ "error": "not found" }));
    }
    // the path is appended to the configured server: refuse anything that could change the host
    if !req.path.starts_with('/') || req.path.starts_with("//") {
        return Response::json(404, &serde_json::json!({ "error": "not found" }));
    }
    let base = server.trim().trim_end_matches('/');
    let url = if req.query.is_empty() { format!("{base}{}", req.path) } else { format!("{base}{}?{}", req.path, req.query) };
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(15)).build();
    let done = |status: u16, r: ureq::Response| {
        let content_type = static_type(r.content_type());
        let mut body = Vec::new();
        std::io::Read::read_to_end(&mut r.into_reader(), &mut body).ok()?;
        Some(Response { status, content_type, body, cache: req.path.starts_with("/fonts/") })
    };
    let res = match agent.get(&url).call() {
        Ok(r) => done(r.status(), r),
        Err(ureq::Error::Status(code, r)) => done(code, r),
        Err(_) => None,
    };
    res.unwrap_or_else(|| unreachable_page(base))
}

#[cfg(test)]
#[path = "../../core/tests/support/mod.rs"]
pub(crate) mod support;

#[cfg(test)]
mod tests {
    use super::support;
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn tmp() -> PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let p = std::env::temp_dir().join(format!("vts-local-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn local_hall_refuses_uploads() {
        let lh = start(tmp(), 0).unwrap();
        assert!(lh.url().starts_with("http://127.0.0.1:"));
        let r = ureq::post(&format!("{}upload?name=A", lh.url())).set("X-Token", "anything").send_bytes(b"x");
        match r {
            Err(ureq::Error::Status(403, _)) => {}
            other => panic!("expected 403, got {:?}", other.map(|r| r.status())),
        }
    }

    #[test]
    fn refuses_foreign_host() {
        let lh = start(tmp(), 0).unwrap();
        for host in ["evil.example", "evil.example:80", "127.0.0.1", "127.0.0.1:1"] {
            match ureq::get(&format!("{}api/characters", lh.url())).set("Host", host).call() {
                Err(ureq::Error::Status(403, _)) => {}
                other => panic!("{host}: expected 403, got {:?}", other.map(|r| r.status())),
            }
        }
        for host in [format!("127.0.0.1:{}", lh.port), format!("localhost:{}", lh.port)] {
            assert_eq!(ureq::get(&format!("{}api/characters", lh.url())).set("Host", &host).call().unwrap().status(), 200);
        }
    }

    #[test]
    fn binds_the_preferred_port_or_any() {
        let a = start(tmp(), 0).unwrap();
        // taken: falls back to a free port
        let b = start(tmp(), a.port).unwrap();
        assert_ne!(b.port, a.port);
        let free = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        assert_eq!(start(tmp(), free).unwrap().port, free);
    }

    #[test]
    fn proxy_forwards_get() {
        let up = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = up.local_addr().unwrap().port();
        std::thread::spawn(move || {
            serve(
                up,
                Arc::new(|r: &Request| {
                    if r.path == "/api/world" && r.query == "a=1" {
                        Response::json(200, &serde_json::json!({ "x": 1 }))
                    } else {
                        Response::json(404, &serde_json::json!({ "error": "not found" }))
                    }
                }),
            )
        });
        let lh = start(tmp(), 0).unwrap();
        *lh.mode.write().unwrap() = Mode::Group { server: format!("http://127.0.0.1:{port}/") };
        let r = ureq::get(&format!("{}api/world?a=1", lh.url())).call().unwrap();
        assert_eq!(r.content_type(), "application/json");
        assert_eq!(r.into_string().unwrap(), r#"{"x":1}"#);
        match ureq::get(&format!("{}api/nope", lh.url())).call() {
            Err(ureq::Error::Status(404, _)) => {}
            other => panic!("expected 404 passthrough, got {:?}", other.map(|r| r.status())),
        }
        // a request target that is not an origin-form path must not change the upstream host
        for target in ["@evil.example/x", "//evil.example/x"] {
            use std::io::{Read, Write};
            let mut c = std::net::TcpStream::connect(("127.0.0.1", lh.port)).unwrap();
            write!(c, "GET {target} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n", lh.port).unwrap();
            let mut out = String::new();
            let _ = c.read_to_string(&mut out);
            assert!(out.starts_with("HTTP/1.1 404"), "{target}: {out}");
        }
        match ureq::post(&format!("{}api/world", lh.url())).send_bytes(b"") {
            Err(ureq::Error::Status(404, _)) => {}
            other => panic!("expected 404 for POST, got {:?}", other.map(|r| r.status())),
        }
    }

    #[test]
    fn proxy_unreachable_gives_502_page() {
        let lh = start(tmp(), 0).unwrap();
        *lh.mode.write().unwrap() = Mode::Group { server: "http://127.0.0.1:1".into() };
        match ureq::get(&lh.url()).call() {
            Err(ureq::Error::Status(502, r)) => {
                assert_eq!(r.content_type(), "text/html");
                assert!(r.into_string().unwrap().contains("Cannot reach the group's server"));
            }
            other => panic!("expected 502, got {:?}", other.map(|r| r.status())),
        }
    }

    #[test]
    fn ingest_shows_in_local_hall() {
        let lh = start(tmp(), 0).unwrap();
        // a minimal hand-built save (FchBuilder from core's test support), not a golden
        lh.hall.store().ingest("A", &support::FchBuilder::new(47).build(), 1.0).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&ureq::get(&format!("{}api/characters", lh.url())).call().unwrap().into_string().unwrap()).unwrap();
        assert_eq!(v["characters"][0]["name"], "A");
    }
}
