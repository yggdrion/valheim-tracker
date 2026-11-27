# Local-first Tracker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port the Python runtime (`fch.py`, `analyze.py`, `mapdata.py`, `worlds.py`, `server.py`) to one Rust core that both Tracker Sync (local Hall, group proxy) and a headless `tracker-server` (Docker) use, then delete the Python runtime.

**Architecture:** `client/` becomes a Cargo workspace: `core` (parsers, analysis, state store, an HTTP-framework-free `Hall::handle`, a small tiny_http `serve`), `server` (bin `tracker-server`), `src-tauri` (the app). The app runs `Hall` on `127.0.0.1:<free port>` fed straight from the characters folder, or forwards the remote server's Hall in group mode, and shows it in an iframe next to the settings view.

**Tech Stack:** Rust (edition 2021, Rust ≥ 1.80 for `LazyLock`, ≥ 1.77 for `round_ties_even`), Tauri 2, tiny_http, ureq 2, flate2, base64, serde/serde_json (`preserve_order`), indexmap, regex. Python stays for `scripts/` only.

**Spec:** `docs/superpowers/specs/2026-11-27-local-first-tracker-design.md`

## Global Constraints

- Same numbers as the Python for the same save and world (parity tests against goldens dumped from the Python before it is deleted).
- One implementation of each piece: no second parser, no second copy of `dashboard.html`. Data JSONs, `dashboard.html`, `worldgen.js`, `fonts/*.woff2` are embedded from the repo root with `include_str!`/`include_bytes!` (e.g. `include_str!("../../../gamedata.json")` from `client/core/src/`).
- HTTP contract identical to `server.py`: routes, JSON shapes, status codes, error strings (`"bad token"`, `"bad name or size"`, `"unknown or outdated, upload again"`, `"not a readable Valheim .fch file"`, `"not found"`), `MAX_BODY = 5_000_000`, name rule `^[A-Za-z0-9_. -]{1,64}$`, `/api/map` and `/api/world` answer 200 `{"error": msg}` on failure.
- Env/flags unchanged for the server: `TRACKER_TOKEN` (required, exit "set TRACKER_TOKEN"), `WORLD_DIR`, `STALE_MINUTES` (default 10), `--port` (8080), `--host` (0.0.0.0), `--state` (state).
- State files keep the Python layout exactly (`<name>.json` = `{name, updated, av, version, worlds, sections, achievements, meta}`, `<name>.map.json` = per-world map dict), so an upgraded server still lists old characters until clients re-upload.
- JSON object order matters (meta dicts are sorted by count, the dashboard shows them in order): use `serde_json` with `preserve_order` and `IndexMap`.
- Python rounding: `round(x)` and `round(x, 1)` are half-to-even; use `f64::round_ties_even` (`(v*10).round_ties_even()/10` for one decimal). `int(x)` truncates toward zero.
- The app never writes to the game folder; the local Hall binds `127.0.0.1` only; the remote page never gets Tauri IPC.
- Commits: conventional commits on `main` (memory: commit to main, no branches; pull before commit). Commit locally after each task, **push once at the end** (Task 12) so the release happens once with a working image and app.
- Test inputs are private: never commit saves or worlds. Goldens live in `testdata/golden/` (already gitignored).

## Review Focus

1. **A save read mid-write or truncated** (local mode reads the file the game is writing): parsing returns an error, never panics; the character shows "rejected" and is retried on the next change. → Task 2 test `every_prefix_is_an_error_not_a_panic`.
2. **A character name outside the name rule** (`Åsa`, 65 chars): the local Hall rejects it with the same message as the server, visible in the character's status, not silently missing. → Task 6 test `ingest_rejects_bad_names`; Task 10 maps it to status `rejected`.
3. **No usable world** (no worlds folder, only pre-1.0 `.db`/`.fwl` files like `~/.config/unity3d/IronGate/Valheim/worlds`, no `.ok`): the Hall still shows the characters; `/api/world` gives `{"error"}`; the world list is empty. → Task 5 test `list_worlds_ignores_old_format_and_backups`; Task 7 test `world_routes_report_errors_with_200`.
4. **Group server unreachable**: the iframe shows a readable page saying the server cannot be reached and that the Local view still works, not a blank frame. → Task 10 test `proxy_unreachable_gives_502_page`.
5. **Another local process posting to the app's Hall**: `POST /upload` on the app's local server is refused (no token configured → 403), and the listener is on `127.0.0.1`. → Task 10 test `local_hall_refuses_uploads`.

---

## File Structure

```
client/Cargo.toml                 workspace: members core, server, src-tauri; Cargo.lock moves here
client/core/Cargo.toml            tracker-core
client/core/src/lib.rs            re-exports
client/core/src/data.rs           embedded files, parsed tables, analysis_version()
client/core/src/fch.rs            .fch reader (port of fch.py)
client/core/src/analyze.rs        sections, achievements, meta (port of analyze.py)
client/core/src/mapdata.rs        per-world map data (port of mapdata.py)
client/core/src/worlds.rs         world folder reader (port of worlds.py) + list_worlds
client/core/src/store.rs          state dir + seen/online (the storage half of server.py)
client/core/src/hall.rs           Hall::handle: routes of server.py, no HTTP library
client/core/src/serve.rs          tiny_http glue: serve(listener, handler)
client/core/tests/support/mod.rs  FchBuilder (synthetic saves), assert_json_close, golden loader
client/core/tests/*.rs            per-module tests + parity tests (skip when no goldens)
client/server/Cargo.toml, src/main.rs   bin tracker-server: serve | check FILE | world DIR
client/src-tauri/src/lib.rs       sync loop (+ local ingest), commands
client/src-tauri/src/local.rs     local Hall server, Mode, group proxy
client/ui/index.html              title bar tabs Hall/Settings, Local/Group switch, iframe, world picker
scripts/parity.py                 dumps goldens from the Python (Task 1; deleted in Task 12)
scripts/_tracker.py               world_json(dir) via tracker-server, stable_hash (Task 12)
Dockerfile, .dockerignore, .github/workflows/build.yml
```

---

### Task 1: Workspace, embedded data, goldens

**Files:**
- Create: `client/Cargo.toml`, `client/core/Cargo.toml`, `client/core/src/{lib.rs,data.rs}`, `client/core/tests/support/mod.rs`, `client/core/tests/data.rs`, `scripts/parity.py`
- Modify: `client/src-tauri/Cargo.toml` (workspace member), move `client/src-tauri/Cargo.lock` → `client/Cargo.lock`, `client/.gitignore` (add `target/`)

**Interfaces:**
- Produces (`tracker_core::data`):
  - `pub static GAMEDATA: LazyLock<serde_json::Value>`, `STAT_MAP`, `STAT_NAMES: LazyLock<Vec<String>>`, `NAMES: LazyLock<IndexMap<String, String>>` (names.json string values, then `setdefault` every gamedata entry token → name, as `analyze.py` does), `LOCATION_NAMES: LazyLock<Vec<String>>`
  - `pub const ACHIEVEMENTS_JSON: &str`, `DASHBOARD_HTML: &str`, `WORLDGEN_JS: &str`, `pub fn font(name: &str) -> Option<&'static [u8]>` (the four files in `fonts/`)
  - `pub fn analysis_version() -> &'static str`: 12 lowercase hex chars, FNV-1a 64 over `CORE_VERSION` (a `const &str`, start at `"1"`) followed by the bytes of gamedata, stat_map, stat_names, names. Bump `CORE_VERSION` whenever analysis output changes.
  - test support: `pub fn golden_dir() -> Option<PathBuf>` (env `VT_GOLDEN`, else `<repo>/testdata/golden`; `None` with an `eprintln!("skipping parity: no goldens")` if `manifest.json` is missing), `pub fn assert_json_close(path: &str, got: &Value, want: &Value)` (same keys **in the same order**, arrays same length, numbers equal within `1e-6 * max(1, |want|)`, except any path containing `.meta.stats.` within `0.1001`; panics with the JSON path of the first difference).

- [ ] **Step 1:** Write `scripts/parity.py dump <characters dir> <world dir> [--out testdata/golden]`. It imports the current Python modules and writes `manifest.json` `{"characters": [{"name", "path"}], "world": "<abs path>"}` (every `*.fch` without `_backup_` whose profile version ≥ 46), per character `<name>.analysis.json` = `{version, worlds, sections, achievements, meta}` exactly as `server.py` builds it (without `name/updated/av`) and `<name>.map.json` = `mapdata.worlds(raw, d)`, plus `world.json` = `worlds.read_world(dir)` and `locations.json` = `worlds.locations(dir)`.
- [ ] **Step 2:** Run it on the developer's data and check the output:
  `python3 scripts/parity.py dump ~/.local/share/Steam/userdata/59568624/892970/remote/characters ~/.local/share/Steam/userdata/59568624/892970/remote/worlds/roota`
  Expected: 6 characters in `manifest.json` (path, roota, rootcus, rooth, rootwo, sav), `git status` shows nothing under `testdata/`.
- [ ] **Step 3:** Create the workspace (`[workspace] members = ["core", "server", "src-tauri"]`, `resolver = "2"`, the release profile moves from `src-tauri/Cargo.toml` to the workspace). `server` can be an empty `fn main(){}` bin for now. `core` deps: `serde` (derive), `serde_json` (`preserve_order`), `indexmap` (serde), `flate2`, `base64`, `regex`, `tiny_http`.
- [ ] **Step 4: Failing tests** in `client/core/tests/data.rs`: `stat_names_has_205` (`STAT_NAMES.len() == 205`), `analysis_version_is_12_hex` (`len == 12`, all `[0-9a-f]`, and two calls equal), `names_include_gamedata_tokens` (every `GAMEDATA.sections[*].entries[*].token` is a key of `NAMES`), `fonts_embedded` (`font("cinzel.woff2").is_some()`, `font("x.woff2").is_none()`). Run `cd client && cargo test -p tracker-core` → FAIL (not defined).
- [ ] **Step 5:** Implement `data.rs`. Run `cargo test -p tracker-core` → PASS; `cargo build -p valheim-tracker-sync` still builds.
- [ ] **Step 6:** Commit `chore: cargo workspace with tracker-core and parity goldens script`.

### Task 2: `.fch` reader

**Files:** Create `client/core/src/fch.rs`, `client/core/tests/fch.rs`; extend `tests/support/mod.rs` with `FchBuilder`.

**Interfaces:**
- Produces (`tracker_core::fch`):
  - consts `MIN_VERSION: i32 = 46`, `TESTED_VERSION = 47`, `STAT_COUNT: usize = 205`, `SET_COUNT: usize = 10`, `SET_RAW = 0`, `SET_ANY = 1`, `SET_NORMAL = 6`, `SET_HARD = 7`, `KILL_KINDS = ["all","unarmed","magic","ranged","melee"]`
  - `pub type Dict = IndexMap<String, f32>` (file order)
  - `pub struct StatSet { pub stats: Vec<f32>, pub worlds: Dict, pub world_keys: Dict, pub commands: Dict, pub kills: Dict, pub kills_by: IndexMap<String, Dict>, pub picked_up: Dict, pub crafted: Dict, pub pickables: Dict, pub foods_eaten: Dict, pub pieces_built: Dict }`
  - `pub struct Parsed { pub version: i32, pub sets: Vec<StatSet>, pub rest: usize }`
  - `pub struct SaveError(pub String)` with `Display`; `pub fn parse(raw: &[u8]) -> Result<Parsed, SaveError>`
  - `pub struct WorldEntry { pub uid: i64, pub spawn: Option<[f32; 3]>, pub logout: Option<[f32; 3]>, pub death: Option<[f32; 3]>, pub home: [f32; 3], pub map: Option<Vec<u8>> }`; `pub fn worlds(raw: &[u8], parsed: &Parsed) -> Result<Vec<WorldEntry>, SaveError>`
  - `pub fn strip(token: &str) -> &str`
  - test support `FchBuilder::new(version: i32) -> Self`, `.stat(set, index, value)`, `.dict(set, field: &str, key, value)` (fields as in `StatSet`, `"kills"` writes the "all" kill dict), `.world(uid, home, map: Option<Vec<u8>>)`, `.build() -> Vec<u8>` (length prefix + payload + empty hash, layout as the docstring of `fch.py`).

Error strings, verbatim from `fch.py`: `"not a Valheim character file (bad length)"`, `"not a Valheim character file"`, the old-version message (`"this save uses profile version {v}; the tracker reads version 46 and newer (Valheim 1.0): load and save the character once in a current game"`), the layout message (`"this save has {n} stats in {m} sets, the tracker expects 205 in 10: the game's save layout changed, the tracker needs an update"`). Any out-of-bounds read or invalid UTF-8 → `SaveError("not a readable Valheim .fch file")`. No `unwrap`/indexing that can panic on input.

- [ ] **Step 1: Failing tests** in `tests/fch.rs`:
  - `parses_built_save`: builder v47 with set 0 stat 3 = 12.5 and set 1 `kills` `$enemy_boar` = 4 → `version == 47`, `sets.len() == 10`, `sets[0].stats[3] == 12.5`, `sets[1].kills["$enemy_boar"] == 4.0`, `sets[1].kills_by["all"]` equals `sets[1].kills`.
  - `rejects_old_version`: v45 → error equals the old-version message with 45.
  - `rejects_bad_length`: `[0xff,0xff,0xff,0x7f]` → `"not a Valheim character file (bad length)"`.
  - `every_prefix_is_an_error_not_a_panic`: for each `k in 0..full.len()` of a built save with one world, `parse(&full[..k])` and then `worlds` (when parse succeeds) return `Err` or `Ok` and never panic (the length prefix is patched to `k-4` so the bad-length check does not short-circuit).
  - `reads_worlds`: one world uid 42 home (1,2,3), map `Some(vec![9,9])` → `uid == 42`, `home == [1.,2.,3.]`, `map == Some(vec![9,9])`, `spawn == None`.
  - `strip_prefixes`: `strip("$item_wood") == "wood"`, `strip("x") == "x"`.
  - `parity_parse` (skips without goldens): for each golden character, `parse` the file; `version` equals `analysis.json.version`; `sets[0].worlds` keys sorted equal `analysis.json.worlds`.
- [ ] **Step 2:** `cargo test -p tracker-core --test fch` → FAIL.
- [ ] **Step 3:** Implement `fch.rs` with a bounds-checked `Reader` (7-bit length strings, i32/i64/f32/vec3/bool/bytes/dict) mirroring `fch.Reader`.
- [ ] **Step 4:** `cargo test -p tracker-core --test fch` → PASS (parity test runs with goldens present).
- [ ] **Step 5:** Commit `feat(core): Valheim .fch reader in Rust`.

### Task 3: Analysis

**Files:** Create `client/core/src/analyze.rs`, `client/core/tests/analyze.rs`.

**Interfaces:**
- Consumes: `fch::{Parsed, SET_RAW, strip}`, `data::{GAMEDATA, STAT_MAP, STAT_NAMES, NAMES}`.
- Produces (`tracker_core::analyze`):
  - `#[derive(Serialize, Deserialize, Clone)] pub struct Entry { pub token: String, pub name: String, pub count: i64, pub biome: serde_json::Value }`
  - `#[derive(Serialize, Deserialize, Clone)] pub struct Section { pub id: String, pub title: String, pub total: usize, pub done: usize, pub entries: Vec<Entry> }` (field order = this order, as Python)
  - `pub fn sections(d: &Parsed) -> Vec<Section>` (= `analyze_parsed(d)`, no `only`)
  - `pub fn achievements(d: &Parsed, sections: &[Section]) -> IndexMap<String, [i64; 2]>`
  - `pub fn meta(d: &Parsed) -> serde_json::Value` — keys `stats, kills, picked_up, eaten, built, harvested, worlds` in that order.

Rules to keep from `analyze.py`: section `source == "stats"` counts `int(stats[index of token in STAT_NAMES])`; else `int(dict.get(token, 0))`. Achievement sums in f64 with `min(value, goal)`, then truncate. "others" entries computed after all others: count of `done >= total`. `meta`: dicts filtered `v > 0`, sorted by `-v` stably (Python `sorted` is stable, ties keep file order), keys mapped by `_nm` (NAMES string, non-empty; else `strip(tok).replace('_',' ')` with the first letter upper-cased and the rest lower-cased, like `str.capitalize`); a collision keeps the first position and the last value (IndexMap `insert`). `stats`: non-zero values, `round(v,1)` keyed by stat name. `harvested`: key gets a space between a lowercase and an uppercase letter (`(?<=[a-z])(?=[A-Z])` — Rust `regex` has no lookbehind; insert the space by scanning chars). `worlds`: `round(v)` as integer.

- [ ] **Step 1: Failing tests:**
  - `section_counts_from_dicts`: builder with set given by the first gamedata section whose `source != "stats"`, its first entry token = 2.7 → that entry `count == 2`, section `done >= 1`, `total == entries.len()`.
  - `capitalize_fallback`: a raw kill `$enemy_greydwarf_elite` not in NAMES → `meta` kills key `"Greydwarf elite"`.
  - `meta_sorted_by_count_with_stable_ties`: raw picked_up `a=1, b=5, c=5` (unnamed tokens) → keys order `B, C, A`.
  - `harvested_spaces`: pickable `"BlueberryBush"` = 3 → key `"Blueberry Bush"`.
  - `parity_analysis` (skips without goldens): `sections`, `achievements`, `meta` serialised equal the golden's via `assert_json_close`.
- [ ] **Step 2:** `cargo test -p tracker-core --test analyze` → FAIL.
- [ ] **Step 3:** Implement `analyze.rs`.
- [ ] **Step 4:** Run → PASS, including parity for all six characters.
- [ ] **Step 5:** Commit `feat(core): achievement and saga analysis in Rust`.

### Task 4: Map data

**Files:** Create `client/core/src/mapdata.rs`, `client/core/tests/mapdata.rs`.

**Interfaces:**
- Consumes: `fch::{worlds, Parsed, WorldEntry}`.
- Produces: `pub fn worlds(raw: &[u8], parsed: &Parsed) -> IndexMap<String, serde_json::Value>` — key = uid as decimal string; value keys in order `spawn, logout, death, home` (each `[x, z]` rounded half-even, or null; `home` null when x and z are both 0), then when a map blob exists `size, explored, pins` (pins `{name, x, z, type}`). `explored` = base64 of zlib (level 9) of the bitmask (bit `i & 7` of byte `i >> 3` set when own **or** shared explored). Any failure inside → the caller treats it as "no map" (`server.py` catches everything), so return `IndexMap::new()` on error instead of `Result`.

- [ ] **Step 1: Failing tests:**
  - `bitmask_merges_own_and_shared`: blob for size 4 with own cell 0 and shared cell 9 explored, one pin "Home" at (12.5, 0, -3.5) type 1 → decoded `explored` (base64 → zlib inflate) is `[0x01, 0x02]`; pin `x == 12`, `z == -4` (half-even: 12.5 → 12, -3.5 → -4), `size == 4`.
  - `home_zero_is_null`: world with home (0, 5, 0) and no map → `home` is null and there is no `size` key.
  - `garbage_map_gives_empty`: map bytes `[1,2,3]` → empty map, no panic.
  - `parity_map` (skips without goldens): equals golden `<name>.map.json` with `explored` compared **after inflating both** (zlib output bytes may differ from Python's).
- [ ] **Step 2:** Run → FAIL. **Step 3:** Implement (gzip via `flate2::read::GzDecoder` on `blob[8..8+n]`). **Step 4:** Run → PASS.
- [ ] **Step 5:** Commit `feat(core): explored map, pins and points from saves in Rust`.

### Task 5: World folder

**Files:** Create `client/core/src/worlds.rs`, `client/core/tests/worlds.rs`.

**Interfaces:**
- Produces (`tracker_core::worlds`):
  - `pub fn latest_save(dir: &Path) -> Option<u32>`
  - `pub fn read_world(dir: &Path) -> Result<serde_json::Value, String>` — keys in order `version, name, seed, seed_int, uid (string), gen_version, modifiers, save, save_mtime, day, net_seconds, bosses, keys`; error text `"no complete _main.N.ok save in {dir}"` when no save.
  - `pub fn locations(dir: &Path) -> Vec<(String, i64, i64)>` (unknown hashes named `#%08x`)
  - `pub fn stable_hash(s: &str) -> u32`
  - `pub struct WorldDir { pub name: String, pub path: PathBuf, pub mtime: f64 }`; `pub fn list_worlds(root: &Path) -> Vec<WorldDir>` — subfolders of `root` with a complete save, skipping names containing `_backup_auto-`, newest `.db2` mtime first.
  - `pub fn worlds_root(characters_dir: &Path) -> Option<PathBuf>` — sibling `worlds_local`, else sibling `worlds`, whichever exists.

Port `global_keys`, the modifier regex (`regex::bytes`), `BOSSES`, `DAY_SEC = 1800`, the StartTemple-anchored location scan exactly as in `worlds.py`. The `.db2` body is `zlib.decompressobj(31)` (gzip-wrapped) of `d[16:]`: use `flate2::read::GzDecoder` and keep whatever was decoded when the stream ends early or has trailing bytes, as `decompressobj` does.

- [ ] **Step 1: Failing tests:**
  - `stable_hash_matches_game`: `stable_hash("StartTemple")` equals the Python value (compute once with `python3 -c "import worlds;print(worlds.stable_hash('StartTemple'))"` and put the number in the test).
  - `list_worlds_ignores_old_format_and_backups`: temp root with `roota/_main.14.{fwl2,db2,ok}` (empty files), `roota_backup_auto-20261126-010630/_main.13.{fwl2,db2,ok}`, `old/_main.2.fwl2` (no `.ok`), and loose `Linux.db`, `Linux.fwl` → exactly `["roota"]`.
  - `worlds_root_prefers_worlds_local`: both siblings exist → `worlds_local`.
  - `read_world_without_save_errors`: empty dir → `Err` starting `"no complete _main.N.ok save"`.
  - `parity_world` (skips without goldens): `read_world` equals `world.json` (`save_mtime` within 1e-3), `locations` equals `locations.json`.
- [ ] **Step 2:** Run → FAIL. **Step 3:** Implement. **Step 4:** Run → PASS.
- [ ] **Step 5:** Commit `feat(core): world folder reader in Rust`.

### Task 6: State store

**Files:** Create `client/core/src/store.rs`, `client/core/tests/store.rs`.

**Interfaces:**
- Consumes: `fch::parse`, `analyze::*`, `mapdata::worlds`, `data::analysis_version`.
- Produces (`tracker_core::store`):
  - `pub enum IngestError { Bad(String) }` (maps to HTTP 400 with that message)
  - `pub struct Store` (dir + `Mutex<HashMap<String, f64>>` seen); `pub fn open(dir: PathBuf) -> io::Result<Store>` (creates dir)
  - `pub fn ingest(&self, name: &str, raw: &[u8], at: f64) -> Result<(), IngestError>` — name rule and size check first (`"bad name or size"`), then parse/analyse; `SaveError` → its message, any other failure → `"not a readable Valheim .fch file"`; writes `<name>.map.json` then `<name>.json` atomically (tmp + rename, map first, as `server.py`); `updated = at`; sets seen.
  - `pub fn touch(&self, name: &str, at: f64) -> bool` — false when the name is invalid, unknown, or its stored `av` differs from `analysis_version()`.
  - `pub fn remove(&self, name: &str)` — deletes both files and the seen entry.
  - `pub fn characters(&self, now: f64, stale_secs: f64) -> serde_json::Value` — the `/api/characters` body: `{"characters": [{name, updated, version, worlds, seen, online, sections: [{id,title,done,total}]}], "stale_minutes": stale_secs/60}`, ordered by file name; `seen` = in-memory seen or `updated`; `online` = `now - seen_mem <= stale` (false when not in memory, like Python's `SEEN.get(name, 0)`).
  - `pub fn character(&self, name: &str) -> Option<Vec<u8>>` (raw file bytes; name rule checked)
  - `pub fn maps_for(&self, world_uid: &str) -> Vec<serde_json::Value>` — for each `*.map.json` sorted, `{"name": name}` merged with that world's entry when it has `explored`.

- [ ] **Step 1: Failing tests:**
  - `ingest_then_list`: ingest a builder save as "Ragnar Lodbrok" at t=100 → `characters(100.0, 600.0)` has one entry, `online == true`, `seen == 100.0`; `characters(800.0, 600.0)` → `online == false`.
  - `ingest_rejects_bad_names`: `"Åsa"`, 65×`"a"`, `""` → `Bad("bad name or size")`; 5_000_001 bytes → same.
  - `ingest_reports_save_errors`: v45 save → `Bad(<old-version message>)`; random bytes `[1,0,0,0,7]` → `Bad("not a readable Valheim .fch file")` or the bad-length message (either is a `SaveError` path; assert it is not a panic and is `Bad`).
  - `touch_needs_current_av`: ingest, then rewrite the stored JSON with `"av": "old"` → `touch` false; unknown name → false.
  - `reads_python_state`: write a file in the Python layout (keys as in Global Constraints, `av` "x") → `characters` lists it with `seen == updated` and `online == false`.
  - `remove_forgets`: ingest, `remove` → `characters` empty, `character` None.
- [ ] **Step 2:** Run → FAIL. **Step 3:** Implement. **Step 4:** Run → PASS.
- [ ] **Step 5:** Commit `feat(core): character state store compatible with the Python layout`.

### Task 7: Hall (routes)

**Files:** Create `client/core/src/hall.rs`, `client/core/src/serve.rs`, `client/core/tests/hall.rs`.

**Interfaces:**
- Consumes: `store::Store`, `worlds::{read_world, locations, latest_save}`, `data::*`.
- Produces (`tracker_core::hall`):
  - `pub struct Request<'a> { pub method: &'a str, pub path: &'a str, pub query: &'a str, pub token: Option<&'a str>, pub body: &'a [u8] }`
  - `pub struct Response { pub status: u16, pub content_type: &'static str, pub body: Vec<u8>, pub cache: bool }` (+ `Response::json(status, &Value)`)
  - `pub struct Hall`; `pub fn new(store: Arc<Store>, world_dir: Option<PathBuf>, token: Option<String>, stale_secs: f64) -> Hall`; `pub fn set_world_dir(&self, dir: Option<PathBuf>)`; `pub fn store(&self) -> &Arc<Store>`; `pub fn handle(&self, req: &Request) -> Response`
  - `pub fn query_param(query: &str, key: &str) -> Option<String>` (percent-decoding, `+` → space, like `parse_qs`)
- Produces (`tracker_core::serve`): `pub fn serve(listener: std::net::TcpListener, handler: Arc<dyn Fn(&Request) -> Response + Send + Sync>)` — blocking; tiny_http from the listener; one thread per request; reads `X-Token`, `Content-Length` bodies up to `MAX_BODY + 1`; sets `Content-Type`, `Content-Length`, and `Cache-Control: public, max-age=86400` when `cache`.

Routes and behaviour: exactly the `do_GET`/`do_POST` of `server.py` (see Global Constraints). Token check uses a constant-time compare; `token: None` → every POST 403 `{"error":"bad token"}`. World and locations are cached per `latest_save` number (in a `Mutex`). `/api/map` body: `{"world": {name, seed, seed_int, uid, gen_version}, "location_names": sorted unique, "locations": [[idx, x, z]], "players": store.maps_for(uid)}`.

- [ ] **Step 1: Failing tests** (call `handle` directly, temp store):
  - `upload_needs_token`: token `Some("t")`; POST `/upload?name=A` with token `"x"` → 403; with `"t"` and a builder save → 200 `{"ok":true}`; then GET `/api/characters` lists `A`.
  - `heartbeat_unknown_is_404`: POST `/heartbeat?name=__probe__` with the right token → 404 `{"error":"unknown or outdated, upload again"}`.
  - `name_is_percent_decoded`: upload with `?name=Ragnar%20Lodbrok` → GET `/api/character/Ragnar%20Lodbrok` 200. (Path decoding: decode `%20` in the path segment too.)
  - `static_routes`: `/` is `text/html; charset=utf-8` and starts with `<!doctype` (case-insensitive); `/worldgen.js` `text/javascript; charset=utf-8`; `/fonts/cinzel.woff2` `font/woff2` with `cache == true`; `/fonts/../x` → 404; `/api/achievements` equals `ACHIEVEMENTS_JSON`.
  - `world_routes_report_errors_with_200`: `world_dir: None` and a dir without saves → `/api/world` and `/api/map` 200 with an `"error"` key.
  - `serve_roundtrip`: bind `127.0.0.1:0`, spawn `serve` with a handler around `Hall`, `ureq` GET `/api/characters` → 200 JSON with `"characters"`.
- [ ] **Step 2:** Run → FAIL. **Step 3:** Implement. **Step 4:** Run → PASS.
- [ ] **Step 5:** Commit `feat(core): the Hall's HTTP routes in Rust`.

### Task 8: `tracker-server` binary

**Files:** Modify `client/server/Cargo.toml`, `client/server/src/main.rs`; Create `client/server/tests/cli.rs`.

**Interfaces:**
- Consumes: `hall::Hall`, `serve::serve`, `store::Store`, `fch`, `analyze`, `mapdata`, `worlds`.
- Produces (CLI, used by Task 9 and Task 12):
  - `tracker-server [--port N] [--host H] [--state DIR]` — env as Global Constraints; prints `listening on {host}:{port}`; exits 1 with `set TRACKER_TOKEN` when the token is empty.
  - `tracker-server check FILE.fch` — prints the sections JSON (what `python analyze.py FILE` printed), indent 1; exit 1 with the SaveError message on stderr.
  - `tracker-server world DIR` — prints `{"world": read_world, "locations": [[name, x, z]], "hashes": [[hash_u32, x, z]]}` (hashes for every location, named or not).

- [ ] **Step 1: Failing tests** (`assert_cmd`-free: `std::process::Command` on `env!("CARGO_BIN_EXE_tracker-server")`): `no_token_exits` (no env → exit code 1, stderr contains `set TRACKER_TOKEN`), `check_prints_sections` (builder save written to a temp file → stdout parses as a JSON array of length `GAMEDATA.sections.len()`), `check_rejects_garbage` (exit 1, stderr non-empty), `world_without_save_fails` (empty dir → exit 1).
- [ ] **Step 2:** Run → FAIL. **Step 3:** Implement (hand-rolled arg parsing is fine). **Step 4:** Run → PASS.
- [ ] **Step 5: Manual parity of the whole server:** run `python server.py` (port 8091, `TRACKER_TOKEN=dev`, `WORLD_DIR=<roota>`, state `/tmp/...py`) and `tracker-server` (port 8092, same, own state dir); upload the six golden characters to both with `curl --data-binary`; compare `/api/characters` (ignore `updated`, `seen`), `/api/character/<n>` (ignore `updated`, `av`), `/api/world`, `/api/map` (explored compared inflated) — a short Python snippet in the scratchpad is enough. Expected: no differences.
- [ ] **Step 6:** Commit `feat: tracker-server, the Rust Hall server`.

### Task 9: Docker image and CI

**Files:** Modify `Dockerfile`, `.dockerignore`, `.github/workflows/build.yml`.

- [ ] **Step 1:** `Dockerfile`: multi-stage. Build stage `FROM --platform=$BUILDPLATFORM rust:1-bookworm`, install `cargo-zigbuild` + zig, map `TARGETARCH` (amd64/arm64) to `x86_64-unknown-linux-musl` / `aarch64-unknown-linux-musl`, `cargo zigbuild --locked --release -p tracker-server --target …` with `client/` and the repo-root data files copied in (the embedded paths must resolve: copy `gamedata.json stat_map.json stat_names.json names.json location_names.json achievements.json dashboard.html worldgen.js fonts/` to the same relative place). Final stage `FROM gcr.io/distroless/static-debian12`, `COPY` the binary to `/app/tracker-server`, `WORKDIR /app`, `EXPOSE 8080`, `CMD ["/app/tracker-server", "--port", "8080", "--state", "/app/state"]`. Volume path `/app/state` unchanged.
- [ ] **Step 2:** `.dockerignore`: add `client/target`, `client/node_modules`, `client/src-tauri/target`, `client/src-tauri/gen`, `testdata`.
- [ ] **Step 3:** Verify: `docker build -t vt-test .` then `docker run --rm -e TRACKER_TOKEN=dev -p 8093:8080 -v <roota>:/world:ro -e WORLD_DIR=/world vt-test` and `curl -s localhost:8093/api/world` → JSON with `"name": "roota"`. Also `docker buildx build --platform linux/arm64 .` builds.
- [ ] **Step 4:** `build.yml` app job: `Swatinem/rust-cache` `workspaces: client -> target`; artifact paths `client/target/**/release/bundle/...` (the workspace moves `target/` up); add a Linux-only step before tauri-action: `cargo test --workspace --locked` (working dir `client`). Image job unchanged except it now builds Rust.
- [ ] **Step 5:** Commit `build: Rust-only tracker image`.

### Task 10: App — local Hall, ingest, group proxy

**Files:** Create `client/src-tauri/src/local.rs`; Modify `client/src-tauri/src/lib.rs`, `client/src-tauri/Cargo.toml` (dep `tracker-core = { path = "../core" }`).

**Interfaces:**
- Consumes: `hall::{Hall, Request, Response}`, `serve::serve`, `store::{Store, IngestError}`, `worlds::{list_worlds, worlds_root, WorldDir}`.
- Produces (`local.rs`):
  - `pub enum Mode { Local, Group { server: String } }`
  - `pub struct LocalHall { pub hall: Arc<Hall>, pub port: u16, pub mode: Arc<RwLock<Mode>> }`; `pub fn start(state_dir: PathBuf) -> io::Result<LocalHall>` — binds `127.0.0.1:0`, token `None`, stale from `STALE_MINUTES` default 10 min, spawns `serve`.
  - `fn handler(lh) -> impl Fn(&Request) -> Response`: `Mode::Local` → `hall.handle`; `Mode::Group` → GET forwarded with ureq to `{server}{path}?{query}` (status, content type and body passed through; `cache` for fonts); POST → 404; ureq transport error → 502 `text/html; charset=utf-8` page: "Cannot reach the group's server ({server}). The Local view still works: switch to Local in the title bar."
  - `pub fn url(&self) -> String` = `http://127.0.0.1:{port}/`
- `lib.rs` changes:
  - `Settings` gains `world: String` (empty = newest) and `view: String` (`"local"` | `"group"`; empty = group when a server is set, else local). Status gains `hall_url`, `view`, `worlds: Vec<String>`, `world`.
  - Sync loop: for every selected character whose signature changed (same settle rule), `store.ingest(name, bytes, mtime_secs)`; `Err(Bad(m))` → status `rejected` with `m` (only when no server is set; with a server the upload result wins); deselected → `store.remove`. Paused → neither ingest nor upload. Uploads unchanged.
  - World: `worlds_root(dir)` → `list_worlds`; chosen = `settings.world` if present in the list else the first; `hall.set_world_dir(Some(path))` or `None`.
  - Commands: `hall_url() -> String`, `set_view(view: String) -> Status` (updates `Mode`, saves), `set_world(name: String) -> Status`. `open_hall` unchanged (group server in the browser).
  - Local state dir: `app_data_dir()/local-state`.

- [ ] **Step 1: Failing tests** (`#[cfg(test)]` in `local.rs`, ports bound for real):
  - `local_hall_refuses_uploads`: `start(tmp)`, POST `/upload?name=A` via ureq with any `X-Token` → 403; the bound address is `127.0.0.1`.
  - `proxy_forwards_get`: start a second `serve` on `127.0.0.1:0` returning 200 `{"x":1}` for `/api/world`; set `Mode::Group` to it → GET `/api/world` on the local hall returns `{"x":1}`.
  - `proxy_unreachable_gives_502_page`: `Mode::Group { server: "http://127.0.0.1:1" }` → 502, content type html, body contains `"Cannot reach the group's server"`.
  - `ingest_shows_in_local_hall`: `hall.store().ingest("A", builder save, 1.0)` → GET `/api/characters` lists `A`.
- [ ] **Step 2:** `cargo test -p valheim-tracker-sync` → FAIL. **Step 3:** Implement `local.rs` and the `lib.rs` changes. **Step 4:** Run → PASS (existing `names` / `encodes_names` tests still pass).
- [ ] **Step 5:** Commit `feat: Tracker Sync runs the Hall itself for local play`.

### Task 11: App — Hall view, settings view, switch

**Files:** Modify `client/ui/index.html`, `client/src-tauri/tauri.conf.json`.

- [ ] **Step 1:** CSP: add `frame-src http://127.0.0.1:*`. Nothing else is framed.
- [ ] **Step 2:** Title bar: two tab buttons **Hall** / **Settings** (left of the window buttons) and, when a server is set, a two-state **Local / Group** switch calling `set_view`. Body: `<iframe id="hall">` filling `main` (no border, the Hall's own background) in the Hall view; the current panels in the Settings view, plus a **World** panel (shown when `worlds` is non-empty): a `<select>` of world names, first option "newest", calling `set_world`; "No 1.0 world found next to the characters folder." when empty. Iframe `src` = `hall_url()`; reload it on `set_view` / `set_world`, not on every status event. Start in Settings when the body has the `setup` class today (no folder), else Hall. Keep the frameless title bar, its drag region and the existing settings behaviour.
- [ ] **Step 3: Verify manually** (`cd client && bun run dev`): with no server set the Hall shows roota and the six characters from the Steam Cloud folder, map page renders; set the server to a local `tracker-server` → Group shows it; switch back to Local; stop the server → Group shows the "Cannot reach" page; in the iframe devtools console `window.__TAURI__` and `window.__TAURI_INTERNALS__` are `undefined`.
- [ ] **Step 4:** Commit `feat: the Hall inside Tracker Sync, with Local and Group views`.

### Task 12: Scripts, Python removal, docs, release

**Files:** Create `scripts/_tracker.py`; Modify `scripts/extract_game_data.py`, `scripts/extract_location_names.py`, `scripts/update_achievements.py`, `README.md`, `TODO.md`; Delete `server.py`, `analyze.py`, `fch.py`, `mapdata.py`, `worlds.py`, `scripts/parity.py`.

**Interfaces:**
- Consumes: `tracker-server world DIR` (Task 8).
- Produces (`scripts/_tracker.py`): `world_json(dir) -> dict` (runs `$TRACKER_BIN` or `client/target/release/tracker-server`, exits with "build it: cd client && cargo build --release -p tracker-server" when missing), `stable_hash(s) -> int` (copied from `worlds.py`).

- [ ] **Step 1:** Before deleting anything, run the full parity suite once more: `cd client && cargo test --workspace` with goldens present → all `parity_*` tests ran (not skipped) and passed.
- [ ] **Step 2:** `extract_game_data.py`: `w, locs = j["world"], j["locations"]` from `world_json(world)`. `extract_location_names.py`: unknown hashes from `j["hashes"]` whose name starts with `#`, `stable_hash` from `_tracker`. `update_achievements.py` `tracked()`: `{s["achievement"] for s in gamedata["sections"]} | set(stat_map)` read from the JSON files.
- [ ] **Step 3:** Verify: before Step 2, save the output of `python3 scripts/extract_location_names.py <roota>` (dry run as it is) and `python3 scripts/update_achievements.py` (dry run; with the old broken `tracked()` it crashes, so record the `trackable:` line expected from `gamedata.json` + `stat_map.json` keys instead). After Step 2 both scripts run and print the same results (`extract_location_names` the same unknown-hash count).
- [ ] **Step 4:** Delete the Python runtime files and `__pycache__`; `git grep -n "server.py\|analyze.py\|import fch\|import worlds"` → only CHANGELOG hits.
- [ ] **Step 5:** README: "How it works" (one Rust core: app runs the Hall locally, server is the same code), Players (solo needs only Tracker Sync; Local/Group switch), Development (`cargo run -p tracker-server`, `tracker-server check FILE`, `cargo test --workspace`; the parity tests skip without goldens, and goldens can only be dumped from a commit before this task), game-data section (scripts need a built `tracker-server`). TODO: move this rework to Done; keep the Windows/macOS test item and add the iframe/IPC check on those platforms.
- [ ] **Step 6:** `cd client && cargo test --workspace && cargo build --release -p tracker-server`; `docker build .` → all pass.
- [ ] **Step 7:** Commit `feat!: Python runtime replaced by the Rust core` — **check with the user before using `!`** (a major version bump); default `feat:`. Then `git pull --rebase && git push` and watch the build workflow produce one release.
