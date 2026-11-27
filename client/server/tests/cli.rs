use std::path::PathBuf;
use std::process::{Command, Output};
use tracker_core::data::GAMEDATA;

fn run(args: &[&str], envs: &[(&str, &str)]) -> Output {
    let mut c = Command::new(env!("CARGO_BIN_EXE_tracker-server"));
    c.args(args).env_remove("TRACKER_TOKEN");
    for (k, v) in envs {
        c.env(k, v);
    }
    c.output().unwrap()
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("tracker-cli-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// First golden character save that still exists on this machine (saves are private, never committed).
fn golden_save() -> Option<String> {
    let dir = std::env::var_os("VT_GOLDEN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/golden"));
    let m: serde_json::Value = serde_json::from_slice(&std::fs::read(dir.join("manifest.json")).ok()?).ok()?;
    m["characters"]
        .as_array()?
        .iter()
        .filter_map(|c| c["path"].as_str())
        .find(|p| std::path::Path::new(p).is_file())
        .map(String::from)
}

#[test]
fn no_token_exits() {
    let o = run(&[], &[]);
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("set TRACKER_TOKEN"));
}

#[test]
fn check_prints_sections() {
    let Some(save) = golden_save() else {
        eprintln!("skipping: no golden save on this machine");
        return;
    };
    let o = run(&["check", &save], &[]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let out = String::from_utf8(o.stdout).unwrap();
    assert!(out.starts_with("[\n {\n  \""), "indent 1: {}", &out[..30]);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v.as_array().unwrap().len(), GAMEDATA["sections"].as_array().unwrap().len());
}

#[test]
fn check_rejects_garbage() {
    let f = tmp("garbage").join("x.fch");
    std::fs::write(&f, b"definitely not a save").unwrap();
    let o = run(&["check", f.to_str().unwrap()], &[]);
    assert_eq!(o.status.code(), Some(1));
    assert!(!o.stderr.is_empty());
}

#[test]
fn world_without_save_fails() {
    let d = tmp("world");
    let o = run(&["world", d.to_str().unwrap()], &[]);
    assert_eq!(o.status.code(), Some(1));
    assert!(!o.stderr.is_empty());
}
