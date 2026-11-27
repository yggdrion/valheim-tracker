mod support;

use base64::Engine;
use flate2::{read::ZlibDecoder, write::GzEncoder, Compression};
use serde_json::Value;
use std::io::{Read, Write};
use support::{assert_json_close, golden_dir, FchBuilder};
use tracker_core::{fch, mapdata};

fn put_str(p: &mut Vec<u8>, s: &str) {
    p.push(s.len() as u8);
    p.extend(s.as_bytes());
}

fn blob(size: i32, own: &[usize], shared: &[usize], pins: &[(&str, [f32; 3], i32)]) -> Vec<u8> {
    let cells = (size * size) as usize;
    let mut z = size.to_le_bytes().to_vec();
    for set in [own, shared] {
        let mut v = vec![0u8; cells];
        for &i in set {
            v[i] = 1;
        }
        z.extend(v);
    }
    z.extend((pins.len() as i32).to_le_bytes());
    for (name, pos, typ) in pins {
        put_str(&mut z, name);
        for c in pos {
            z.extend(c.to_le_bytes());
        }
        z.extend(typ.to_le_bytes());
        z.push(0);
        z.extend(0i64.to_le_bytes());
        put_str(&mut z, "me");
    }
    let mut gz = GzEncoder::new(Vec::new(), Compression::default());
    gz.write_all(&z).unwrap();
    let gz = gz.finish().unwrap();
    let mut out = 1i32.to_le_bytes().to_vec();
    out.extend((gz.len() as i32).to_le_bytes());
    out.extend(gz);
    out
}

fn run(raw: &[u8]) -> indexmap::IndexMap<String, Value> {
    mapdata::worlds(raw, &fch::parse(raw).unwrap())
}

fn inflate(b64: &str) -> Vec<u8> {
    let z = base64::engine::general_purpose::STANDARD.decode(b64).unwrap();
    let mut out = Vec::new();
    ZlibDecoder::new(&z[..]).read_to_end(&mut out).unwrap();
    out
}

#[test]
fn bitmask_merges_own_and_shared() {
    let m = blob(4, &[0], &[9], &[("Home", [12.5, 0.0, -3.5], 1)]);
    let raw = FchBuilder::new(47).world(7, [3.0, 1.0, 4.0], Some(m)).build();
    let out = run(&raw);
    let w = &out["7"];
    let keys: Vec<&String> = w.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["spawn", "logout", "death", "home", "size", "explored", "pins"]);
    assert_eq!(w["size"], 4);
    assert_eq!(inflate(w["explored"].as_str().unwrap()), [0x01, 0x02]);
    assert_eq!(w["pins"][0]["name"], "Home");
    assert_eq!(w["pins"][0]["x"], 12);
    assert_eq!(w["pins"][0]["z"], -4);
    assert_eq!(w["pins"][0]["type"], 1);
    assert_eq!(w["home"], serde_json::json!([3, 4]));
}

#[test]
fn home_zero_is_null() {
    let raw = FchBuilder::new(47).world(7, [0.0, 5.0, 0.0], None).build();
    let w = &run(&raw)["7"];
    assert!(w["home"].is_null());
    assert!(w.get("size").is_none());
}

#[test]
fn garbage_map_gives_empty() {
    let raw = FchBuilder::new(47).world(7, [1.0, 1.0, 1.0], Some(vec![1, 2, 3])).build();
    assert!(run(&raw).is_empty());
}

#[test]
fn parity_map() {
    let Some(dir) = golden_dir() else { return };
    let manifest: Value = serde_json::from_slice(&std::fs::read(dir.join("manifest.json")).unwrap()).unwrap();
    let mut compared = 0;
    for c in manifest["characters"].as_array().unwrap() {
        let (name, path) = (c["name"].as_str().unwrap(), c["path"].as_str().unwrap());
        let Ok(raw) = std::fs::read(path) else {
            eprintln!("skipping {name}: {path} not readable");
            continue;
        };
        let want: Value =
            serde_json::from_slice(&std::fs::read(dir.join(format!("{name}.map.json"))).unwrap()).unwrap();
        let got = run(&raw);
        let mut got = serde_json::to_value(&got).unwrap();
        for (uid, w) in want.as_object().unwrap() {
            let g = got.get_mut(uid).unwrap_or_else(|| panic!("{name}: world {uid} missing"));
            if let Some(e) = w.get("explored") {
                let (ge, we) = (inflate(g["explored"].as_str().unwrap()), inflate(e.as_str().unwrap()));
                assert_eq!(ge, we, "{name} {uid}: explored bitmask");
                g["explored"] = Value::Null;
                compared += 1;
            }
        }
        let mut want = want.clone();
        for w in want.as_object_mut().unwrap().values_mut() {
            if w.get("explored").is_some() {
                w["explored"] = Value::Null;
            }
        }
        assert_json_close(name, &got, &want);
    }
    assert!(compared > 0, "no explored maps compared");
}
