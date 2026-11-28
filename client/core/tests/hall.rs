mod support;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use support::FchBuilder;
use tracker_core::data;
use tracker_core::hall::{query_param, Hall, Request, Response};
use tracker_core::store::Store;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("tracker-hall-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn hall(tag: &str, world: Option<PathBuf>) -> Hall {
    Hall::new(Arc::new(Store::open(tmp(tag)).unwrap()), world, Some("t".into()), 600.0)
}

fn call(h: &Hall, method: &str, path: &str, query: &str, token: Option<&str>, body: &[u8]) -> Response {
    h.handle(&Request { method, path, query, token, host: None, body })
}

fn json(r: &Response) -> Value {
    serde_json::from_slice(&r.body).unwrap()
}

#[test]
fn upload_needs_token() {
    let h = hall("upload", None);
    let save = FchBuilder::new(47).build();
    assert_eq!(call(&h, "POST", "/upload", "name=A", Some("x"), &save).status, 403);
    let r = call(&h, "POST", "/upload", "name=A", None, &save);
    assert_eq!((r.status, json(&r)["error"].as_str()), (403, Some("bad token")));
    let r = call(&h, "POST", "/upload", "name=A", Some("t"), &save);
    assert_eq!((r.status, json(&r)), (200, serde_json::json!({"ok": true})));
    let r = call(&h, "GET", "/api/characters", "", None, b"");
    assert_eq!(json(&r)["characters"][0]["name"], "A");
    let r = call(&h, "POST", "/upload", "name=%C3%85sa", Some("t"), &save);
    assert_eq!((r.status, json(&r)["error"].as_str()), (400, Some("bad name or size")));
    assert_eq!(call(&h, "POST", "/nope", "", Some("t"), b"").status, 404);
}

#[test]
fn no_configured_token_refuses_everything() {
    let h = Hall::new(Arc::new(Store::open(tmp("notoken")).unwrap()), None, None, 600.0);
    assert_eq!(call(&h, "POST", "/upload", "name=A", Some(""), &FchBuilder::new(47).build()).status, 403);
}

#[test]
fn heartbeat_unknown_is_404() {
    let h = hall("hb", None);
    let r = call(&h, "POST", "/heartbeat", "name=__probe__", Some("t"), b"");
    assert_eq!((r.status, json(&r)["error"].as_str()), (404, Some("unknown or outdated, upload again")));
    call(&h, "POST", "/upload", "name=A", Some("t"), &FchBuilder::new(47).build());
    assert_eq!(call(&h, "POST", "/heartbeat", "name=A", Some("t"), b"").status, 200);
}

#[test]
fn name_is_percent_decoded() {
    let h = hall("pct", None);
    let r = call(&h, "POST", "/upload", "name=Ragnar%20Lodbrok", Some("t"), &FchBuilder::new(47).build());
    assert_eq!(r.status, 200);
    let r = call(&h, "GET", "/api/character/Ragnar%20Lodbrok", "", None, b"");
    assert_eq!((r.status, json(&r)["name"].as_str()), (200, Some("Ragnar Lodbrok")));
    assert_eq!(call(&h, "GET", "/api/character/nobody", "", None, b"").status, 404);
    assert_eq!(query_param("a=1&name=x+y%21&name=z", "name").as_deref(), Some("x y!"));
    assert_eq!(query_param("a=1", "name"), None);
}

#[test]
fn static_routes() {
    let h = hall("static", None);
    let r = call(&h, "GET", "/", "", None, b"");
    assert_eq!(r.content_type, "text/html; charset=utf-8");
    assert!(String::from_utf8_lossy(&r.body[..9]).eq_ignore_ascii_case("<!doctype"));
    assert_eq!(call(&h, "GET", "/worldgen.js", "", None, b"").content_type, "text/javascript; charset=utf-8");
    let r = call(&h, "GET", "/fonts/cinzel.woff2", "", None, b"");
    assert_eq!((r.status, r.content_type, r.cache), (200, "font/woff2", true));
    assert_eq!(call(&h, "GET", "/fonts/../x", "", None, b"").status, 404);
    assert_eq!(call(&h, "GET", "/fonts/missing.woff2", "", None, b"").status, 404);
    assert_eq!(call(&h, "GET", "/api/achievements", "", None, b"").body, data::ACHIEVEMENTS_JSON.as_bytes());
}

#[test]
fn world_routes_report_errors_with_200() {
    let empty = tmp("emptyworld");
    std::fs::create_dir_all(&empty).unwrap();
    for dir in [None, Some(empty)] {
        let h = hall("world", dir);
        for p in ["/api/world", "/api/map"] {
            let r = call(&h, "GET", p, "", None, b"");
            assert_eq!(r.status, 200, "{p}");
            assert!(json(&r).get("error").is_some(), "{p}");
        }
    }
}

#[test]
fn serve_roundtrip() {
    let h = Arc::new(hall("serve", None));
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || tracker_core::serve::serve(l, Arc::new(move |r| h.handle(r))));
    let base = format!("http://127.0.0.1:{port}");
    let v: Value = serde_json::from_str(&ureq::get(&format!("{base}/api/characters")).call().unwrap().into_string().unwrap()).unwrap();
    assert!(v.get("characters").is_some());
    let up = ureq::post(&format!("{base}/upload?name=Ragnar%20Lodbrok")).set("X-Token", "t").send_bytes(&FchBuilder::new(47).build()).unwrap();
    assert_eq!(up.status(), 200);
    let r = ureq::post(&format!("{base}/upload?name=A")).set("X-Token", "bad").send_bytes(b"x");
    assert!(matches!(r, Err(ureq::Error::Status(403, _))));
    let font = ureq::get(&format!("{base}/fonts/cinzel.woff2")).call().unwrap();
    assert_eq!(font.header("Cache-Control"), Some("public, max-age=86400"));
}

#[test]
fn unreadable_world_is_200_error_and_recovers() {
    let dir = tmp("badworld");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("_main.1.ok"), b"").unwrap();
    std::fs::write(dir.join("_main.1.fwl2"), [1u8, 2, 3]).unwrap();
    std::fs::write(dir.join("_main.1.db2"), [1u8, 2, 3]).unwrap();
    let h = hall("badworld", Some(dir.clone()));
    for _ in 0..2 {
        for p in ["/api/world", "/api/map"] {
            let r = call(&h, "GET", p, "", None, b"");
            assert_eq!(r.status, 200, "{p}");
            assert!(json(&r).get("error").is_some(), "{p}");
        }
    }
    h.set_world_dir(None);
    assert!(json(&call(&h, "GET", "/api/world", "", None, b""))["error"].is_string());
}

#[test]
fn map_matches_goldens() {
    let Some(g) = support::golden_dir() else { return };
    let rd = |f: &str| -> Value { serde_json::from_slice(&std::fs::read(g.join(f)).unwrap()).unwrap() };
    let manifest = rd("manifest.json");
    let (world, locs) = (rd("world.json"), rd("locations.json"));
    let h = hall("map", Some(PathBuf::from(manifest["world"].as_str().unwrap())));
    let r = call(&h, "GET", "/api/map", "", None, b"");
    assert_eq!(r.status, 200);
    let m = json(&r);
    assert!(m.get("error").is_none(), "{m}");
    for k in ["name", "seed", "seed_int", "uid", "gen_version"] {
        assert_eq!(m["world"][k], world[k], "{k}");
    }
    assert_eq!(m["world"].as_object().unwrap().len(), 5);
    let names: Vec<&str> = m["location_names"].as_array().unwrap().iter().map(|n| n.as_str().unwrap()).collect();
    assert!(names.windows(2).all(|w| w[0] < w[1]), "sorted and unique");
    let (got, want) = (m["locations"].as_array().unwrap(), locs.as_array().unwrap());
    assert!(!want.is_empty());
    assert_eq!(got.len(), want.len());
    for (g, w) in got.iter().zip(want) {
        assert_eq!(names[g[0].as_u64().unwrap() as usize], w[0].as_str().unwrap());
        assert_eq!((&g[1], &g[2]), (&w[1], &w[2]));
    }
    assert!(m["players"].is_array());
}

#[test]
fn serve_passes_the_host_header() {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        tracker_core::serve::serve(l, Arc::new(|r: &Request| Response::json(200, &serde_json::json!({ "host": r.host }))))
    });
    let base = format!("http://127.0.0.1:{port}");
    let v: Value = serde_json::from_str(&ureq::get(&base).call().unwrap().into_string().unwrap()).unwrap();
    assert_eq!(v["host"], format!("127.0.0.1:{port}"));
    let v: Value = serde_json::from_str(&ureq::get(&base).set("Host", "evil.example").call().unwrap().into_string().unwrap()).unwrap();
    assert_eq!(v["host"], "evil.example");
}

fn names(h: &Hall) -> (Vec<String>, bool) {
    let v = json(&call(h, "GET", "/api/characters", "", None, b""));
    let n = v["characters"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap().to_string()).collect();
    (n, v["world_only"] == true)
}

fn upload_in(h: &Hall, name: &str, world: &str) {
    let save = FchBuilder::new(47).dict(0, "worlds", world, 3.0).build();
    assert_eq!(call(h, "POST", "/upload", &format!("name={name}"), Some("t"), &save).status, 200);
}

#[test]
fn world_only_without_a_world_lists_everyone() {
    let h = hall("wonly-none", None);
    upload_in(&h, "A", "roota");
    upload_in(&h, "B", "savheim");
    h.set_world_only(true);
    assert_eq!(names(&h), (vec!["A".to_string(), "B".to_string()], false));
}

#[test]
fn world_only_lists_the_shown_worlds_characters() {
    let Some(g) = support::golden_dir() else { return };
    let manifest: Value = serde_json::from_slice(&std::fs::read(g.join("manifest.json")).unwrap()).unwrap();
    let dir = PathBuf::from(manifest["world"].as_str().unwrap());
    let h = hall("wonly", Some(dir));
    let wname = json(&call(&h, "GET", "/api/world", "", None, b""))["name"].as_str().unwrap().to_uppercase();
    upload_in(&h, "In", &wname);
    upload_in(&h, "Out", "elsewhere");
    assert_eq!(names(&h).0.len(), 2);
    h.set_world_only(true);
    assert_eq!(names(&h), (vec!["In".to_string()], true));
    h.set_world_only(false);
    assert_eq!(names(&h).0.len(), 2);
}

#[test]
fn loading_flag_until_cleared() {
    let h = hall("loading", None);
    let get = |h: &Hall| json(&call(h, "GET", "/api/characters", "", None, b""));
    assert!(get(&h).get("loading").is_none());
    h.set_loading(true);
    assert_eq!(get(&h)["loading"], true);
    h.set_loading(false);
    assert!(get(&h).get("loading").is_none());
}
