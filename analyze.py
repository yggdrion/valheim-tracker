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



# Not-achievement stats for the dashboard's saga: the game's own counter names (scripts/extract_stat_names.py).
STAT_NAMES = json.load(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "stat_names.json"), encoding="utf8"))
assert len(STAT_NAMES) == ah.STAT_COUNT, "stat_names.json is out of date (run scripts/extract_stat_names.py)"
_NAMES = {**ah.CRAFTABLE, **ah.BUILDABLE, **ah.TROPHIES, **ah.FOOD_EATEN, **ah.ENEMIES, **ah.BOSSES, **ah.MINIBOSSES, **ah.FISH}


def _nm(tok):
    n = _NAMES.get(tok)
    return n if isinstance(n, str) and n else ah.strip(tok).replace("_", " ").capitalize()


def meta(d):
    """Plain counters from the any-difficulty set: {stats: {StatName: v}, kills, harvested, picked_up, eaten, built, worlds}."""
    s = d["sets"][0]
    cnt = lambda m: {_nm(k): int(v) for k, v in sorted(m.items(), key=lambda kv: -kv[1]) if v > 0}
    return {"stats": {n: round(v, 1) for n, v in zip(STAT_NAMES, s["stats"]) if v},
            "kills": cnt(s["kills"]), "picked_up": cnt(s["picked_up"]), "eaten": cnt(s["foods_eaten"]),
            "built": cnt(s["pieces_built"]),
            "harvested": {k: int(v) for k, v in sorted(s["pickables"].items(), key=lambda kv: -kv[1]) if v > 0},
            "worlds": {k: round(v) for k, v in s["worlds"].items()}}

if __name__ == "__main__":
    print(json.dumps(analyze_parsed(ah.parse(sys.argv[1])), indent=1))
