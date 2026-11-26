# TODO

## Wanted next
- [ ] **Map**: the explored map lives in each character's `.fch` (compressed block after the stat sets, per character). Decode it, merge all players, render it (biome/height colours, player spawns, boss altars) on the dashboard. World-side: locations and structures from the chunk files.
- [ ] **Dashboard rework**: layout and flow are still a first pass (see the last screenshots). Ideas: group achievements by category, per-player detail page, nicer progress for the counter achievements, better empty/offline states, mobile check.
- [ ] **Font**: not happy yet. Cinzel + Averia Serif Libre is the current pick; try other self-hosted options (OFL), compare in the browser, keep it readable for the long item lists.
- [ ] **Test the client on Linux** (and macOS): `fchsync` build, Steam Cloud / `characters_local` paths (`~/.config/unity3d/IronGate/Valheim/characters_local`, Proton prefix), file-change detection.

## Decided, do later: raw upload, scripts as clients, no Go
Today the `.fch` parser exists twice (Go in `fchparse`, Python in the vendored Melichor script) and `analyze.py --check` keeps them in sync. Decision: the client uploads the raw `.fch`, only the server parses it (Python, upstream `parse()`), and the client becomes a small script. Sizes: 27-152 KB per character (4-46 KB gzipped), uploaded only on change (a few per hour) plus a tiny heartbeat, so bandwidth is no issue.

Server (`server.py`):
- [ ] `POST /upload?name=X` takes the raw `.fch` bytes (keep `X-Token`, 5 MB cap, name check). Parse with `achi_hunter.parse(bytes)`; `SaveError` / parse failures -> 400 with the message (the client logs it). Drop the JSON-from-Go payload path.
- [ ] `analyze.from_go()` goes away: `parse()` already returns `{"version", "sets": [...]}` in the shape `build_sections()` / `achievements()` want. Rename `analyze_go_json` accordingly.
- [ ] Optional: keep the last raw file per character in `state/` so the analysis can be recomputed after a table update without waiting for the next upload (then the `av` re-upload trick is no longer needed).
- [ ] Parsing untrusted bytes: bound CPU/memory (size cap before parsing, catch `struct.error`, `IndexError`, `UnicodeDecodeError`, `MemoryError`), and consider a per-name rate limit.

Clients (read-only on the game folder):
- [ ] **Windows `fchsync.ps1`** (about 30 lines): loop every 2 s over `*.fch` (skip `*_backup_*`); compare `LastWriteTimeUtc` + `Length`; wait ~0.7 s and re-check (the game writes `.fch.new` then renames); read with `FileStream` and `FileShare.ReadWrite`; `Invoke-RestMethod -Method Post -Body $bytes -Headers @{ 'X-Token' = $token }`; `POST /heartbeat` every 30 s; on 404 from the heartbeat, upload again. Parameters: `-Dir`, `-Server`, `-Token` (or env `TRACKER_TOKEN`). Default `-Dir` to `characters_local`, falling back to the Steam Cloud `userdata\*\892970\remote\characters`.
- [ ] `fchsync.bat`: double-click wrapper, `powershell -NoProfile -ExecutionPolicy Bypass -File fchsync.ps1 %*`, plus a place to put server URL and token (a small `fchsync.ini`/env file next to it, gitignored).
- [ ] **Linux / macOS `fchsync.sh`**: `curl` + `stat` loop with the same logic (`--data-binary @file`), `characters_local` under `~/.config/unity3d/IronGate/Valheim` and the Proton prefix for Steam Deck; this also covers the "test on Linux" item.
- [ ] Optional later: autostart (Task Scheduler entry / systemd user unit) and a tray-less "run at login" how-to in the README.

Cleanup once the scripts work:
- [ ] Delete `fchparse/`, `cmd/`, `go.mod`; drop `analyze.py --check` and the parity step from `scripts/update_melichor.py` and `.claude/skills/update-melichor/SKILL.md` (updating Melichor becomes "copy the file and run `update_achievements.py` / `build_stat_map.py`"); keep the stat-count check against upstream constants.
- [ ] README: new client install steps (download the script, set URL + token), remove the Go build instructions.
- [ ] Test: same four characters as today must give identical numbers before and after the switch (compare `/api/character/<name>` JSON), including a deliberately truncated/garbage upload (must 400, not crash).

## Also open
- [ ] Reverse proxy + TLS in front of the tracker (planned, not done; uploads are plain HTTP with a shared token).
- [ ] Match characters to worlds (a character file knows its worlds; the dashboard shows one world only).
- [ ] Autodetect the characters folder in `fchsync`; run it as a service / autostart on Windows and Linux.
- [ ] Go tests for `fchparse` (a small synthetic `.fch`), CI that runs them plus `analyze.py --check`.
- [ ] Upstream Melichor has no license: ask the author or rewrite the achievement tables before sharing widely.
- [ ] More world data: boss altars/locations, structure counts, day length from the save instead of the 1800 s default.
- [ ] README screenshots.
