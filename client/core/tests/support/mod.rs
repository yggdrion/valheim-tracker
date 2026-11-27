//! Shared test helpers: golden loading and tolerant JSON comparison.
#![allow(dead_code)]

use serde_json::Value;
use std::path::PathBuf;

/// Directory with the goldens dumped from the original Python implementation, or `None` (parity tests then skip).
pub fn golden_dir() -> Option<PathBuf> {
    let dir = match std::env::var_os("VT_GOLDEN") {
        Some(d) => PathBuf::from(d),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/golden"),
    };
    if dir.join("manifest.json").is_file() {
        Some(dir)
    } else {
        eprintln!("skipping parity: no goldens");
        None
    }
}

/// Same keys in the same order, same array lengths, numbers within 1e-6 relative
/// (0.1001 under `.meta.stats.`, where Python rounds half-to-even on a float32 sum).
pub fn assert_json_close(path: &str, got: &Value, want: &Value) {
    match (got, want) {
        (Value::Object(g), Value::Object(w)) => {
            let gk: Vec<&String> = g.keys().collect();
            let wk: Vec<&String> = w.keys().collect();
            assert_eq!(gk, wk, "{path}: object keys differ (or are in another order)");
            for (k, wv) in w {
                assert_json_close(&format!("{path}.{k}"), &g[k], wv);
            }
        }
        (Value::Array(g), Value::Array(w)) => {
            assert_eq!(g.len(), w.len(), "{path}: array length");
            for (i, (gv, wv)) in g.iter().zip(w).enumerate() {
                assert_json_close(&format!("{path}[{i}]"), gv, wv);
            }
        }
        (Value::Number(g), Value::Number(w)) => {
            let (g, w) = (g.as_f64().unwrap(), w.as_f64().unwrap());
            let tol = if path.contains(".meta.stats.") { 0.1001 } else { 1e-6 * w.abs().max(1.0) };
            assert!((g - w).abs() <= tol, "{path}: got {g}, want {w}");
        }
        _ => assert_eq!(got, want, "{path}"),
    }
}

use indexmap::IndexMap;

/// Builds a synthetic Valheim 1.0 `.fch` file (layout as the game's `PlayerProfile.SavePlayerToDisk`).
pub struct FchBuilder {
    version: i32,
    stats: Vec<Vec<f32>>,
    dicts: Vec<IndexMap<String, IndexMap<String, f32>>>,
    worlds: Vec<(i64, [f32; 3], Option<Vec<u8>>)>,
}

const DICT_FIELDS: [&str; 9] = [
    "worlds", "world_keys", "commands", "kills", "picked_up", "crafted", "pickables", "foods_eaten",
    "pieces_built",
];

impl FchBuilder {
    pub fn new(version: i32) -> Self {
        FchBuilder {
            version,
            stats: vec![vec![0.0; 205]; 10],
            dicts: vec![IndexMap::new(); 10],
            worlds: Vec::new(),
        }
    }

    pub fn stat(mut self, set: usize, index: usize, value: f32) -> Self {
        self.stats[set][index] = value;
        self
    }

    /// `field` is a `StatSet` dict field; `"kills"` writes the "all" kill dict.
    pub fn dict(mut self, set: usize, field: &str, key: &str, value: f32) -> Self {
        assert!(DICT_FIELDS.contains(&field), "unknown dict field {field}");
        self.dicts[set].entry(field.to_string()).or_default().insert(key.to_string(), value);
        self
    }

    pub fn world(mut self, uid: i64, home: [f32; 3], map: Option<Vec<u8>>) -> Self {
        self.worlds.push((uid, home, map));
        self
    }

    pub fn build(&self) -> Vec<u8> {
        let mut p = Vec::new();
        for v in [self.version, 205, 10] {
            p.extend(v.to_le_bytes());
        }
        let empty = IndexMap::new();
        for (set, stats) in self.stats.iter().enumerate() {
            for s in stats {
                p.extend(s.to_le_bytes());
            }
            let d = |f: &str| self.dicts[set].get(f).unwrap_or(&empty);
            for f in ["worlds", "world_keys", "commands"] {
                put_dict(&mut p, d(f));
            }
            p.extend(5i32.to_le_bytes());
            put_dict(&mut p, d("kills"));
            for _ in 0..4 {
                put_dict(&mut p, &empty);
            }
            for f in ["picked_up", "crafted", "pickables", "foods_eaten", "pieces_built"] {
                put_dict(&mut p, d(f));
            }
        }
        p.push(0); // first spawn
        p.extend((self.worlds.len() as i32).to_le_bytes());
        for (uid, home, map) in &self.worlds {
            p.extend(uid.to_le_bytes());
            for _ in 0..3 {
                p.push(0);
                p.extend([0u8; 12]);
            }
            for c in home {
                p.extend(c.to_le_bytes());
            }
            match map {
                Some(m) => {
                    p.push(1);
                    p.extend((m.len() as i32).to_le_bytes());
                    p.extend(m);
                }
                None => p.push(0),
            }
        }
        put_str(&mut p, "Test");
        p.extend(1i64.to_le_bytes());
        put_str(&mut p, "seed");
        p.push(0);
        p.extend(0i64.to_le_bytes());
        p.push(0);
        let mut out = (p.len() as i32).to_le_bytes().to_vec();
        out.extend(p);
        out.extend(0i32.to_le_bytes()); // empty hash
        out
    }
}

fn put_str(p: &mut Vec<u8>, s: &str) {
    let mut n = s.len();
    loop {
        let b = (n & 0x7f) as u8;
        n >>= 7;
        if n == 0 {
            p.push(b);
            break;
        }
        p.push(b | 0x80);
    }
    p.extend(s.as_bytes());
}

fn put_dict(p: &mut Vec<u8>, d: &IndexMap<String, f32>) {
    p.extend((d.len() as i32).to_le_bytes());
    for (k, v) in d {
        put_str(p, k);
        p.extend(v.to_le_bytes());
    }
}
