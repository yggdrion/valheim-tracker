# TODO

## Wanted next
- [ ] **Map**: the explored map lives in each character's `.fch` (compressed block after the stat sets, per character). Decode it, merge all players, render it (biome/height colours, player spawns, boss altars) on the dashboard. World-side: locations and structures from the chunk files.
- [ ] **Dashboard rework**, rest: better empty/offline states, mobile check, fold the three Slay rows into one, readable names for a few pieces ("Levelground").

## Done
- Font: compared 8 OFL pairs in the browser (2026-11-26), kept Cinzel + Averia Serif Libre.
- Dashboard rework, part 1: co-op framing (no leaderboard), Saga panel with non-achievement stats from the save (travel, slaying, gathering, building, life), The Road (what is left per biome, who still needs it), achievements grouped by category, player page `#player/<name>`, characters matched to the shown world.

## Raw upload (done)
Server takes raw `.fch` (`POST /upload?name=X`, parsed with upstream `parse()`), Go parser removed, `fchsync.sh` is the Linux/macOS client (tested on Linux with six real characters: numbers identical to the old Go path; garbage/empty/old-version uploads give 400).
- [ ] Test `fchsync.ps1` / `.bat` on a Windows box (written on Linux, never run; no pwsh here).
- [ ] Optional: keep the last raw file per character in `state/` to recompute after a table update; per-name rate limit.
- [ ] Autostart how-to (systemd user unit / Task Scheduler); macOS paths in `fchsync.sh` are untested.

## Also open
- TLS: handled later by a separate reverse proxy (out of scope here; uploads are plain HTTP with a shared token until then).
- [ ] Upstream Melichor has no license: ask the author or rewrite the achievement tables before sharing widely.
- [ ] More world data: boss altars/locations, structure counts, day length from the save instead of the 1800 s default.
- [ ] README screenshots.
- [ ] **GHCR image**: build and publish the tracker image to `ghcr.io` (GitHub Actions on tag/main), then point `docker-compose.yaml` at the image instead of `build: .`.
- [ ] **Tiny GUI client** for Windows/macOS/Linux: enter server URL + token (optionally the characters folder), it autodetects the folder, lists the characters found and lets you select which ones to sync (default all; the choice is remembered), and syncs in the background (status, last upload, errors), remembers settings, autostart option. Same protocol as `fchsync.sh` (raw `.fch` upload + heartbeat). Candidates: Go + small GUI toolkit, or Wails/Tauri; decide. It replaces `fchsync.ps1` and `fchsync.bat` (delete them once it works); `fchsync.sh` stays as the headless option.
