//! Reader for a Valheim 1.0 character file (`.fch`); port of `fch.py`.
//!
//! File: int32 payload length | payload | int32 hash length | hash. Payload (little-endian; strings are 7-bit
//! length prefixed UTF-8; a "dict" is int32 count + (string key, float32 value) pairs):
//! version, stat count (205), set count (10), per set: stats, worlds, world keys, commands, 5 kill dicts,
//! picked up, crafted, pickables, foods eaten, pieces placed; then first spawn, per-world data.
//! Every read is bounds-checked: a truncated or garbled file is an error, never a panic.

use indexmap::IndexMap;

pub const MIN_VERSION: i32 = 46;
pub const TESTED_VERSION: i32 = 47;
pub const STAT_COUNT: usize = 205;
pub const SET_COUNT: usize = 10;
pub const SET_RAW: usize = 0;
pub const SET_ANY: usize = 1;
pub const SET_NORMAL: usize = 6;
pub const SET_HARD: usize = 7;
pub const KILL_KINDS: [&str; 5] = ["all", "unarmed", "magic", "ranged", "melee"];

pub type Dict = IndexMap<String, f32>;

#[derive(Debug, Clone, PartialEq)]
pub struct SaveError(pub String);

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SaveError {}

fn unreadable() -> SaveError {
    SaveError("not a readable Valheim .fch file".into())
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct StatSet {
    pub stats: Vec<f32>,
    pub worlds: Dict,
    pub world_keys: Dict,
    pub commands: Dict,
    pub kills: Dict,
    pub kills_by: IndexMap<String, Dict>,
    pub picked_up: Dict,
    pub crafted: Dict,
    pub pickables: Dict,
    pub foods_eaten: Dict,
    pub pieces_built: Dict,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Parsed {
    pub version: i32,
    pub sets: Vec<StatSet>,
    /// Offset (into the payload) of the per-world data.
    pub rest: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorldEntry {
    pub uid: i64,
    pub spawn: Option<[f32; 3]>,
    pub logout: Option<[f32; 3]>,
    pub death: Option<[f32; 3]>,
    pub home: [f32; 3],
    pub map: Option<Vec<u8>>,
}

struct Reader<'a> {
    d: &'a [u8],
    p: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], SaveError> {
        let end = self.p.checked_add(n).ok_or_else(unreadable)?;
        let s = self.d.get(self.p..end).ok_or_else(unreadable)?;
        self.p = end;
        Ok(s)
    }

    fn arr<const N: usize>(&mut self) -> Result<[u8; N], SaveError> {
        self.take(N)?.try_into().map_err(|_| unreadable())
    }

    fn i32(&mut self) -> Result<i32, SaveError> {
        Ok(i32::from_le_bytes(self.arr()?))
    }
    fn i64(&mut self) -> Result<i64, SaveError> {
        Ok(i64::from_le_bytes(self.arr()?))
    }
    fn f32(&mut self) -> Result<f32, SaveError> {
        Ok(f32::from_le_bytes(self.arr()?))
    }
    fn bool(&mut self) -> Result<bool, SaveError> {
        Ok(self.arr::<1>()?[0] != 0)
    }
    fn vec3(&mut self) -> Result<[f32; 3]> {
        Ok([self.f32()?, self.f32()?, self.f32()?])
    }

    fn string(&mut self) -> Result<String, SaveError> {
        let (mut n, mut shift) = (0usize, 0u32);
        loop {
            let b = self.arr::<1>()?[0];
            if shift > 28 {
                return Err(unreadable());
            }
            n |= ((b & 0x7f) as usize) << shift;
            shift += 7;
            if b < 0x80 {
                break;
            }
        }
        let bytes = self.take(n)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| unreadable())
    }

    fn bytes(&mut self) -> Result<Vec<u8>, SaveError> {
        let n = usize::try_from(self.i32()?).map_err(|_| unreadable())?;
        Ok(self.take(n)?.to_vec())
    }

    fn dict(&mut self) -> Result<Dict, SaveError> {
        let mut out = Dict::new();
        for _ in 0..self.i32()?.max(0) {
            let k = self.string()?;
            out.insert(k, self.f32()?);
        }
        Ok(out)
    }
}

type Result<T, E = SaveError> = std::result::Result<T, E>;

fn payload(raw: &[u8]) -> Result<&[u8]> {
    let bad = || SaveError("not a Valheim character file (bad length)".into());
    let head: [u8; 4] = raw.get(..4).and_then(|h| h.try_into().ok()).ok_or_else(bad)?;
    let n = i32::from_le_bytes(head);
    if n <= 0 || n as usize > raw.len() - 4 {
        return Err(bad());
    }
    Ok(&raw[4..4 + n as usize])
}

pub fn parse(raw: &[u8]) -> Result<Parsed> {
    let mut r = Reader { d: payload(raw)?, p: 0 };
    let version = r.i32()?;
    if !(1..=1000).contains(&version) {
        return Err(SaveError("not a Valheim character file".into()));
    }
    if version < MIN_VERSION {
        return Err(SaveError(format!(
            "this save uses profile version {version}; the tracker reads version {MIN_VERSION} and newer (Valheim 1.0): load and save the character once in a current game"
        )));
    }
    let (n_stats, n_sets) = (r.i32()?, r.i32()?);
    if (n_stats, n_sets) != (STAT_COUNT as i32, SET_COUNT as i32) {
        return Err(SaveError(format!(
            "this save has {n_stats} stats in {n_sets} sets, the tracker expects {STAT_COUNT} in {SET_COUNT}: the game's save layout changed, the tracker needs an update"
        )));
    }
    let mut sets = Vec::with_capacity(SET_COUNT);
    for _ in 0..SET_COUNT {
        let mut stats = Vec::with_capacity(STAT_COUNT);
        for _ in 0..STAT_COUNT {
            stats.push(r.f32()?);
        }
        let (worlds, world_keys, commands) = (r.dict()?, r.dict()?, r.dict()?);
        let mut kills = Vec::new();
        for _ in 0..r.i32()?.max(0) {
            kills.push(r.dict()?);
        }
        let first = kills.first().cloned().ok_or_else(unreadable)?;
        let kills_by = KILL_KINDS.iter().map(|k| k.to_string()).zip(kills).collect();
        sets.push(StatSet {
            stats,
            worlds,
            world_keys,
            commands,
            kills: first,
            kills_by,
            picked_up: r.dict()?,
            crafted: r.dict()?,
            pickables: r.dict()?,
            foods_eaten: r.dict()?,
            pieces_built: r.dict()?,
        });
    }
    Ok(Parsed { version, sets, rest: r.p })
}

pub fn worlds(raw: &[u8], parsed: &Parsed) -> Result<Vec<WorldEntry>> {
    let mut r = Reader { d: payload(raw)?, p: parsed.rest };
    r.bool()?; // first spawn
    let mut out = Vec::new();
    for _ in 0..r.i32()?.max(0) {
        let uid = r.i64()?;
        let mut opt = || -> Result<Option<[f32; 3]>> {
            let has = r.bool()?;
            let v = r.vec3()?;
            Ok(has.then_some(v))
        };
        let (spawn, logout, death) = (opt()?, opt()?, opt()?);
        let home = r.vec3()?;
        let map = if r.bool()? { Some(r.bytes()?) } else { None };
        out.push(WorldEntry { uid, spawn, logout, death, home, map });
    }
    Ok(out)
}

/// `"$item_wood"` -> `"wood"` (fallback display name).
pub fn strip(token: &str) -> &str {
    for pre in ["$enemy_", "$item_", "$piece_"] {
        if let Some(rest) = token.strip_prefix(pre) {
            return rest;
        }
    }
    token
}
