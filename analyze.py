"""Server side: turn a parsed .fch into achievement progress, via achi_hunter's tables.
usage: python analyze.py FILE.fch"""
import json, sys, os
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "vendor"))
import achi_hunter as ah  # ponytail: no license upstream; fine locally, ask author before redistributing

def analyze_parsed(d, only=None):
    """d = achi_hunter.parse() result: {"version", "sets": [...]}"""
    out = []
    for sec in ah.build_sections(d):
        if only and sec["id"] not in only: continue
        ents = [{"token": t, "name": sec["universe"][t] or ah.strip(t), "count": int(sec["done"].get(t, 0)),
                 "biome": sec["biomes"].get(t, "")} for t in sec["universe"]]
        out.append({"id": sec["id"], "title": sec["title"], "total": len(ents),
                    "done": sum(1 for e in ents if e["count"] > 0), "entries": ents})
    return out

# Steam achievement -> save data. Everything else on Steam's list is not derivable from the character counters.
SECTION_ACH = {  # api_name -> achi_hunter section id (done/total entries)
    "AllItemCraft": "crafted", "AllWeaponCraft": "weapons", "AllFoodCooked": "cooked", "AllFoodEaten": "food-eaten",
    "AllBuildPieces": "built", "DeathByAllTypes": "deaths", "DeathByTreeAll": "tree-deaths",
    "KillAllCreatures": "enemies", "KillAllCreaturesHard": "enemies-hard", "AllBosses": "bosses-any",
    "AllBossesNormal": "bosses-normal", "AllBossesHard": "bosses-hard", "AllMiniBosses": "minibosses",
    "GrindFish": "fishing", "FindAllTrophies": "trophies",
}
# Everything else comes from the game's own Achievement assets: stat_map.json (scripts/build_stat_map.py).
STAT_MAP = json.load(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "stat_map.json"), encoding="utf8"))


def achievements(d, sections):
    """{api_name: [done, total]} for every Steam achievement. done == total means unlocked."""
    sets = d["sets"]
    secs = {x["id"]: x for x in sections}
    out = {api: [secs[sid]["done"], secs[sid]["total"]] for api, sid in SECTION_ACH.items()}
    for api, e in STAT_MAP.items():
        if "others" in e:
            continue
        s, done, total = sets[e["set"]], 0.0, 0.0
        for i, goal in e.get("stats", []):
            done += min(s["stats"][i], goal); total += goal
        for tok, goal in e.get("enemies", []):
            done += min(s["kills"].get(tok, 0), goal); total += goal
        for tok, goal in e.get("crafts", []):
            done += min(s["crafted"].get(tok, 0), goal); total += goal
        out[api] = [int(done), int(total)]
    for api, e in STAT_MAP.items():   # Kings and Queens / Child of Odin: made of other achievements
        if "others" in e:
            out[api] = [sum(out[o][0] >= out[o][1] for o in e["others"]), len(e["others"])]
    return out


if __name__ == "__main__":
    print(json.dumps(analyze_parsed(ah.parse(sys.argv[1])), indent=1))
