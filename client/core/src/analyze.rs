//! Achievement progress and saga counters from a parsed `.fch` (port of `analyze.py`).
use crate::data::{GAMEDATA, NAMES, STAT_MAP, STAT_NAMES};
use crate::fch::{self, Dict, Parsed, StatSet};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

#[derive(Serialize, Deserialize, Clone)]
pub struct Entry {
    pub token: String,
    pub name: String,
    pub count: i64,
    pub biome: Value,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Section {
    pub id: String,
    pub title: String,
    pub total: usize,
    pub done: usize,
    pub entries: Vec<Entry>,
}

fn dict<'a>(s: &'a StatSet, source: &str) -> &'a Dict {
    match source {
        "kills" => &s.kills,
        "picked_up" => &s.picked_up,
        "crafted" => &s.crafted,
        "pickables" => &s.pickables,
        "foods_eaten" => &s.foods_eaten,
        "pieces_built" => &s.pieces_built,
        other => panic!("gamedata.json: unknown source {other}"),
    }
}

pub fn sections(d: &Parsed) -> Vec<Section> {
    let mut out = Vec::new();
    for sec in GAMEDATA["sections"].as_array().into_iter().flatten() {
        let set = &d.sets[sec["set"].as_u64().unwrap() as usize];
        let source = sec["source"].as_str().unwrap();
        let entries: Vec<Entry> = sec["entries"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|e| {
                let token = e["token"].as_str().unwrap();
                let count = if source == "stats" {
                    let i = STAT_NAMES.iter().position(|n| n == token).unwrap_or_else(|| panic!("stat {token}"));
                    set.stats[i] as i64
                } else {
                    dict(set, source).get(token).copied().unwrap_or(0.0) as i64
                };
                Entry {
                    token: token.to_string(),
                    name: e["name"].as_str().unwrap().to_string(),
                    count,
                    biome: e["biome"].clone(),
                }
            })
            .collect();
        out.push(Section {
            id: sec["id"].as_str().unwrap().to_string(),
            title: sec["title"].as_str().unwrap().to_string(),
            total: entries.len(),
            done: entries.iter().filter(|e| e.count > 0).count(),
            entries,
        });
    }
    out
}

pub fn achievements(d: &Parsed, sections: &[Section]) -> IndexMap<String, [i64; 2]> {
    let mut out: IndexMap<String, [i64; 2]> = IndexMap::new();
    for s in GAMEDATA["sections"].as_array().into_iter().flatten() {
        if let Some(x) = sections.iter().find(|x| x.id == s["id"].as_str().unwrap_or("")) {
            out.insert(s["achievement"].as_str().unwrap().to_string(), [x.done as i64, x.total as i64]);
        }
    }
    let map = STAT_MAP.as_object().unwrap();
    for (api, e) in map {
        if e.get("others").is_some() {
            continue;
        }
        let s = &d.sets[e["set"].as_u64().unwrap() as usize];
        let (mut done, mut total) = (0.0f64, 0.0f64);
        let mut add = |value: f32, goal: f64| {
            done += (value as f64).min(goal);
            total += goal;
        };
        for pair in e["stats"].as_array().into_iter().flatten() {
            add(s.stats[pair[0].as_u64().unwrap() as usize], pair[1].as_f64().unwrap());
        }
        for (key, src) in [("enemies", &s.kills), ("crafts", &s.crafted)] {
            for pair in e[key].as_array().into_iter().flatten() {
                add(src.get(pair[0].as_str().unwrap()).copied().unwrap_or(0.0), pair[1].as_f64().unwrap());
            }
        }
        out.insert(api.clone(), [done as i64, total as i64]);
    }
    for (api, e) in map {
        if let Some(others) = e.get("others").and_then(Value::as_array) {
            let n = others
                .iter()
                .filter(|o| {
                    let v = out[o.as_str().unwrap()];
                    v[0] >= v[1]
                })
                .count();
            out.insert(api.clone(), [n as i64, others.len() as i64]);
        }
    }
    out
}

/// Python `str.capitalize`.
fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c.flat_map(|x| x.to_lowercase())).collect(),
        None => String::new(),
    }
}

fn nm(tok: &str) -> String {
    match NAMES.get(tok) {
        Some(n) if !n.is_empty() => n.clone(),
        _ => capitalize(&fch::strip(tok).replace('_', " ")),
    }
}

/// Positive entries, highest count first (stable), keys renamed by `key`, counts truncated.
fn counts(m: &Dict, key: impl Fn(&str) -> String) -> Value {
    let mut v: Vec<(&String, f32)> = m.iter().filter(|(_, v)| **v > 0.0).map(|(k, v)| (k, *v)).collect();
    v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let mut out = Map::new();
    for (k, v) in v {
        out.insert(key(k), json!(v as i64));
    }
    Value::Object(out)
}

fn space_camel(k: &str) -> String {
    let mut out = String::with_capacity(k.len() + 2);
    let mut prev_lower = false;
    for c in k.chars() {
        if prev_lower && c.is_uppercase() {
            out.push(' ');
        }
        prev_lower = c.is_lowercase();
        out.push(c);
    }
    out
}

pub fn meta(d: &Parsed) -> Value {
    let s = &d.sets[fch::SET_RAW];
    let mut stats = Map::new();
    for (n, v) in STAT_NAMES.iter().zip(&s.stats) {
        if *v != 0.0 {
            stats.insert(n.clone(), json!(((*v as f64) * 10.0).round_ties_even() / 10.0));
        }
    }
    let mut worlds = Map::new();
    for (k, v) in &s.worlds {
        worlds.insert(k.clone(), json!((*v as f64).round_ties_even() as i64));
    }
    json!({
        "stats": stats,
        "kills": counts(&s.kills, nm),
        "picked_up": counts(&s.picked_up, nm),
        "eaten": counts(&s.foods_eaten, nm),
        "built": counts(&s.pieces_built, nm),
        "harvested": counts(&s.pickables, space_camel),
        "worlds": worlds,
    })
}
