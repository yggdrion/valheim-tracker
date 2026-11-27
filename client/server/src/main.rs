//! `tracker-server`: `serve` (default), `check FILE.fch`, `world DIR`.
use serde::Serialize;
use serde_json::{json, Value};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::exit;
use std::sync::Arc;
use tracker_core::hall::Hall;
use tracker_core::store::Store;
use tracker_core::{analyze, fch, serve, worlds};

fn fail(msg: &str) -> ! {
    eprintln!("{msg}");
    exit(1)
}

/// `json.dumps(v, indent=1)`: one space per level.
fn pretty(v: &Value) -> String {
    let mut out = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(b" ");
    let mut ser = serde_json::Serializer::with_formatter(&mut out, fmt);
    v.serialize(&mut ser).expect("serialise json");
    String::from_utf8(out).expect("json is utf-8")
}

fn check(file: &str) {
    let raw = std::fs::read(file).unwrap_or_else(|e| fail(&format!("{file}: {e}")));
    let parsed = fch::parse(&raw).unwrap_or_else(|e| fail(&e.0));
    let v = serde_json::to_value(analyze::sections(&parsed)).expect("sections serialise");
    println!("{}", pretty(&v));
}

fn world(dir: &str) {
    let dir = PathBuf::from(dir);
    let w = worlds::read_world(&dir).unwrap_or_else(|e| fail(&e));
    let locations: Vec<Value> = worlds::locations(&dir).into_iter().map(|(n, x, z)| json!([n, x, z])).collect();
    let hashes: Vec<Value> = worlds::location_hashes(&dir).into_iter().map(|(h, x, z)| json!([h, x, z])).collect();
    println!("{}", pretty(&json!({"world": w, "locations": locations, "hashes": hashes})));
}

fn serve_cmd(args: &[String]) {
    let (mut port, mut host, mut state) = (8080u16, "0.0.0.0".to_string(), "state".to_string());
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = || it.next().cloned().unwrap_or_else(|| fail(&format!("{a} needs a value")));
        match a.as_str() {
            "--port" => port = val().parse().unwrap_or_else(|_| fail("--port needs a number")),
            "--host" => host = val(),
            "--state" => state = val(),
            _ => fail(&format!("unknown argument {a}")),
        }
    }
    let token = std::env::var("TRACKER_TOKEN").unwrap_or_default();
    if token.is_empty() {
        fail("set TRACKER_TOKEN");
    }
    let stale_min: f64 = std::env::var("STALE_MINUTES").ok().and_then(|s| s.trim().parse().ok()).unwrap_or(10.0);
    let world_dir = std::env::var("WORLD_DIR").ok().filter(|s| !s.is_empty()).map(PathBuf::from);
    let store = Store::open(PathBuf::from(&state)).unwrap_or_else(|e| fail(&format!("{state}: {e}")));
    let hall = Hall::new(Arc::new(store), world_dir, Some(token), stale_min * 60.0);
    let listener = TcpListener::bind((host.as_str(), port)).unwrap_or_else(|e| fail(&format!("{host}:{port}: {e}")));
    println!("listening on {host}:{port}");
    serve::serve(listener, Arc::new(move |req| hall.handle(req)));
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("check") if args.len() == 2 => check(&args[1]),
        Some("world") if args.len() == 2 => world(&args[1]),
        Some("check") | Some("world") => fail("usage: tracker-server check FILE.fch | world DIR"),
        _ => serve_cmd(&args),
    }
}
