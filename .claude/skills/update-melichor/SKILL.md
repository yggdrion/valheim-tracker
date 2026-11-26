---
name: update-melichor
description: Update this repo to a newer Melichor/ValheimSaveParser (achi_hunter.py) or a new Valheim save layout, and keep the achievement mapping and Steam list in sync. Use when a new Valheim patch lands, upstream Melichor changes, the dashboard shows wrong or missing achievement numbers, or uploads fail with "layout changed".
---

# Update Melichor port

The tracker never edits game files. Work only on copies of `.fch` files (a `testdata/` folder is gitignored).

## What depends on upstream

| Thing here | Comes from upstream | If upstream changes |
|---|---|---|
| `vendor/achi_hunter.py` | copied verbatim | re-copy (script does it) |
| `vendor/UPSTREAM.json` | sha, `parse()` hash, constants | rewritten by the script |
| `analyze.py` `SECTION_ACH` | upstream section ids (list-style achievements) | edit by hand |
| `stat_map.json` | not upstream: the game's own Achievement assets (stat index + goal per achievement) | `extract_achievements.py` then `build_stat_map.py --apply` after a Valheim patch |
| `achievements.json` | not upstream: Steam's page | `scripts/update_achievements.py` |

## Steps

1. Dry run: `python scripts/update_melichor.py`
   It fetches upstream into `.cache/melichor`, lists new upstream commits touching `achi_hunter.py`/`data`, and prints `[OK]`, `[INFO]`, `[WARN]` or `[FAIL]`. Exit 1 means a FAIL.
2. Read each non-OK line:
   - `INFO parse() changed`: the server calls upstream `parse()` directly, so nothing to port; just check a real save in step 4.
   - `FAIL/WARN STAT_COUNT`: stat indices in `stat_map.json` may have shifted; regenerate it (below).
   - `FAIL stat_map.json ...` or `SECTION_ACH`: regenerate the stat map (next section) and/or update `SECTION_ACH` in `analyze.py`.
   - `INFO upstream sections we do not map`: a new upstream list may match a Steam achievement; add it to `SECTION_ACH`.
3. Apply and check against a real save: `python scripts/update_melichor.py --apply --fch <copy-of-a-.fch>`
   The last line must say `smoke: OK ...`. Run it on more than one character, one with progress and one fresh.
4. Refresh the Steam list: `python scripts/update_achievements.py`, then `--apply` if the diff is right. It prints new achievements that are not trackable yet.
5. Rebuild and look: `docker compose up -d --build tracker`, then open the dashboard (port 8088) and check a few numbers against `python vendor/achi_hunter.py <fch> -f`.
6. Commit `vendor/`, `analyze.py`, `achievements.json` together.

## Stat map (exact, from the game)

After a Valheim patch the stat enum or achievement goals can change:

1. `python -m venv .cache/venv && .cache/venv/Scripts/pip install UnityPy` (once; needs approval to install).
2. `.cache/venv/Scripts/python.exe scripts/extract_achievements.py` reads the 53 Achievement assets from the game install (read-only, ~30 s) into `.cache/achievement_defs.json`.
3. `python scripts/build_stat_map.py` shows what changed in `stat_map.json`; add `--apply` to write it.
4. Check `PlayerStatType` count (205 now): `STAT_COUNT` in upstream must match the indices, otherwise indices shift.

`stat_map.json` entry: `set` (difficulty set the achievement reads; `1` = any, `6` normal, `7` hard; set 0 is raw and never used), `stats` `[index, goal]` (all must reach goal), `enemies`, `crafts`, `others` (made of other achievements, e.g. Kings and Queens).

## Pitfalls

- Upstream has no license. The repo is public: ask the author before redistributing, or rewrite the tables.
- On Windows pass `encoding="utf8"` when reading these files in Python.
- If `fetch()` in `update_achievements.py` fails ("Steam changed its markup"), inspect the page for the escaped `"achievements":[` JSON and adjust.
- Never run tools against the live `characters` / `worlds` folders; copy first.
