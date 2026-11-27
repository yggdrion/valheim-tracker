//! Character state directory (the storage half of `server.py`): analysed saves on disk,
//! last-contact times in memory.
use crate::{analyze, data, fch, mapdata};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::sync::Mutex;

pub const MAX_BODY: usize = 5_000_000;

#[derive(Debug, Clone, PartialEq)]
pub enum IngestError {
    Bad(String),
}

pub fn valid_name(name: &str) -> bool {
    (1..=64).contains(&name.chars().count())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | ' ' | '-'))
}

pub struct Store {
    dir: PathBuf,
    seen: Mutex<HashMap<String, f64>>,
    /// Serialises writes (ingest, touch, remove) so tmp files and `seen` never race.
    io: Mutex<()>,
}

/// Runs `f`, turning a panic into `None`.
pub fn guarded<T>(f: impl FnOnce() -> T) -> Option<T> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).ok()
}

impl Store {
    pub fn open(dir: PathBuf) -> io::Result<Store> {
        std::fs::create_dir_all(&dir)?;
        Ok(Store { dir, seen: Mutex::new(HashMap::new()), io: Mutex::new(()) })
    }

    fn path(&self, file: &str) -> PathBuf {
        self.dir.join(file)
    }

    fn load(&self, name: &str) -> Option<Value> {
        serde_json::from_slice(&std::fs::read(self.path(&format!("{name}.json"))).ok()?).ok()
    }

    pub fn ingest(&self, name: &str, raw: &[u8], at: f64) -> Result<(), IngestError> {
        if !valid_name(name) || raw.is_empty() || raw.len() > MAX_BODY {
            return Err(IngestError::Bad("bad name or size".into()));
        }
        let parsed = fch::parse(raw).map_err(|e| IngestError::Bad(e.0))?;
        let unreadable = || IngestError::Bad("not a readable Valheim .fch file".into());
        let (secs, ach, meta) = guarded(|| {
            let secs = analyze::sections(&parsed);
            let ach = analyze::achievements(&parsed, &secs);
            (secs, ach, analyze::meta(&parsed))
        })
        .ok_or_else(unreadable)?;
        let maps = guarded(|| mapdata::worlds(raw, &parsed)).unwrap_or_default();
        let mut worlds: Vec<&String> = parsed.sets[0].worlds.keys().collect();
        worlds.sort();
        let doc = json!({
            "name": name, "updated": at, "av": data::analysis_version(), "version": parsed.version,
            "worlds": worlds, "sections": secs, "achievements": ach, "meta": meta,
        });
        let tmp = self.path(&format!("{name}.tmp"));
        let tmpm = self.path(&format!("{name}.tmpm"));
        let _g = self.io.lock().unwrap();
        let write = || -> io::Result<()> {
            std::fs::write(&tmp, serde_json::to_vec(&doc).unwrap())?;
            std::fs::write(&tmpm, serde_json::to_vec(&maps).unwrap())?;
            std::fs::rename(&tmpm, self.path(&format!("{name}.map.json")))?;
            std::fs::rename(&tmp, self.path(&format!("{name}.json")))
        };
        if let Err(e) = write() {
            let _ = std::fs::remove_file(&tmp);
            let _ = std::fs::remove_file(&tmpm);
            return Err(IngestError::Bad(format!("could not store the character: {e}")));
        }
        self.seen.lock().unwrap().insert(name.to_string(), at);
        Ok(())
    }

    pub fn touch(&self, name: &str, at: f64) -> bool {
        if !valid_name(name) {
            return false;
        }
        let _g = self.io.lock().unwrap();
        match self.load(name) {
            Some(c) if c["av"].as_str() == Some(data::analysis_version()) => {
                self.seen.lock().unwrap().insert(name.to_string(), at);
                true
            }
            _ => false,
        }
    }

    pub fn remove(&self, name: &str) {
        let _g = self.io.lock().unwrap();
        if valid_name(name) {
            let _ = std::fs::remove_file(self.path(&format!("{name}.json")));
            let _ = std::fs::remove_file(self.path(&format!("{name}.map.json")));
        }
        self.seen.lock().unwrap().remove(name);
    }

    /// File names in the state dir ending in `suffix`, sorted.
    fn files(&self, suffix: &str) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|f| f.ends_with(suffix))
            .collect();
        v.sort();
        v
    }

    pub fn characters(&self, now: f64, stale_secs: f64) -> Value {
        let seen = self.seen.lock().unwrap().clone();
        let mut out = Vec::new();
        for f in self.files(".json") {
            if f.ends_with(".map.json") {
                continue;
            }
            let Some(c) = self.load(&f[..f.len() - 5]) else { continue };
            let name = c["name"].as_str().unwrap_or_default().to_string();
            let mem = seen.get(&name).copied();
            let mut o = Map::new();
            for k in ["name", "updated", "version", "worlds"] {
                o.insert(k.into(), c[k].clone());
            }
            o.insert("seen".into(), match mem {
                Some(t) if t != 0.0 => json!(t),
                _ => c["updated"].clone(),
            });
            o.insert("online".into(), json!(now - mem.unwrap_or(0.0) <= stale_secs));
            let secs: Vec<Value> = c["sections"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|s| json!({"id": s["id"], "title": s["title"], "done": s["done"], "total": s["total"]}))
                        .collect()
                })
                .unwrap_or_default();
            o.insert("sections".into(), Value::Array(secs));
            out.push(Value::Object(o));
        }
        json!({"characters": out, "stale_minutes": stale_secs / 60.0})
    }

    pub fn character(&self, name: &str) -> Option<Vec<u8>> {
        if !valid_name(name) {
            return None;
        }
        std::fs::read(self.path(&format!("{name}.json"))).ok()
    }

    pub fn maps_for(&self, world_uid: &str) -> Vec<Value> {
        let mut out = Vec::new();
        for f in self.files(".map.json") {
            let name = &f[..f.len() - 9];
            let Some(m) = std::fs::read(self.path(&f)).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()) else {
                continue;
            };
            let Some(w) = m.get(world_uid).and_then(Value::as_object) else { continue };
            if w.get("explored").map_or(true, |e| e.is_null() || e == &json!(false) || e == &json!(0) || e == &json!("") || e == &json!([]) || e == &json!({})) {
                continue;
            }
            let mut o = Map::new();
            o.insert("name".into(), json!(name));
            o.extend(w.clone());
            out.push(Value::Object(o));
        }
        out
    }
}
