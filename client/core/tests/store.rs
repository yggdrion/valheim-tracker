mod support;
use serde_json::{json, Value};
use std::path::PathBuf;
use support::FchBuilder;
use tracker_core::store::{IngestError, Store};

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("tracker-store-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn bad(msg: &str) -> Result<(), IngestError> {
    Err(IngestError::Bad(msg.into()))
}

#[test]
fn ingest_then_list() {
    let s = Store::open(tmp("list")).unwrap();
    s.ingest("Ragnar Lodbrok", &FchBuilder::new(47).build(), 100.0).unwrap();
    let c = s.characters(100.0, 600.0);
    let l = c["characters"].as_array().unwrap();
    assert_eq!(l.len(), 1);
    assert_eq!(l[0]["name"], "Ragnar Lodbrok");
    assert_eq!(l[0]["online"], true);
    assert_eq!(l[0]["seen"], 100.0);
    assert_eq!(c["stale_minutes"], 10.0);
    assert!(l[0]["sections"][0].get("entries").is_none());
    assert_eq!(s.characters(800.0, 600.0)["characters"][0]["online"], false);
    assert!(s.character("Ragnar Lodbrok").is_some());
}

#[test]
fn ingest_rejects_bad_names() {
    let s = Store::open(tmp("names")).unwrap();
    let ok = FchBuilder::new(47).build();
    for n in ["Åsa".to_string(), "a".repeat(65), String::new()] {
        assert_eq!(s.ingest(&n, &ok, 1.0), bad("bad name or size"), "{n}");
    }
    assert_eq!(s.ingest("x", &vec![0u8; 5_000_001], 1.0), bad("bad name or size"));
    assert_eq!(s.ingest("x", &[], 1.0), bad("bad name or size"));
}

#[test]
fn ingest_reports_save_errors() {
    let s = Store::open(tmp("errs")).unwrap();
    let r = s.ingest("old", &FchBuilder::new(45).build(), 1.0);
    match r {
        Err(IngestError::Bad(m)) => assert!(m.contains("profile version 45"), "{m}"),
        other => panic!("{other:?}"),
    }
    assert!(matches!(s.ingest("junk", &[1, 0, 0, 0, 7], 1.0), Err(IngestError::Bad(_))));
    assert!(s.characters(1.0, 600.0)["characters"].as_array().unwrap().is_empty());
}

#[test]
fn touch_needs_current_av() {
    let dir = tmp("touch");
    let s = Store::open(dir.clone()).unwrap();
    s.ingest("a", &FchBuilder::new(47).build(), 1.0).unwrap();
    assert!(s.touch("a", 50.0));
    assert_eq!(s.characters(50.0, 600.0)["characters"][0]["seen"], 50.0);
    let p = dir.join("a.json");
    let mut v: Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
    v["av"] = json!("old");
    std::fs::write(&p, serde_json::to_vec(&v).unwrap()).unwrap();
    assert!(!s.touch("a", 60.0));
    assert!(!s.touch("nobody", 60.0));
    assert!(!s.touch("Åsa", 60.0));
}

#[test]
fn reads_python_state() {
    let dir = tmp("py");
    let s = Store::open(dir.clone()).unwrap();
    let doc = json!({"name": "Py", "updated": 42.5, "av": "x", "version": 47, "worlds": ["w"],
        "sections": [{"id": "i", "title": "T", "total": 3, "done": 1, "entries": []}],
        "achievements": {}, "meta": {}});
    std::fs::write(dir.join("Py.json"), doc.to_string()).unwrap();
    let c = s.characters(1_700_000_000.0, 600.0);
    let e = &c["characters"][0];
    assert_eq!(e["seen"], 42.5);
    assert_eq!(e["online"], false);
    assert_eq!(e["sections"][0], json!({"id": "i", "title": "T", "done": 1, "total": 3}));
}

#[test]
fn maps_listed_per_world() {
    let dir = tmp("maps");
    let s = Store::open(dir.clone()).unwrap();
    std::fs::write(dir.join("B.map.json"), json!({"7": {"explored": [1], "pins": []}}).to_string()).unwrap();
    std::fs::write(dir.join("A.map.json"), json!({"7": {"explored": []}, "8": {"explored": [1]}}).to_string()).unwrap();
    let m = s.maps_for("7");
    assert_eq!(m.len(), 1);
    assert_eq!(m[0]["name"], "B");
    assert_eq!(m[0]["pins"], json!([]));
}

#[test]
fn remove_forgets() {
    let s = Store::open(tmp("rm")).unwrap();
    s.ingest("a", &FchBuilder::new(47).build(), 1.0).unwrap();
    s.remove("a");
    assert!(s.characters(1.0, 600.0)["characters"].as_array().unwrap().is_empty());
    assert!(s.character("a").is_none());
}

#[test]
fn guarded_maps_panic_to_none() {
    assert_eq!(tracker_core::store::guarded(|| 5), Some(5));
    assert_eq!(tracker_core::store::guarded(|| -> i32 { panic!("boom") }), None);
}

#[test]
fn concurrent_ingest_same_name() {
    let dir = tmp("conc");
    let s = std::sync::Arc::new(Store::open(dir.clone()).unwrap());
    let raw = std::sync::Arc::new(FchBuilder::new(47).build());
    let hs: Vec<_> = (0..8)
        .map(|i| {
            let (s, raw) = (s.clone(), raw.clone());
            std::thread::spawn(move || {
                for j in 0..10 {
                    assert_eq!(s.ingest("same", &raw, (i * 10 + j) as f64), Ok(()));
                    let _ = s.characters(1e9, 600.0);
                    if let Some(b) = s.character("same") {
                        serde_json::from_slice::<Value>(&b).expect("torn read");
                    }
                }
            })
        })
        .collect();
    for h in hs {
        h.join().unwrap();
    }
    serde_json::from_slice::<Value>(&s.character("same").unwrap()).unwrap();
    assert!(!dir.join("same.tmp").exists() && !dir.join("same.tmpm").exists());
}
