# valheim-tracker

Read-only achievement tracker for a Valheim 1.0 all-achievements run. No game mods.

- `cmd/fchsync` (Go, per player PC): reads `.fch` character files, posts parsed counters. Never writes to the game folder.
- `server.py` (Python, stdlib): receives uploads, analyses them, reads the world folder read-only, serves the dashboard.
- `docker-compose.yaml`: dedicated server ([community-valheim-tools/valheim-server-docker](https://github.com/community-valheim-tools/valheim-server-docker)) + tracker.

## Run

    cp .env.example .env            # edit passwords/token
    # put your world folder into config/worlds_local/<WORLD_NAME>/
    docker compose up -d --build    # dashboard: http://localhost:8088

## Player upload

    go build -o fchsync ./cmd/fchsync
    ./fchsync -dir "<characters_local or Steam remote/characters>" -server http://host:8088 -token <TRACKER_TOKEN>

Character files live on the player's PC (`%USERPROFILE%\AppData\LocalLow\IronGate\Valheim\characters_local`, or
`Steam\userdata\<id>\892970\remote\characters` with Steam Cloud). The server never has them.

## Notes

- Achievement tables in `vendor/achi_hunter.py` come from [Melichor/ValheimSaveParser](https://github.com/Melichor/ValheimSaveParser). To update after a patch: `python scripts/update_melichor.py` (see `.claude/skills/update-melichor/SKILL.md`), and `python scripts/update_achievements.py` for the Steam list.
- No TLS: put a reverse proxy in front before exposing it.
- Self-check of the Go parser against the reference: `python analyze.py --check Some.fch`.
- Not done: the other ~47 Steam achievements, per-world character matching.
