//! The repo's data files, embedded at build time, and the tables parsed from them.

use indexmap::IndexMap;
use serde_json::Value;
use std::sync::LazyLock;

const GAMEDATA_RAW: &str = include_str!("../../../gamedata.json");
const STAT_MAP_RAW: &str = include_str!("../../../stat_map.json");
const STAT_NAMES_RAW: &str = include_str!("../../../stat_names.json");
const NAMES_RAW: &str = include_str!("../../../names.json");
const LOCATION_NAMES_RAW: &str = include_str!("../../../location_names.json");

pub const ACHIEVEMENTS_JSON: &str = include_str!("../../../achievements.json");
pub const DASHBOARD_HTML: &str = include_str!("../../../dashboard.html");
pub const WORLDGEN_JS: &str = include_str!("../../../worldgen.js");

/// Bump whenever the analysis output changes, so clients upload their characters again.
pub const CORE_VERSION: &str = "1";

pub static GAMEDATA: LazyLock<Value> = LazyLock::new(|| parse("gamedata.json", GAMEDATA_RAW));
pub static STAT_MAP: LazyLock<Value> = LazyLock::new(|| parse("stat_map.json", STAT_MAP_RAW));
pub static STAT_NAMES: LazyLock<Vec<String>> = LazyLock::new(|| parse("stat_names.json", STAT_NAMES_RAW));
pub static LOCATION_NAMES: LazyLock<Vec<String>> = LazyLock::new(|| parse("location_names.json", LOCATION_NAMES_RAW));

/// English names: names.json, then every gamedata entry token that is not in it yet.
pub static NAMES: LazyLock<IndexMap<String, String>> = LazyLock::new(|| {
    let mut names: IndexMap<String, String> = parse("names.json", NAMES_RAW);
    for section in GAMEDATA["sections"].as_array().into_iter().flatten() {
        for entry in section["entries"].as_array().into_iter().flatten() {
            if let (Some(token), Some(name)) = (entry["token"].as_str(), entry["name"].as_str()) {
                names.entry(token.to_string()).or_insert_with(|| name.to_string());
            }
        }
    }
    names
});

fn parse<T: serde::de::DeserializeOwned>(what: &str, raw: &str) -> T {
    serde_json::from_str(raw).unwrap_or_else(|e| panic!("embedded {what}: {e}"))
}

static FONTS: [(&str, &[u8]); 4] = [
    ("averia-400.woff2", include_bytes!("../../../fonts/averia-400.woff2")),
    ("averia-400i.woff2", include_bytes!("../../../fonts/averia-400i.woff2")),
    ("averia-700.woff2", include_bytes!("../../../fonts/averia-700.woff2")),
    ("cinzel.woff2", include_bytes!("../../../fonts/cinzel.woff2")),
];

pub fn font(name: &str) -> Option<&'static [u8]> {
    FONTS.iter().find(|(n, _)| *n == name).map(|(_, b)| *b)
}

/// 12 hex chars identifying the analysis code version and its tables (FNV-1a 64).
/// A stored character with another value is uploaded again by the client.
pub fn analysis_version() -> &'static str {
    static V: LazyLock<String> = LazyLock::new(|| {
        let mut h: u64 = 0xcbf29ce484222325;
        for part in [CORE_VERSION, GAMEDATA_RAW, STAT_MAP_RAW, STAT_NAMES_RAW, NAMES_RAW] {
            for b in part.bytes() {
                h = (h ^ b as u64).wrapping_mul(0x100000001b3);
            }
        }
        format!("{h:016x}")[..12].to_string()
    });
    &V
}
