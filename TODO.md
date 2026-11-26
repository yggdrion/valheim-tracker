# TODO


## Done
- Font: compared 8 OFL pairs in the browser (2026-11-26), kept Cinzel + Averia Serif Libre.
- Dashboard rework, part 1: co-op framing (no leaderboard), Saga panel with non-achievement stats from the save (travel, slaying, gathering, building, life), The Road (what is left per biome, who still needs it), achievements grouped by category, player page `#player/<name>`, characters matched to the shown world.
- Dashboard rework, part 2: one Slay row (full dot = slain on hard), the game's English names for items/pieces (`names.json`, `scripts/extract_names.py`), offline characters stay visible as "resting", empty-hall and lost-contact states, phone layout checked at 375 px.
- Map page (`#map`): biomes drawn from the world seed (`worldgen.js`, a port of the game's WorldGenerator; 99.65 % of a real world's 12,176 locations land in their own biome, all altars/traders/dungeons 100 %), everyone's explored area merged from the `.fch` files (`mapdata.py`), pins, beds, deaths, boss altars, traders and dungeons from the world save (`location_names.json`, `scripts/extract_location_names.py`); undiscovered land and places only with "reveal".
- GHCR image: `.github/workflows/build.yml` publishes ghcr.io/yggdrion/valheim-tracker (amd64 + arm64; `:latest` = main, `:X.Y.Z` from tags); `docker-compose.yaml` pulls it (`TRACKER_TAG`).
- Tracker Sync desktop client (`client/`, Tauri): server + token with a check, auto-detected characters folder (most recently played first), per-character on/off (remembered), status per character, pause, sync now, start with the computer. Builds: `.github/workflows/build.yml`, same trigger as the image; releases are automatic (semantic-release on push to main).
- Own game data instead of Melichor's achi_hunter.py (no licence): `fch.py` reads saves (from the game's save code), `scripts/extract_game_data.py` builds all 15 achievement sections from the game files by the game's rules (lists identical to before; The Hunter now counts the 8 bosses, 108 like the game) and derives the biome of every entry (1,057 of 1,082 equal to the old tables, the rest reviewed; `curated/`).

## Raw upload (done; the fchsync scripts were replaced by Tracker Sync)
Server takes raw `.fch` (`POST /upload?name=X`, parsed with our `fch.py`), Go parser removed, `fchsync.sh` was the first client (tested on Linux with six real characters: numbers identical to the old Go path; garbage/empty/old-version uploads give 400).

## Also open
- TLS: handled later by a separate reverse proxy (out of scope here; uploads are plain HTTP with a shared token until then).
- [ ] Tracker Sync client: try the Windows and macOS builds (from the releases). Untested so far: folder picker, autostart, "Open the Hall" (tested on Linux: detection, token check, upload, rejections, selection).
