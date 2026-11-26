# TODO

## Wanted next
- [ ] Map extras: rivers and real terrain height (the game's GetBiomeHeight/rivers are not ported), per-player explored overlay, worlds other than the one in WORLD_DIR.

## Done
- Font: compared 8 OFL pairs in the browser (2026-11-26), kept Cinzel + Averia Serif Libre.
- Dashboard rework, part 1: co-op framing (no leaderboard), Saga panel with non-achievement stats from the save (travel, slaying, gathering, building, life), The Road (what is left per biome, who still needs it), achievements grouped by category, player page `#player/<name>`, characters matched to the shown world.
- Dashboard rework, part 2: one Slay row (full dot = slain on hard), the game's English names for items/pieces (`names.json`, `scripts/extract_names.py`), offline characters stay visible as "resting", empty-hall and lost-contact states, phone layout checked at 375 px.
- Map page (`#map`): biomes drawn from the world seed (`worldgen.js`, a port of the game's WorldGenerator; 99.65 % of a real world's 12,176 locations land in their own biome, all altars/traders/dungeons 100 %), everyone's explored area merged from the `.fch` files (`mapdata.py`), pins, beds, deaths, boss altars, traders and dungeons from the world save (`location_names.json`, `scripts/extract_location_names.py`); undiscovered land and places only with "reveal".
- GHCR image: `.github/workflows/build.yml` publishes ghcr.io/yggdrion/valheim-tracker (amd64 + arm64; `:latest` = main, `:X.Y.Z` from tags); `docker-compose.yaml` pulls it (`TRACKER_TAG`).
- Tracker Sync desktop client (`client/`, Tauri): server + token with a check, auto-detected characters folder (most recently played first), per-character on/off (remembered), status per character, pause, sync now, start with the computer. Builds: `.github/workflows/build.yml`, same trigger as the image; releases are automatic (semantic-release on push to main).

## Raw upload (done)
Server takes raw `.fch` (`POST /upload?name=X`, parsed with upstream `parse()`), Go parser removed, `fchsync.sh` is the Linux/macOS client (tested on Linux with six real characters: numbers identical to the old Go path; garbage/empty/old-version uploads give 400).
- [ ] Test `fchsync.ps1` / `.bat` on a Windows box (written on Linux, never run; no pwsh here).
- [ ] Optional: keep the last raw file per character in `state/` to recompute after a table update; per-name rate limit.
- [ ] Autostart how-to (systemd user unit / Task Scheduler); macOS paths in `fchsync.sh` are untested.

## Also open
- TLS: handled later by a separate reverse proxy (out of scope here; uploads are plain HTTP with a shared token until then).
- [ ] Upstream Melichor has no license: ask the author or rewrite the achievement tables before sharing widely.
- [ ] More world data: boss altars/locations, structure counts, day length from the save instead of the 1800 s default.
- [ ] Tracker Sync client: try the Windows and macOS builds (from the `build` workflow), then delete `fchsync.ps1` / `fchsync.bat`. Untested so far: folder picker, autostart, "Open the Hall" (tested on Linux: detection, token check, upload, rejections, selection).
