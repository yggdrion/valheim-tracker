mod support;

use support::{golden_dir, FchBuilder};
use tracker_core::fch::*;

#[test]
fn parses_built_save() {
    let raw = FchBuilder::new(47).stat(0, 3, 12.5).dict(1, "kills", "$enemy_boar", 4.0).build();
    let p = parse(&raw).unwrap();
    assert_eq!(p.version, 47);
    assert_eq!(p.sets.len(), 10);
    assert_eq!(p.sets[0].stats[3], 12.5);
    assert_eq!(p.sets[1].kills["$enemy_boar"], 4.0);
    assert_eq!(p.sets[1].kills_by["all"], p.sets[1].kills);
}

#[test]
fn rejects_old_version() {
    let e = parse(&FchBuilder::new(45).build()).unwrap_err();
    assert_eq!(
        e.0,
        "this save uses profile version 45; the tracker reads version 46 and newer (Valheim 1.0): load and save the character once in a current game"
    );
}

#[test]
fn rejects_bad_length() {
    let e = parse(&[0xff, 0xff, 0xff, 0x7f]).unwrap_err();
    assert_eq!(e.0, "not a Valheim character file (bad length)");
}

#[test]
fn every_prefix_is_an_error_not_a_panic() {
    let full = FchBuilder::new(47).world(42, [1.0, 2.0, 3.0], Some(vec![9, 9])).build();
    for k in 0..full.len() {
        let mut prefix = full[..k].to_vec();
        if k < 4 {
            assert!(parse(&prefix).is_err(), "prefix {k}");
            continue;
        }
        prefix[..4].copy_from_slice(&((k - 4) as i32).to_le_bytes());
        if let Ok(p) = parse(&prefix) {
            let _ = worlds(&prefix, &p);
        }
    }
}

#[test]
fn reads_worlds() {
    let raw = FchBuilder::new(47).world(42, [1.0, 2.0, 3.0], Some(vec![9, 9])).build();
    let p = parse(&raw).unwrap();
    let w = worlds(&raw, &p).unwrap();
    assert_eq!(w.len(), 1);
    assert_eq!(w[0].uid, 42);
    assert_eq!(w[0].home, [1.0, 2.0, 3.0]);
    assert_eq!(w[0].map, Some(vec![9, 9]));
    assert_eq!(w[0].spawn, None);
}

#[test]
fn strip_prefixes() {
    assert_eq!(strip("$item_wood"), "wood");
    assert_eq!(strip("x"), "x");
}

#[test]
fn parity_parse() {
    let Some(dir) = golden_dir() else { return };
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("manifest.json")).unwrap()).unwrap();
    let chars = manifest["characters"].as_array().unwrap();
    assert!(!chars.is_empty());
    for c in chars {
        let (name, path) = (c["name"].as_str().unwrap(), c["path"].as_str().unwrap());
        let Ok(raw) = std::fs::read(path) else {
            eprintln!("skipping {name}: {path} not readable");
            continue;
        };
        let want: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.join(format!("{name}.analysis.json"))).unwrap()).unwrap();
        let p = parse(&raw).unwrap_or_else(|e| panic!("{name}: {}", e.0));
        assert_eq!(p.version as i64, want["version"].as_i64().unwrap(), "{name}");
        let mut got: Vec<&str> = p.sets[0].worlds.keys().map(|s| s.as_str()).collect();
        got.sort();
        let mut w: Vec<&str> = want["worlds"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
        w.sort();
        assert_eq!(got, w, "{name}");
    }
}
