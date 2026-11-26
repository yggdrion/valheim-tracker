# TODO

## Wanted next
- [ ] **Map**: the explored map lives in each character's `.fch` (compressed block after the stat sets, per character). Decode it, merge all players, render it (biome/height colours, player spawns, boss altars) on the dashboard. World-side: locations and structures from the chunk files.
- [ ] **Dashboard rework**: layout and flow are still a first pass (see the last screenshots). Ideas: group achievements by category, per-player detail page, nicer progress for the counter achievements, better empty/offline states, mobile check.
- [ ] **Font**: not happy yet. Cinzel + Averia Serif Libre is the current pick; try other self-hosted options (OFL), compare in the browser, keep it readable for the long item lists.
- [ ] **Test the client on Linux** (and macOS): `fchsync` build, Steam Cloud / `characters_local` paths (`~/.config/unity3d/IronGate/Valheim/characters_local`, Proton prefix), file-change detection.

## Also open
- [ ] Reverse proxy + TLS in front of the tracker (planned, not done; uploads are plain HTTP with a shared token).
- [ ] Match characters to worlds (a character file knows its worlds; the dashboard shows one world only).
- [ ] Autodetect the characters folder in `fchsync`; run it as a service / autostart on Windows and Linux.
- [ ] Go tests for `fchparse` (a small synthetic `.fch`), CI that runs them plus `analyze.py --check`.
- [ ] Upstream Melichor has no license: ask the author or rewrite the achievement tables before sharing widely.
- [ ] More world data: boss altars/locations, structure counts, day length from the save instead of the 1800 s default.
- [ ] README screenshots.
