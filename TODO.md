# TODO

## Wanted next
- [ ] **Map**: the explored map lives in each character's `.fch` (compressed block after the stat sets, per character). Decode it, merge all players, render it (biome/height colours, player spawns, boss altars) on the dashboard. World-side: locations and structures from the chunk files.
- [ ] **Dashboard rework**: layout and flow are still a first pass (see the last screenshots). Ideas: group achievements by category, per-player detail page, nicer progress for the counter achievements, better empty/offline states, mobile check.
- [ ] **Font**: not happy yet. Cinzel + Averia Serif Libre is the current pick; try other self-hosted options (OFL), compare in the browser, keep it readable for the long item lists.

## Raw upload (done)
Server takes raw `.fch` (`POST /upload?name=X`, parsed with upstream `parse()`), Go parser removed, `fchsync.sh` is the Linux/macOS client (tested on Linux with six real characters: numbers identical to the old Go path; garbage/empty/old-version uploads give 400).
- [ ] Test `fchsync.ps1` / `.bat` on a Windows box (written on Linux, never run; no pwsh here).
- [ ] Optional: keep the last raw file per character in `state/` to recompute after a table update; per-name rate limit.
- [ ] Autostart how-to (systemd user unit / Task Scheduler); macOS paths in `fchsync.sh` are untested.

## Also open
- [ ] Reverse proxy + TLS in front of the tracker (planned, not done; uploads are plain HTTP with a shared token).
- [ ] Match characters to worlds (a character file knows its worlds; the dashboard shows one world only).
- [ ] Upstream Melichor has no license: ask the author or rewrite the achievement tables before sharing widely.
- [ ] More world data: boss altars/locations, structure counts, day length from the save instead of the 1800 s default.
- [ ] README screenshots.
