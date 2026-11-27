mod support;

use std::fs;
use std::path::{Path, PathBuf};
use support::{assert_json_close, golden_dir};
use tracker_core::worlds;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vt-worlds-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn touch_save(dir: &Path, n: u32, ok: bool) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join(format!("_main.{n}.fwl2")), b"").unwrap();
    fs::write(dir.join(format!("_main.{n}.db2")), b"").unwrap();
    if ok {
        fs::write(dir.join(format!("_main.{n}.ok")), b"").unwrap();
    }
}

#[test]
fn stable_hash_matches_game() {
    assert_eq!(worlds::stable_hash("StartTemple"), 2749981249);
}

#[test]
fn list_worlds_ignores_old_format_and_backups() {
    let root = tmp("list");
    touch_save(&root.join("roota"), 14, true);
    touch_save(&root.join("roota_backup_auto-20261126-010630"), 13, true);
    fs::create_dir_all(root.join("old")).unwrap();
    fs::write(root.join("old/_main.2.fwl2"), b"").unwrap();
    fs::write(root.join("Linux.db"), b"").unwrap();
    fs::write(root.join("Linux.fwl"), b"").unwrap();
    let names: Vec<String> = worlds::list_worlds(&root).into_iter().map(|w| w.name).collect();
    assert_eq!(names, ["roota"]);
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn worlds_root_prefers_worlds_local() {
    let base = tmp("root");
    let chars = base.join("characters");
    fs::create_dir_all(&chars).unwrap();
    assert_eq!(worlds::worlds_root(&chars), None);
    fs::create_dir_all(base.join("worlds")).unwrap();
    assert_eq!(worlds::worlds_root(&chars), Some(base.join("worlds")));
    fs::create_dir_all(base.join("worlds_local")).unwrap();
    assert_eq!(worlds::worlds_root(&chars), Some(base.join("worlds_local")));
    fs::remove_dir_all(&base).unwrap();
}

#[test]
fn read_world_without_save_errors() {
    let d = tmp("empty");
    let e = worlds::read_world(&d).unwrap_err();
    assert!(e.starts_with("no complete _main.N.ok save"), "{e}");
    assert!(worlds::locations(&d).is_empty());
    fs::remove_dir_all(&d).unwrap();
}

#[test]
fn parity_world() {
    let Some(g) = golden_dir() else { return };
    let manifest: serde_json::Value = serde_json::from_slice(&fs::read(g.join("manifest.json")).unwrap()).unwrap();
    let dir = PathBuf::from(manifest["world"].as_str().unwrap());
    let want: serde_json::Value = serde_json::from_slice(&fs::read(g.join("world.json")).unwrap()).unwrap();
    let got = worlds::read_world(&dir).unwrap();
    let (gm, wm) = (got["save_mtime"].as_f64().unwrap(), want["save_mtime"].as_f64().unwrap());
    assert!((gm - wm).abs() < 1e-3, "save_mtime {gm} vs {wm}");
    let strip = |mut v: serde_json::Value| {
        v["save_mtime"] = 0.into();
        v
    };
    assert_json_close("world", &strip(got), &strip(want));

    let wl: serde_json::Value = serde_json::from_slice(&fs::read(g.join("locations.json")).unwrap()).unwrap();
    let want_locs = wl.as_array().unwrap();
    assert!(!want_locs.is_empty(), "no golden locations compared");
    let got_locs: Vec<serde_json::Value> = worlds::locations(&dir).into_iter().map(|(n, x, z)| serde_json::json!([n, x, z])).collect();
    assert_eq!(got_locs.len(), want_locs.len());
    assert_eq!(&got_locs, want_locs);
}
