//! The Hall's routes (the request handling of `server.py`), without an HTTP library.
use crate::store::{guarded, valid_name, IngestError, Store};
use crate::{data, worlds};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Request<'a> {
    pub method: &'a str,
    pub path: &'a str,
    pub query: &'a str,
    pub token: Option<&'a str>,
    pub body: &'a [u8],
}

pub struct Response {
    pub status: u16,
    pub content_type: &'static str,
    pub body: Vec<u8>,
    pub cache: bool,
}

impl Response {
    pub fn json(status: u16, v: &Value) -> Response {
        Response::raw(status, "application/json", v.to_string().into_bytes())
    }

    fn raw(status: u16, content_type: &'static str, body: Vec<u8>) -> Response {
        Response { status, content_type, body, cache: false }
    }

    fn err(status: u16, msg: &str) -> Response {
        Response::json(status, &json!({ "error": msg }))
    }
}

type Cached<T> = Mutex<Option<(Option<u32>, T)>>;

/// Locks, ignoring poison: the guarded data is a plain cache.
fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub struct Hall {
    store: Arc<Store>,
    world_dir: Mutex<Option<PathBuf>>,
    token: Option<String>,
    stale_secs: f64,
    world: Cached<Value>,
    locs: Cached<Vec<(String, i64, i64)>>,
}

fn now() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64())
}

fn ct_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn percent_decode(s: &str, plus: bool) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() && b[i + 1].is_ascii_hexdigit() && b[i + 2].is_ascii_hexdigit() => {
                out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap());
                i += 3;
                continue;
            }
            b'+' if plus => out.push(b' '),
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// First value of `key` in a query string, percent-decoded (`+` is a space), like `parse_qs`.
pub fn query_param(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        (percent_decode(k, true) == key).then(|| percent_decode(v, true))
    })
}

impl Hall {
    pub fn new(store: Arc<Store>, world_dir: Option<PathBuf>, token: Option<String>, stale_secs: f64) -> Hall {
        Hall {
            store,
            world_dir: Mutex::new(world_dir),
            token: token.filter(|t| !t.is_empty()),
            stale_secs,
            world: Mutex::new(None),
            locs: Mutex::new(None),
        }
    }

    pub fn set_world_dir(&self, dir: Option<PathBuf>) {
        *lock(&self.world_dir) = dir;
        *lock(&self.world) = None;
        *lock(&self.locs) = None;
    }

    pub fn store(&self) -> &Arc<Store> {
        &self.store
    }

    pub fn handle(&self, req: &Request) -> Response {
        match req.method {
            "POST" => self.post(req),
            "GET" | "HEAD" => self.get(req),
            _ => Response::err(404, "not found"),
        }
    }

    fn post(&self, req: &Request) -> Response {
        if req.path != "/upload" && req.path != "/heartbeat" {
            return Response::err(404, "not found");
        }
        let ok = matches!((&self.token, req.token), (Some(t), Some(r)) if ct_eq(r, t));
        if !ok {
            return Response::err(403, "bad token");
        }
        let name = query_param(req.query, "name").unwrap_or_default();
        if req.path == "/heartbeat" {
            return if valid_name(&name) && self.store.touch(&name, now()) {
                Response::json(200, &json!({"ok": true}))
            } else {
                Response::err(404, "unknown or outdated, upload again")
            };
        }
        match self.store.ingest(&name, req.body, now()) {
            Ok(()) => Response::json(200, &json!({"ok": true})),
            Err(IngestError::Bad(m)) => Response::err(400, &m),
        }
    }

    fn get(&self, req: &Request) -> Response {
        let p = percent_decode(req.path, false);
        match p.as_str() {
            "/" => return Response::raw(200, "text/html; charset=utf-8", data::DASHBOARD_HTML.as_bytes().to_vec()),
            "/api/characters" => return Response::json(200, &self.store.characters(now(), self.stale_secs)),
            "/api/achievements" => return Response::raw(200, "application/json", data::ACHIEVEMENTS_JSON.as_bytes().to_vec()),
            "/worldgen.js" => return Response::raw(200, "text/javascript; charset=utf-8", data::WORLDGEN_JS.as_bytes().to_vec()),
            "/api/world" => {
                return match self.world() {
                    Ok(w) => Response::json(200, &w),
                    Err(e) => Response::err(200, &e),
                }
            }
            "/api/map" => {
                return match self.map() {
                    Ok(m) => Response::json(200, &m),
                    Err(e) => Response::err(200, &e),
                }
            }
            _ => {}
        }
        if let Some(f) = p.strip_prefix("/fonts/") {
            let named = !f.is_empty()
                && f.strip_suffix(".woff2")
                    .is_some_and(|s| !s.is_empty() && s.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-'));
            if let (true, Some(b)) = (named, data::font(f)) {
                return Response { cache: true, ..Response::raw(200, "font/woff2", b.to_vec()) };
            }
        }
        if let Some(n) = p.strip_prefix("/api/character/") {
            if !n.contains('/') {
                if let Some(b) = self.store.character(n) {
                    return Response::raw(200, "application/json", b);
                }
            }
        }
        Response::err(404, "not found")
    }

    fn dir_and_save(&self) -> Result<(PathBuf, Option<u32>), String> {
        let dir = lock(&self.world_dir).clone().ok_or("no world folder configured")?;
        let n = worlds::latest_save(&dir);
        Ok((dir, n))
    }

    fn world(&self) -> Result<Value, String> {
        let (dir, n) = self.dir_and_save()?;
        if let Some((k, v)) = &*lock(&self.world) {
            if *k == n {
                return Ok(v.clone());
            }
        }
        let v = guarded(|| worlds::read_world(&dir)).unwrap_or_else(|| Err("world unreadable".to_string()))?;
        *lock(&self.world) = Some((n, v.clone()));
        Ok(v)
    }

    fn locations(&self) -> Result<Vec<(String, i64, i64)>, String> {
        let (dir, n) = self.dir_and_save()?;
        if let Some((k, v)) = &*lock(&self.locs) {
            if *k == n {
                return Ok(v.clone());
            }
        }
        let v = guarded(|| worlds::locations(&dir)).ok_or("world unreadable")?;
        *lock(&self.locs) = Some((n, v.clone()));
        Ok(v)
    }

    fn map(&self) -> Result<Value, String> {
        let w = self.world()?;
        let locs = self.locations()?;
        let mut names: Vec<&str> = locs.iter().map(|(n, _, _)| n.as_str()).collect();
        names.sort();
        names.dedup();
        let rows: Vec<Value> = locs
            .iter()
            .map(|(n, x, z)| json!([names.binary_search(&n.as_str()).unwrap(), x, z]))
            .collect();
        let uid = w["uid"].as_str().map(str::to_string).unwrap_or_else(|| w["uid"].to_string());
        let world: serde_json::Map<String, Value> = ["name", "seed", "seed_int", "uid", "gen_version"]
            .iter()
            .map(|k| (k.to_string(), w[*k].clone()))
            .collect();
        Ok(json!({"world": world, "location_names": names, "locations": rows, "players": self.store.maps_for(&uid)}))
    }
}
