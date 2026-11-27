//! Shared test helpers: golden loading and tolerant JSON comparison.
#![allow(dead_code)]

use serde_json::Value;
use std::path::PathBuf;

/// Directory with the goldens dumped by `scripts/parity.py`, or `None` (parity tests then skip).
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
