# valheim-tracker

Read-only achievement tracker for a Valheim 1.0 all-achievements run. No game mods.

- `fchsync.sh` (bash + curl, per player PC; a Windows `fchsync.ps1` is still to do): uploads changed `.fch` character files raw. Never writes to the game folder.
- `server.py` (Python, stdlib): receives raw uploads, parses and analyses them, reads the world folder read-only, serves the dashboard.
- `docker-compose.yaml`: dedicated server ([community-valheim-tools/valheim-server-docker](https://github.com/community-valheim-tools/valheim-server-docker)) + tracker.

## Run

    cp .env.example .env            # edit passwords/token
    # put your world folder into config/worlds_local/<WORLD_NAME>/
    docker compose up -d --build    # dashboard: http://localhost:8088

## Player upload

    TRACKER_TOKEN=<token> ./fchsync.sh -s http://host:8088          # Linux / macOS / Steam Deck
    ./fchsync.sh -d "<characters folder>" -s http://host:8088 -t <token> -1   # explicit folder, sync once

Without `-d` it looks in `~/.config/unity3d/IronGate/Valheim/characters_local`, the Steam Cloud `userdata/*/892970/remote/characters`
and the Proton prefix. Env alternatives: `TRACKER_SERVER`, `TRACKER_TOKEN`, `FCH_DIR`.

Character files live on the player's PC (`%USERPROFILE%\AppData\LocalLow\IronGate\Valheim\characters_local`, or
`Steam\userdata\<id>\892970\remote\characters` with Steam Cloud). The server never has them.

## Notes

- Achievement tables in `vendor/achi_hunter.py` come from [Melichor/ValheimSaveParser](https://github.com/Melichor/ValheimSaveParser). To update after a patch: `python scripts/update_melichor.py` (see `.claude/skills/update-melichor/SKILL.md`), and `python scripts/update_achievements.py` for the Steam list.
- No TLS: put a reverse proxy in front before exposing it.
- The server parses uploads with the vendored upstream `parse()`; bad or garbage uploads get HTTP 400. Quick local check: `python analyze.py Some.fch`.
- Not done: the other ~47 Steam achievements, per-world character matching.
