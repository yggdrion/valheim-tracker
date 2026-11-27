# Local-first tracker: one Rust core for the app and the server

Date: 2026-11-27. Status: approved approach (A), design for review.

## Intent

Playing alone on your own PC should need nothing but Tracker Sync: the app reads the characters and the local
world itself and shows the Hall in its own window, no Docker, no server. A remote server stays optional, for the
group: with one configured, the app shows the group's Hall from that server and keeps uploading to it.

Python leaves the runtime. The data extraction scripts in `scripts/` stay Python (they run rarely, after a game
patch, and their JSON output is the contract).

## Goals

- Tracker Sync works fully offline for solo play: Hall, Road, achievements, warrior pages, map.
- One implementation of every piece of logic: one save/world parser, one analysis, one API, one `dashboard.html`.
  The app and the server run the same code; nothing exists twice in two languages or two copies of the page.
- The server is the same Rust core as a headless binary; the Docker image contains no Python.
- The server keeps serving the Hall to browsers and phones (the dashboard's phone layout stays useful).
- Existing installs keep working: same `docker-compose` variables, same upload/heartbeat protocol.

## Non-goals

- Changing what the dashboard shows or how it looks (this is a port and a re-plumbing, not a redesign).
- Porting `scripts/` (extraction from the game files) to Rust.
- TLS (still a reverse proxy's job), accounts, per-user auth.
- Merging a local solo world into the group view: the group view is exactly what the server has.

## Anti-goals (would make this a failure even if it works)

- Numbers that differ from today's Python output for the same save and world.
- A second copy of the dashboard or of the parser to keep in sync.
- The app ever writing to the game folder.
- A remote page getting access to the app's Tauri commands.
- Leaderboard / best-player framing creeping in (co-op only, as now).

## Constraints

- Client stays Rust/Tauri 2 (current: `client/src-tauri`, ureq, frameless window with its own title bar in
  `client/ui/index.html`, signed self-update).
- Releases: push to `main` → semantic-release builds the image and the installers (`.github/workflows/build.yml`).
- Data files consumed at runtime: `gamedata.json`, `stat_map.json`, `stat_names.json`, `names.json`,
  `location_names.json`, `achievements.json`, plus `dashboard.html`, `worldgen.js`, `fonts/*.woff2`.
- Dedicated-server worlds live only on the server; players' PCs do not have them. So the server must keep reading
  the world folder (seed, day, bosses, modifiers, placed locations).

## Approaches considered

- **A. One Rust core shared by app and server, raw `.fch` uploads as today (chosen).** All logic once; protocol
  unchanged; the analysis-version re-upload mechanism stays. Cost: porting ~560 lines of stdlib Python.
- B. The app parses and uploads finished JSON; the server mostly stores. Rejected: the server still needs the world
  parser, and clients on different versions would send results from different analysis code. Saves only upload
  traffic.
- C. Port the parser to Rust for the app only, keep `server.py`. Rejected: two parsers in two languages, the
  duplication the user explicitly does not want.

## Design

### Layout

`client/` becomes a Cargo workspace (paths are the builder's call within this shape):

- `tracker-core` (lib): everything that is not UI or networking glue.
  - `fch` — `.fch` reader (port of `fch.py`), including per-world data.
  - `analyze` — sections, achievements, meta (port of `analyze.py`).
  - `mapdata` — explored bitmask, pins, spawn/bed/death (port of `mapdata.py`).
  - `worlds` — `.fwl2` / `.db2` reader: name, seed, day, bosses, keys, modifiers, locations (port of `worlds.py`).
  - `store` — the character state (what `server.py` writes to `state/` today) with "seen"/"online" logic.
  - `hall` — the HTTP API and static files: `/`, `/api/characters`, `/api/character/<name>`,
    `/api/achievements`, `/api/world`, `/api/map`, `/worldgen.js`, `/fonts/*.woff2`, `POST /upload`,
    `POST /heartbeat`, same JSON shapes and status codes as `server.py`. The data JSONs, `dashboard.html`,
    `worldgen.js` and fonts are embedded at compile time (`include_str!` / `include_bytes!` from the repo root, so
    `scripts/` keep writing to where they write now).
  - The analysis version `AV` is a hash of the embedded data plus a core version constant, so a new release
    triggers re-uploads exactly like today.
- `tracker-server` (bin): `hall` over a TCP port. Env: `TRACKER_TOKEN`, `WORLD_DIR`, `STALE_MINUTES`; flags
  `--port`, `--host`, `--state`. Subcommands that print JSON:
  - `check <file.fch>` replaces `python analyze.py FILE`;
  - `world <dir>` prints `read_world` plus `locations` (raw location hashes included, so
    `scripts/extract_location_names.py` can match them).
  The extraction scripts that import `worlds.py` today (`scripts/extract_game_data.py`,
  `scripts/extract_location_names.py`) call this subcommand instead, so the world parser exists once. The scripts
  then need a built `tracker-server` (they already need UnityPy and bun/node). `stable_hash` (eight lines, no
  parsing) may be copied into the scripts. `scripts/update_achievements.py` already references a missing
  `analyze.SECTION_ACH`; it is fixed to read the section achievements from `gamedata.json` during the port.
- `src-tauri` (the app): sync loop as today plus a local Hall.

`server.py`, `analyze.py`, `fch.py`, `mapdata.py`, `worlds.py` are deleted once parity holds (see Verification).
The `Dockerfile` becomes a multi-stage Rust build producing a small image with only `tracker-server`; amd64 + arm64
as now. `docker-compose.yaml` variables and volumes stay the same.

### The app

- The window keeps its own title bar and becomes two views: **the Hall** (default) and **settings** (today's sync
  page: server, token, characters folder, per-character on/off, pause, autostart, update).
- The Hall is shown in an iframe inside the app's own page, so the title bar stays and a remote page never gets
  Tauri IPC (the iframe is a different origin; `withGlobalTauri` must not leak into it — the builder verifies this).
  The iframe is **always** `http://127.0.0.1:<port>` (CSP `frame-src` names only that): in group mode the app's
  local server forwards the Hall's GET routes (`/`, `/api/*`, `/worldgen.js`, `/fonts/*`) to the configured server.
  This avoids a CSP that has to name a server only known at runtime, and mixed-content blocking of a plain-HTTP
  remote inside the app page. Our call (the alternatives were a broad `frame-src http: https:` or a separate window
  without the shared title bar).
- **Local mode** (no server set, or the Local side of the switch): the app runs `hall` on `127.0.0.1` on a free
  port, fed directly from disk instead of uploads:
  - characters: the selected (ticked) `.fch` files in the characters folder, re-analysed when their size/mtime
    changes (same settle rule as the sync loop);
  - world: the `worlds_local` (or Steam Cloud `worlds`) folder next to the characters folder. In 1.0 each world
    is its own subfolder (`worlds/roota/_main.14.{fwl2,db2,ok}`), with `<name>_backup_auto-<date>` folders next to
    it that are skipped, like character backups. *Builder's/our call:*
    default to the world with the newest complete save (`_main.N.ok` logic as in `worlds.py`); settings lets the user
    pick another. One world shown at a time, as now.
  - online/resting: a local character counts as online while its file changed within `STALE_MINUTES` (default 10);
    our call, refine if it reads wrong in play.
  - Uploads and the token check are not used in local mode.
- **Group mode** (a server is set): the iframe shows the server's Hall; the sync loop uploads as today.
- A small **Local / Group** switch in the title bar or Hall header when a server is set, so a solo world can still be
  looked at. Our call.
- "Open the Hall" in the browser stays for group mode.

### The server

Behaviour identical to `server.py`: token-checked raw uploads (`X-Token`, max 5 MB, name rule
`^[A-Za-z0-9_. -]{1,64}$`), heartbeat 404 for unknown/outdated, 400 with the parser's message for unreadable saves,
map failure never blocks an upload, atomic writes of the state files, world reads cached by latest save number,
`/api/map` / `/api/world` return `{"error": ...}` with 200 on failure (the dashboard relies on that).

State is not migrated: the new `AV` differs, heartbeats get 404, and every client re-uploads by itself. The state
file layout may stay as is or change; the builder decides.

### Verification

- **Parity before deletion.** A test harness runs the Python modules and the Rust core on the same inputs and
  compares JSON: at least the six 1.0 characters (profile version 46/47) and the `roota` world in the developer's
  Steam Cloud folder (`~/.local/share/Steam/userdata/<id>/892970/remote/{characters,worlds/roota}`) (sections, achievements,
  meta, map data, world, locations). Floats compared as the Python rounds them. Test fixtures (saves, world) must
  not be committed if they are private; the harness takes paths.
- Unit tests in `tracker-core` for the parser edge cases `fch.py` handles (bad length, old version, layout change,
  garbage → clear error) and the existing client tests (names, URL encoding).
- Manual: app in local mode with a real characters folder and world; app in group mode against the Rust server in
  docker; dashboard in a phone-width browser against the server.

## Decided by the user

- Approach A.
- With a remote configured, the app shows the group's (remote) view.
- The server stays and keeps serving the website; no duplicate dashboard or logic between app and website.
- Client stays Rust; the Python server goes; extraction scripts stay Python.

## Calls made here (check these)

- Workspace inside `client/`, data embedded at compile time from the repo root.
- Hall in an iframe inside the app page (title bar kept, remote page isolated).
- Local mode picks the most recently saved world; switchable in settings.
- Local "online" = file changed within `STALE_MINUTES`.
- Local / Group switch when a server is set.
- No state migration; re-upload via the analysis version.
- `check` subcommand replaces `python analyze.py`.
- In group mode the local server forwards the remote Hall, so the iframe is always `127.0.0.1`.
- The extraction scripts get world data from a `tracker-server world <dir>` subcommand, not from a kept `worlds.py`.

## Left to the builder

- Crate and module names, HTTP library for the embedded server (a small sync one such as `tiny_http` fits the
  current ureq/thread style), zlib/gzip crate (`flate2`).
- Exact state file format, free-port selection, how the local Hall is fed (shared store vs. a direct file source
  behind the same API).
- Order of the port (suggested: `fch` → `analyze` → `mapdata` → `worlds` → `hall` → server bin → app views →
  Docker/CI → delete Python), README/TODO updates.
