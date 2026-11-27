//! Per-world map data from a `.fch`; port of `mapdata.py`.
//!
//! The map blob: int32 version, int32 n, n bytes of gzip(int32 size, bool[size*size] explored,
//! bool[size*size] explored by others, int32 pin count, pins); pin: string name, vec3 pos, int32 type,
//! bool checked, int64 owner, string author. Map pixels are 12 m; pixel (size/2, size/2) is the world centre.

use crate::fch::{self, Parsed, Reader, SaveError};
use base64::Engine;
use flate2::{read::GzDecoder, write::ZlibEncoder, Compression};
use indexmap::IndexMap;
use serde_json::{json, Value};
use std::io::{Read, Write};

fn bad() -> SaveError {
    SaveError("unreadable map".into())
}

/// Python `round(x)`: half to even; non-finite values are an error (Python raises).
fn round(x: f32) -> Result<i64, SaveError> {
    let x = x as f64;
    if x.is_finite() {
        Ok(x.round_ties_even() as i64)
    } else {
        Err(bad())
    }
}

fn map(blob: &[u8]) -> Result<[(String, Value); 3], SaveError> {
    let mut head = Reader { d: blob, p: 0 };
    head.i32()?; // version
    let n = usize::try_from(head.i32()?).map_err(|_| bad())?;
    let gz = head.take(n)?;
    let mut z = Vec::new();
    GzDecoder::new(gz).read_to_end(&mut z).map_err(|_| bad())?;

    let mut r = Reader { d: &z, p: 0 };
    let size = r.i32()?;
    let cells = usize::try_from(size).map_err(|_| bad())?.checked_mul(size as usize).ok_or_else(bad)?;
    let own = r.take(cells)?;
    let shared = r.take(cells)?;
    let mut bits = vec![0u8; cells / 8];
    for (i, (a, b)) in own.iter().zip(shared).enumerate() {
        if a | b != 0 {
            *bits.get_mut(i >> 3).ok_or_else(bad)? |= 1 << (i & 7);
        }
    }
    let mut pins = Vec::new();
    for _ in 0..r.i32()?.max(0) {
        let name = r.string()?;
        let [x, _y, zz] = r.vec3()?;
        let typ = r.i32()?;
        r.take(1 + 8)?;
        r.string()?;
        pins.push(json!({"name": name, "x": round(x)?, "z": round(zz)?, "type": typ}));
    }
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::new(9));
    enc.write_all(&bits).map_err(|_| bad())?;
    let packed = enc.finish().map_err(|_| bad())?;
    Ok([
        ("size".into(), json!(size)),
        ("explored".into(), json!(base64::engine::general_purpose::STANDARD.encode(packed))),
        ("pins".into(), Value::Array(pins)),
    ])
}

fn world(w: &fch::WorldEntry) -> Result<Value, SaveError> {
    let xz = |v: Option<[f32; 3]>| -> Result<Value, SaveError> {
        Ok(match v {
            Some(v) => json!([round(v[0])?, round(v[2])?]),
            None => Value::Null,
        })
    };
    let mut m = serde_json::Map::new();
    m.insert("spawn".into(), xz(w.spawn)?);
    m.insert("logout".into(), xz(w.logout)?);
    m.insert("death".into(), xz(w.death)?);
    let home = if (w.home[0], w.home[2]) != (0.0, 0.0) { xz(Some(w.home))? } else { Value::Null };
    m.insert("home".into(), home);
    if let Some(blob) = w.map.as_deref().filter(|b| !b.is_empty()) {
        m.extend(map(blob)?);
    }
    Ok(Value::Object(m))
}

fn try_worlds(raw: &[u8], parsed: &Parsed) -> Result<IndexMap<String, Value>, SaveError> {
    let mut out = IndexMap::new();
    for w in fch::worlds(raw, parsed)? {
        out.insert(w.uid.to_string(), world(&w)?);
    }
    Ok(out)
}

/// `{world uid: {spawn, logout, death, home: [x, z] or null, size, explored (b64 zlib bitmask), pins}}`.
/// Any failure means "no map" (as `server.py` treats it), so the result is then empty.
pub fn worlds(raw: &[u8], parsed: &Parsed) -> IndexMap<String, Value> {
    try_worlds(raw, parsed).unwrap_or_default()
}
