"""Server side: turn a parsed .fch (fch.py) into achievement progress and saga stats, from gamedata.json.
usage: python analyze.py FILE.fch"""
import json, os, re, sys

import fch

_HERE = os.path.dirname(os.path.abspath(__file__))
_load = lambda f: json.load(open(os.path.join(_HERE, f), encoding="utf8"))
# the achievement sections, built from the game's files (scripts/extract_game_data.py)
GAMEDATA = _load("gamedata.json")
# the game's PlayerStatType names (scripts/extract_stat_names.py) and English names (scripts/extract_names.py)
STAT_NAMES = _load("stat_names.json")
assert len(STAT_NAMES) == fch.STAT_COUNT, "stat_names.json is out of date (run scripts/extract_stat_names.py)"
_STAT = {n: i for i, n in enumerate(STAT_NAMES)}
_NAMES = _load("names.json")
for _s in GAMEDATA["sections"]:
    for _e in _s["entries"]:
        _NAMES.setdefault(_e["token"], _e["name"])


def analyze_parsed(d, only=None):
    """d = fch.parse() result. One dict per section: id, title, total, done, entries [{token, name, count, biome}]."""
    out = []
    for sec in GAMEDATA["sections"]:
        if only and sec["id"] not in only:
            continue
        s = d["sets"][sec["set"]]
        if sec["source"] == "stats":
            count = lambda t: int(s["stats"][_STAT[t]])
        else:
            src = s[sec["source"]]
            count = lambda t: int(src.get(t, 0))
        ents = [{"token": e["token"], "name": e["name"], "count": count(e["token"]), "biome": e["biome"]}
                for e in sec["entries"]]
        out.append({"id": sec["id"], "title": sec["title"], "total": len(ents),
                    "done": sum(1 for e in ents if e["count"] > 0), "entries": ents})
    return out


# Everything else comes from the game's own Achievement assets: stat_map.json (scripts/build_stat_map.py).
STAT_MAP = _load("stat_map.json")


def achievements(d, sections):
    """{api_name: [done, total]} for every Steam achievement. done == total means unlocked."""
    sets = d["sets"]
    secs = {x["id"]: x for x in sections}
    out = {s["achievement"]: [secs[s["id"]]["done"], secs[s["id"]]["total"]]
           for s in GAMEDATA["sections"] if s["id"] in secs}
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


def _nm(tok):
    n = _NAMES.get(tok)
    return n if isinstance(n, str) and n else fch.strip(tok).replace("_", " ").capitalize()


def meta(d):
    """Plain counters from the raw set: {stats: {StatName: v}, kills, harvested, picked_up, eaten, built, worlds}."""
    s = d["sets"][fch.SET_RAW]
    cnt = lambda m: {_nm(k): int(v) for k, v in sorted(m.items(), key=lambda kv: -kv[1]) if v > 0}
    return {"stats": {n: round(v, 1) for n, v in zip(STAT_NAMES, s["stats"]) if v},
            "kills": cnt(s["kills"]), "picked_up": cnt(s["picked_up"]), "eaten": cnt(s["foods_eaten"]),
            "built": cnt(s["pieces_built"]),
            "harvested": {re.sub(r"(?<=[a-z])(?=[A-Z])", " ", k): int(v)
                          for k, v in sorted(s["pickables"].items(), key=lambda kv: -kv[1]) if v > 0},
            "worlds": {k: round(v) for k, v in s["worlds"].items()}}


if __name__ == "__main__":
    print(json.dumps(analyze_parsed(fch.parse(open(sys.argv[1], "rb").read())), indent=1))
