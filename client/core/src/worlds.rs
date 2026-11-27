//! Read-only parser for a Valheim 1.0 world folder (`_main.N.fwl2` / `.db2` / `.ok`); port of `worlds.py`.
//! Gives: name, seed, in-game day, global keys (boss kills etc.), world modifiers, placed locations.

use crate::data::LOCATION_NAMES;
use regex::bytes::Regex;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

const DAY_SEC: f64 = 1800.0; // default Valheim day length, not read from the save
const BOSSES: [(&str, &str); 7] = [
    ("defeated_eikthyr", "Eikthyr"),
    ("defeated_gdking", "The Elder"),
    ("defeated_bonemass", "Bonemass"),
    ("defeated_dragon", "Moder"),
    ("defeated_goblinking", "Yagluth"),
    ("defeated_queen", "The Queen"),
    ("defeated_fader", "Fader"),
];

static MODIFIER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?-u)[\x05-\x40]([a-zA-Z][a-zA-Z_]{2,30}) ([a-z0-9_.:]{1,80})").unwrap());
static PRESET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?-u)preset ([a-z_:]+)").unwrap());
static LOC_NAMES: LazyLock<HashMap<u32, &'static str>> =
    LazyLock::new(|| LOCATION_NAMES.iter().map(|n| (stable_hash(n), n.as_str())).collect());

/// Highest N that has an `.ok` marker (the game writes `.ok` last, so it is a complete save).
pub fn latest_save(dir: &Path) -> Option<u32> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let n = name.strip_prefix("_main.")?.strip_suffix(".ok")?;
            if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            n.parse().ok()
        })
        .max()
}

fn i32_at(d: &[u8], pos: usize) -> Option<i32> {
    Some(i32::from_le_bytes(d.get(pos..pos.checked_add(4)?)?.try_into().ok()?))
}

/// Python's `^[A-Za-z0-9_ .:-]{1,80}$` (where `$` also matches before one trailing newline).
fn is_key_string(s: &[u8]) -> bool {
    let s = s.strip_suffix(b"\n").unwrap_or(s);
    (1..=80).contains(&s.len()) && s.iter().all(|b| b.is_ascii_alphanumeric() || b"_ .:-".contains(b))
}

/// Length-prefixed (<128) strings from `pos`: the list, or `None` if it is not a clean list.
fn strings(raw: &[u8], mut pos: usize) -> Option<Vec<String>> {
    let n = i32_at(raw, pos)?;
    if !(0 < n && n < 500) {
        return None;
    }
    pos += 4;
    let mut out = Vec::new();
    for _ in 0..n {
        let ln = *raw.get(pos)? as usize;
        let s = raw.get(pos + 1..).map(|r| &r[..ln.min(r.len())]).unwrap_or(&[]);
        if ln >= 128 || !is_key_string(s) {
            return None;
        }
        out.push(String::from_utf8_lossy(s).into_owned());
        pos += 1 + ln;
    }
    Some(out)
}

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if from > hay.len() {
        return None;
    }
    hay[from..].windows(needle.len()).position(|w| w == needle).map(|p| p + from)
}

/// The ZoneSystem key list: int32 count + strings, always containing `activebosses N`.
fn global_keys(raw: &[u8]) -> Vec<String> {
    let mut at = find(raw, b"activebosses ", 0);
    while let Some(i) = at {
        for start in i.saturating_sub(5)..i {
            // the count sits right before the first string's length byte
            for pos in start.saturating_sub(3)..=start {
                if let Some(keys) = strings(raw, pos) {
                    if keys.iter().any(|k| k.starts_with("activebosses")) {
                        return keys;
                    }
                }
            }
        }
        at = find(raw, b"activebosses ", i + 1);
    }
    Vec::new()
}

fn read_fwl2(path: &Path) -> Result<Map<String, Value>, String> {
    let d = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let bad = || format!("{}: truncated world file", path.display());
    let ver = i32_at(&d, 4).ok_or_else(bad)?;
    let mut pos = 8usize;
    let mut s = || -> Result<String, String> {
        let ln = *d.get(pos).ok_or_else(bad)? as usize;
        let b = d.get(pos + 1..pos + 1 + ln).ok_or_else(bad)?;
        pos += 1 + ln;
        Ok(String::from_utf8_lossy(b).into_owned())
    };
    let (name, seed) = (s()?, s()?);
    let tail = d.get(pos..pos + 16).ok_or_else(bad)?;
    let seed_int = i32::from_le_bytes(tail[0..4].try_into().unwrap());
    let uid = i64::from_le_bytes(tail[4..12].try_into().unwrap());
    let gen_version = i32::from_le_bytes(tail[12..16].try_into().unwrap());
    let mut mods = Map::new();
    for m in MODIFIER.captures_iter(&d[pos..]) {
        let whole = m.get(0).unwrap().as_bytes();
        if whole[0] as usize == whole.len() - 1 {
            let key = String::from_utf8_lossy(&m[1]).to_lowercase();
            mods.insert(key, Value::String(String::from_utf8_lossy(&m[2]).into_owned()));
        }
    }
    if let Some(p) = PRESET.captures(&d) {
        mods.insert("preset".into(), Value::String(String::from_utf8_lossy(&p[1]).into_owned()));
    }
    let mut out = Map::new();
    out.insert("version".into(), ver.into());
    out.insert("name".into(), name.into());
    out.insert("seed".into(), seed.into());
    out.insert("seed_int".into(), seed_int.into());
    out.insert("uid".into(), uid.to_string().into());
    out.insert("gen_version".into(), gen_version.into());
    out.insert("modifiers".into(), Value::Object(mods));
    Ok(out)
}

/// Decompressed body of a `.db2` (gzip stream at offset 16). Like Python's `decompressobj`, keeps
/// whatever was decoded when the stream ends early or has trailing bytes.
fn db2_body(d: &[u8]) -> Vec<u8> {
    let mut raw = Vec::new();
    if let Some(z) = d.get(16..) {
        let _ = flate2::read::GzDecoder::new(z).read_to_end(&mut raw);
    }
    raw
}

pub fn read_world(dir: &Path) -> Result<Value, String> {
    let n = latest_save(dir).ok_or_else(|| format!("no complete _main.N.ok save in {}", dir.display()))?;
    let mut out = read_fwl2(&dir.join(format!("_main.{n}.fwl2")))?;
    let db2 = dir.join(format!("_main.{n}.db2"));
    let d = std::fs::read(&db2).map_err(|e| format!("{}: {e}", db2.display()))?;
    let net = f64::from_le_bytes(d.get(4..12).ok_or_else(|| format!("{}: truncated", db2.display()))?.try_into().unwrap());
    let keys = global_keys(&db2_body(&d));
    let kset: Vec<String> = keys.iter().map(|k| k.to_lowercase()).collect();
    let bosses: Vec<Value> = BOSSES
        .iter()
        .map(|(k, nm)| json!({"key": k, "name": nm, "defeated": kset.iter().any(|x| x == k)}))
        .collect();
    let mut rest: Vec<&String> = keys.iter().filter(|k| !BOSSES.iter().any(|(b, _)| *b == k.to_lowercase())).collect();
    rest.sort();
    out.insert("save".into(), n.into());
    out.insert("save_mtime".into(), mtime(&db2).into());
    out.insert("day".into(), (((net / DAY_SEC).floor()) as i64 + 1).into());
    out.insert("net_seconds".into(), json!(net));
    out.insert("bosses".into(), Value::Array(bosses));
    out.insert("keys".into(), json!(rest));
    Ok(Value::Object(out))
}

/// The game's `string.GetStableHashCode`.
pub fn stable_hash(s: &str) -> u32 {
    let (mut h1, mut h2) = (5381u32, 5381u32);
    let cs: Vec<u32> = s.chars().map(|c| c as u32).collect();
    for pair in cs.chunks(2) {
        h1 = (h1.wrapping_shl(5).wrapping_add(h1)) ^ pair[0];
        if let Some(&c) = pair.get(1) {
            h2 = (h2.wrapping_shl(5).wrapping_add(h2)) ^ c;
        }
    }
    h1.wrapping_add(h2.wrapping_mul(1566083941))
}

/// Placed locations of the latest save: `(name, x, z)` (boss altars, traders, dungeons, ruins, ...).
/// The db2 holds int32 count + (uint32 name hash, vec3 pos, bool placed) per location; every world has
/// exactly one StartTemple, which anchors the search.
pub fn locations(dir: &Path) -> Vec<(String, i64, i64)> {
    let Some(n) = latest_save(dir) else { return Vec::new() };
    let Ok(d) = std::fs::read(dir.join(format!("_main.{n}.db2"))) else { return Vec::new() };
    let raw = db2_body(&d);
    let Some(i) = find(&raw, &stable_hash("StartTemple").to_le_bytes(), 0) else { return Vec::new() };
    let f32_at = |p: usize| f32::from_le_bytes(raw[p..p + 4].try_into().unwrap());
    let ok = |q: usize| -> bool {
        if q + 17 > raw.len() || raw[q + 16] > 1 {
            return false;
        }
        let (x, y, z) = (f32_at(q + 4), f32_at(q + 8), f32_at(q + 12));
        x.abs() < 11000.0 && z.abs() < 11000.0 && -500.0 < y && y < 3000.0
    };
    let mut start = i;
    while start >= 17 && ok(start - 17) {
        start -= 17;
    }
    let Some(count) = start.checked_sub(4).and_then(|p| i32_at(&raw, p)) else { return Vec::new() };
    if count <= 0 || count as usize > (raw.len() - start) / 17 {
        return Vec::new();
    }
    (0..count as usize)
        .map(|k| {
            let p = start + 17 * k;
            let h = u32::from_le_bytes(raw[p..p + 4].try_into().unwrap());
            let name = LOC_NAMES.get(&h).map(|s| s.to_string()).unwrap_or_else(|| format!("#{h:08x}"));
            (name, (f32_at(p + 4) as f64).round_ties_even() as i64, (f32_at(p + 12) as f64).round_ties_even() as i64)
        })
        .collect()
}

fn mtime(p: &Path) -> f64 {
    std::fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0.0, |d| d.as_secs_f64())
}

pub struct WorldDir {
    pub name: String,
    pub path: PathBuf,
    pub mtime: f64,
}

/// Subfolders of `root` with a complete save (not auto-backups), newest `.db2` first.
pub fn list_worlds(root: &Path) -> Vec<WorldDir> {
    let Ok(rd) = std::fs::read_dir(root) else { return Vec::new() };
    let mut out: Vec<WorldDir> = rd
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            let name = e.file_name().into_string().ok()?;
            if !path.is_dir() || name.contains("_backup_auto-") {
                return None;
            }
            let n = latest_save(&path)?;
            let mtime = mtime(&path.join(format!("_main.{n}.db2")));
            Some(WorldDir { name, path, mtime })
        })
        .collect();
    out.sort_by(|a, b| b.mtime.total_cmp(&a.mtime).then_with(|| a.name.cmp(&b.name)));
    out
}

/// The worlds folder next to the characters folder: `worlds_local`, else `worlds`, whichever exists.
pub fn worlds_root(characters_dir: &Path) -> Option<PathBuf> {
    let parent = characters_dir.parent()?;
    ["worlds_local", "worlds"].iter().map(|n| parent.join(n)).find(|p| p.is_dir())
}
