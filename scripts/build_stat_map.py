#!/usr/bin/env python3
"""Turn .cache/achievement_defs.json (scripts/extract_achievements.py) into stat_map.json, which the Rust core reads.

  python scripts/build_stat_map.py            # dry run: print what would change
  python scripts/build_stat_map.py --apply    # write stat_map.json

stat_map.json: {api_name: {"set": difficulty-set index,
                           "stats":   [[PlayerStatType index, goal], ...]   all must reach their goal
                           "enemies": [[enemy token, goal], ...]            kills in the set
                           "crafts":  [[item token, goal], ...]
                           "others":  [api_name, ...]}}                     other achievements it embodies
List-style achievements (dynamic lists, bosses as a group, fish, trophies, ...) are not in here: the analysis
reads them from gamedata.json (scripts/extract_game_data.py).
"""
import argparse, json, os, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DEFS = os.path.join(ROOT, ".cache", "achievement_defs.json")
OUT = os.path.join(ROOT, "stat_map.json")


def build(defs):
    by_pid = {d["_path_id"]: d["m_id"] for d in defs}
    out = {}
    for d in sorted(defs, key=lambda d: d["m_id"]):
        e = {"set": d["m_difficultyRequirement"]}
        if d["m_statTrigger"]:
            e["stats"] = [[t["m_stat"], t["m_amountAboveEquals"]] for t in d["m_statTrigger"]]
        if not d["m_isDynamicallyPopulated"]:
            if len(d["m_enemyStatsTriggers"]) == 1:      # a single boss; the group achievements use sections
                t = d["m_enemyStatsTriggers"][0]
                e["enemies"] = [[t["m_stat"], t["m_amount"]]]
            if len(d["m_itemCraftTriggers"]) == 1:
                t = d["m_itemCraftTriggers"][0]
                e["crafts"] = [[t["m_stat"], t["m_amount"]]]
        if d["m_otherAchievementTriggers"]:
            e["others"] = [by_pid[t["m_PathID"]] for t in d["m_otherAchievementTriggers"]]
        if len(e) > 1:
            out[d["m_id"]] = e
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--apply", action="store_true")
    a = ap.parse_args()
    if not os.path.exists(DEFS):
        sys.exit("run scripts/extract_achievements.py first")
    new = build(json.load(open(DEFS, encoding="utf8")))
    old = json.load(open(OUT, encoding="utf8")) if os.path.exists(OUT) else {}
    print(f"{len(new)} achievements in the map (was {len(old)})")
    for k in sorted(new.keys() | old.keys()):
        if new.get(k) != old.get(k):
            print(" ", "changed" if k in new and k in old else "new" if k in new else "gone", k, json.dumps(new.get(k)))
    if a.apply:
        json.dump(new, open(OUT, "w", encoding="utf8", newline=""), indent=0, sort_keys=True)
        print("wrote", OUT)


if __name__ == "__main__":
    main()
