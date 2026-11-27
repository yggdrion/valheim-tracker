#!/usr/bin/env python3
"""Build gamedata.json from the game's own files: the achievement sections the tracker shows (what each one lists,
which save counter and difficulty set it reads) and the earliest biome of every entry (to sort The Road).

  python scripts/extract_game_data.py <world folder> [--game "<steamapps/common/Valheim>"] [--rescan]

Read-only on the game install and the world. Needs UnityPy (pip install UnityPy, e.g. in .cache/venv) and bun or node.
The world folder (a dedicated-server style folder with _main.N.db2) is used only to see which biome each location type
is placed in (the game's location lists do not say). Run it after a Valheim patch, then check the summary it prints.

Where the lists come from (Achievements.Initialize and ObjectDB in assembly_valheim.dll, Valheim 1.0.30):
  AllItemCraft      every recipe with a crafting station, minus Achievements.m_itemsExcluded
  AllWeaponCraft    every recipe whose item is a weapon (one-/two-handed, left two-handed, bow, torch), minus excluded
  AllFoodCooked     every recipe for an item with food value, plus every food a cooking station makes
  AllFoodEaten      every consumable with food value, minus Achievements.m_consumablesExcluded
  AllBuildPieces    every piece of every build table that allows removing, except repair pieces
  FindAllTrophies   every trophy item, one per name, minus excluded
  KillAllCreatures  every prefab whose Character is flagged killed-for-achievements, one per name (bosses included)
  the rest          the fixed trigger lists in the Achievement assets (bosses, mini-bosses, fish, ways to die, trees)

Biomes: a graph of "what you need to get X" (world placement by vegetation, spawn lists and locations; drops; pickables;
smelting and cooking; recipes and their stations; offspring; summons), solved for the earliest biome in progression
order. curated/biome_links.json adds links the game keeps in attack scripts; curated/biome_overrides.json pins reviewed
exceptions (and the ways to die and trees, which have no place in the world).
"""
import argparse, collections, glob, json, os, pickle, shutil, subprocess, sys, tempfile, time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _tracker import world_json  # noqa: E402

DEFAULT_GAME = os.path.expanduser("~/.local/share/Steam/steamapps/common/Valheim")
ORDER = ["Meadows", "Black Forest", "Ocean", "Swamp", "Mountain", "Plains", "Mistlands", "Ashlands", "Deep North"]
BIT = {1: "Meadows", 8: "Black Forest", 256: "Ocean", 2: "Swamp", 4: "Mountain", 16: "Plains", 512: "Mistlands",
       32: "Ashlands", 64: "Deep North"}
INF = 99
WEAPON_TYPES = {3, 4, 14, 15, 22}           # ItemType OneHandedWeapon, Bow, TwoHandedWeapon, Torch, TwoHandedWeaponLeft
CONSUMABLE, TROPHY = 2, 13

# MonoBehaviours are recognised by their fields (the script names live in a file the bundles only reference)
SIG = {
    "ObjectDB": ("m_items", "m_recipes"), "ZNetScene": ("m_prefabs", "m_nonNetViewPrefabs"),
    "Achievements": ("m_itemsExcluded",), "Achievement": ("m_isDynamicallyPopulated", "m_id"),
    "ItemDrop": ("m_itemData",), "Recipe": ("m_craftingStation", "m_resources", "m_item"),
    "PieceTable": ("m_pieces", "m_canRemovePieces"), "Piece": ("m_repairPiece", "m_category"),
    "Character": ("m_killedForAchievements",), "CharacterDrop": ("m_drops",), "Conversion": ("m_conversion",),
    "CraftingStation": ("m_rangeBuild", "m_craftRequireRoof"), "OfferingBowl": ("m_bossPrefab",),
    "ZoneSystem": ("m_vegetation", "m_locations"), "SpawnSystemList": ("m_spawners",),
    "CreatureSpawner": ("m_creaturePrefab",), "SpawnArea": ("m_prefabs", "m_levelupChance"),
    "SpawnAbility": ("m_spawnPrefab",), "Procreation": ("m_offspring",), "Trader": ("m_items", "m_standRange"),
    "Location": ("m_clearArea", "m_exteriorRadius"), "DungeonGenerator": ("m_themes", "m_maxRooms"),
    "Room": ("m_theme", "m_entrance"), "Pickable": ("m_itemPrefab", "m_amount", "m_respawnTimeMinutes"),
    "Destructible": ("m_spawnWhenDestroyed",), "DropOnDestroyed": ("m_dropWhenDestroyed",),
    "MineRock": ("m_dropItems",), "Container": ("m_defaultItems",), "Beehive": ("m_honeyItem",),
    "SapCollector": ("m_spawnItem", "m_maxLevel", "m_secPerUnit"), "EggGrow": ("m_grownPrefab",),
    "Plant": ("m_grownPrefabs",), "Fish": ("m_baits", "m_pickupItem"),
}


# ---------------------------------------------------------------------------------------------------- collect
def collect(game):
    import UnityPy
    data = os.path.join(game, "valheim_Data")
    files = sorted(glob.glob(os.path.join(data, "StreamingAssets", "SoftRef", "Bundles", "*")))
    files += [os.path.join(data, f) for f in ("level0", "sharedassets0.assets", "resources.assets",
                                              "globalgamemanagers.assets")]
    meta, objs, gos, t0 = {}, {}, {}, time.time()
    for n, f in enumerate(files, 1):
        try:
            env = UnityPy.load(f)
        except Exception:
            continue
        for o in env.objects:
            key = o.assets_file.name
            if key not in meta:
                meta[key] = [e.path for e in o.assets_file.externals]
            if o.type.name == "GameObject":
                try:
                    d = o.read_typetree()
                    gos[(key, o.path_id)] = {"name": d.get("m_Name"), "comps": [
                        (c["component"]["m_FileID"], c["component"]["m_PathID"]) for c in d.get("m_Component", [])]}
                except Exception:
                    pass
            elif o.type.name == "MonoBehaviour":
                try:
                    d = o.read_typetree()
                except Exception:
                    continue
                for cls, keys in SIG.items():
                    if all(k in d for k in keys):
                        d["_cls"] = cls
                        objs[(key, o.path_id)] = d
                        break
        if n % 100 == 0:
            print(f"  {n}/{len(files)} files, {len(objs)} components, {int(time.time() - t0)} s", flush=True)
    return {"files": meta, "objs": objs, "gos": gos}


class Game:
    """The collected objects with reference resolution. Prefab names are unique in Valheim and scene bundles hold
    component-less copies of the prefabs, so objects are linked by GameObject name."""

    def __init__(self, raw):
        self.F, self.O, self.GO = raw["files"], raw["objs"], raw["gos"]
        self.byname = collections.defaultdict(list)
        for k in self.O:
            g = self.go_of(k)
            if g in self.GO:
                self.byname[self.GO[g]["name"]].append(k)

    def res(self, key, p):
        if not isinstance(p, dict) or "m_PathID" not in p or p["m_PathID"] == 0:
            return None
        fid, pid = p["m_FileID"], p["m_PathID"]
        if fid == 0:
            return (key, pid)
        ext = self.F.get(key, [])
        return (ext[fid - 1].split("/")[-1], pid) if fid - 1 < len(ext) else None

    def go_of(self, k):
        return self.res(k[0], self.O[k].get("m_GameObject"))

    def name_of(self, gk):
        return self.GO.get(gk, {}).get("name") if gk else None

    def comps(self, gk, cls=None):
        return [c for c in self.byname.get(self.name_of(gk), []) if cls is None or self.O[c]["_cls"] == cls]

    def comp(self, gk, cls):
        c = self.comps(gk, cls)
        return c[0] if c else None

    def all(self, cls):
        return [(k, v) for k, v in self.O.items() if v["_cls"] == cls]

    def first(self, cls):
        return self.all(cls)[0]

    def item(self, k):
        """ItemDrop component key for a reference to an ItemDrop or its GameObject."""
        if k in self.O and self.O[k]["_cls"] == "ItemDrop":
            return k
        if k in self.O:
            k = self.go_of(k)
        return self.comp(k, "ItemDrop") if k in self.GO else None


def shared(g, ik):
    return g.O[ik]["m_itemData"]["m_shared"]


# ---------------------------------------------------------------------------------------------------- lists
def build_lists(g):
    """The dynamically populated achievements, by the game's rules (see the module docstring)."""
    (odbk, odb), (znk, zn), (ack, ac) = g.first("ObjectDB"), g.first("ZNetScene"), g.first("Achievements")
    items = [ik for ik in (g.item(g.res(odbk[0], p)) for p in odb["m_items"]) if ik]
    def names_of(refs, base):
        out = set()
        for p in refs:
            ik = g.item(g.res(base[0], p))
            if ik:
                out.add(shared(g, ik)["m_name"])
        return out
    excluded, consumables_excluded = names_of(ac["m_itemsExcluded"], ack), names_of(ac["m_consumablesExcluded"], ack)
    recipes = []
    for p in odb["m_recipes"]:
        rk = g.res(odbk[0], p)
        if rk not in g.O:
            continue
        ik = g.item(g.res(rk[0], g.O[rk]["m_item"]))
        if ik and shared(g, ik)["m_name"]:
            recipes.append((g.O[rk], rk, ik))
    L = {}
    L["AllItemCraft"] = {shared(g, ik)["m_name"] for r, rk, ik in recipes if g.res(rk[0], r["m_craftingStation"])}
    L["AllWeaponCraft"] = {shared(g, ik)["m_name"] for r, rk, ik in recipes if shared(g, ik)["m_itemType"] in WEAPON_TYPES}
    L["AllItemCraft"] -= excluded
    L["AllWeaponCraft"] -= excluded
    cooked = {shared(g, ik)["m_name"] for r, rk, ik in recipes if shared(g, ik)["m_food"] > 0}
    for p in zn["m_prefabs"]:                                  # cooking stations: CookingStation.m_conversion
        for ck in g.comps(g.res(znk[0], p), "Conversion"):
            v = g.O[ck]
            if "m_addFoodSwitch" not in v:
                continue                                       # a fermenter or smelter, not a cooking station
            for c in v["m_conversion"]:
                to = g.item(g.res(ck[0], c["m_to"]))
                if to and shared(g, to)["m_food"] > 0 and shared(g, to)["m_itemType"] == CONSUMABLE:
                    cooked.add(shared(g, to)["m_name"])
    L["AllFoodCooked"] = cooked
    L["AllFoodEaten"] = {shared(g, ik)["m_name"] for ik in items
                         if shared(g, ik)["m_itemType"] == CONSUMABLE and shared(g, ik)["m_food"] > 0} - consumables_excluded
    pieces = set()
    for ik in items:
        t = g.res(ik[0], shared(g, ik).get("m_buildPieces")) if shared(g, ik).get("m_buildPieces") else None
        if t in g.O and g.O[t]["m_canRemovePieces"]:
            for p in g.O[t]["m_pieces"]:
                for pk in g.comps(g.res(t[0], p), "Piece"):    # a few prefab names are shared by two pieces
                    if not g.O[pk]["m_repairPiece"]:
                        pieces.add(g.O[pk]["m_name"])
    L["AllBuildPieces"] = pieces
    L["FindAllTrophies"] = {shared(g, ik)["m_name"] for ik in items if shared(g, ik)["m_itemType"] == TROPHY} - excluded
    creatures = set()
    for p in zn["m_prefabs"]:
        ck = g.comp(g.res(znk[0], p), "Character")
        if ck and g.O[ck]["m_killedForAchievements"] == 1:
            creatures.add(g.O[ck]["m_name"])
    L["KillAllCreatures"] = L["KillAllCreaturesHard"] = creatures
    return L


# ---------------------------------------------------------------------------------------------------- sections
# section id -> (achievement, title); the dashboard and the Road key on these ids
SECTIONS = [
    ("crafted", "AllItemCraft", "Items crafted"), ("weapons", "AllWeaponCraft", "Weapons crafted"),
    ("cooked", "AllFoodCooked", "Food cooked"), ("food-eaten", "AllFoodEaten", "Food eaten (The Gourmet)"),
    ("built", "AllBuildPieces", "Pieces built"), ("deaths", "DeathByAllTypes", "Ways to die"),
    ("tree-deaths", "DeathByTreeAll", "Killed by each kind of tree"), ("enemies", "KillAllCreatures", "Creatures killed"),
    ("enemies-hard", "KillAllCreaturesHard", "Creatures killed"), ("bosses-any", "AllBosses", "Bosses killed"),
    ("bosses-normal", "AllBossesNormal", "Bosses killed"), ("bosses-hard", "AllBossesHard", "Bosses killed"),
    ("minibosses", "AllMiniBosses", "Mini-bosses killed"), ("fishing", "GrindFish", "Fish caught"),
    ("trophies", "FindAllTrophies", "Trophies collected"),
]
SET_LABEL = {1: "any difficulty", 6: "Normal or higher", 7: "Hard"}
TRIGGERS = [("m_enemyStatsTriggers", "kills"), ("m_itemCraftTriggers", "crafted"), ("m_foodEatenTriggers", "foods_eaten"),
            ("m_piecePlacedTriggers", "pieces_built"), ("m_itemPickupTriggers", "picked_up"),
            ("m_pickableTriggers", "pickables"), ("m_statTrigger", "stats")]


def pretty_stat(name):
    """'DeathByEnemyHit' -> 'Enemy hit', 'TreeYggdrasilShoot' -> 'Yggdrasil shoot'."""
    import re
    for pre in ("DeathBy", "Tree"):
        if name.startswith(pre) and name != pre:
            name = name[len(pre):]
    words = re.sub(r"(?<=[a-z])(?=[A-Z])", " ", name).split()
    return " ".join([words[0]] + [w.lower() for w in words[1:]]) if words else name


def build_sections(g, lists, names, stat_names):
    ach = {v["m_id"]: v for k, v in g.all("Achievement")}
    out = []
    for sid, api, title in SECTIONS:
        a = ach[api]
        source, trig = next((s, a[f]) for f, s in TRIGGERS if a.get(f))
        if api in lists:
            tokens = sorted(lists[api])
        elif source == "stats":
            tokens = [stat_names[t["m_stat"]] for t in trig]
        else:
            tokens = [t["m_stat"] for t in trig]
        st = a["m_difficultyRequirement"]
        entries = [{"token": t, "name": pretty_stat(t) if source == "stats" else (names.get(t) or t.split("_", 1)[-1])}
                   for t in tokens]
        out.append({"id": sid, "achievement": api, "title": f"{title} ({SET_LABEL.get(st, st)})"
                    if sid not in ("deaths", "tree-deaths", "food-eaten", "fishing", "trophies") else title,
                    "set": st, "source": source, "entries": entries})
    return out


# ---------------------------------------------------------------------------------------------------- biomes
def location_biomes(world):
    """{location name: {biome: count}} from a real world's placed locations and our port of the biome generator."""
    j = world_json(world)
    w, locs = j["world"], j["locations"]
    runner = shutil.which("bun") or shutil.which("node")
    if not runner:
        raise SystemExit("needs bun or node to run worldgen.js")
    with tempfile.TemporaryDirectory() as tmp:
        inp, outp, js = (os.path.join(tmp, f) for f in ("in.json", "out.json", "run.mjs"))
        json.dump({"seed": w["seed_int"], "gen": w["gen_version"], "locs": locs}, open(inp, "w"))
        open(js, "w").write(
            f"import {{worldGen, BIOME}} from {json.dumps(os.path.join(ROOT, 'worldgen.js'))};\n"
            "import fs from 'fs';\nconst D = JSON.parse(fs.readFileSync(process.argv[2])); const W = worldGen(D.seed, D.gen);\n"
            "const c = {}; for (const [n, x, z] of D.locs) { const b = BIOME[W.biome(x, z)]; (c[n] ??= {})[b] = (c[n][b] || 0) + 1 }\n"
            "fs.writeFileSync(process.argv[3], JSON.stringify(c));\n")
        subprocess.run([runner, js, inp, outp], check=True)
        return json.load(open(outp))


def rank_of_mask(m):
    r = [ORDER.index(n) for b, n in BIT.items() if m & b]
    return min(r) if r else INF


def solve_biomes(g, locbiome, links, overrides):
    """Earliest biome rank of items, pieces and creatures: node -> [(base rank, [needed nodes])], solved to a fixpoint."""
    O, res, name_of, go_of = g.O, g.res, g.name_of, g.go_of
    ways = collections.defaultdict(list)

    def way(node, base=0, reqs=()):
        if node is not None and node[1] is not None:
            ways[node].append((base, [r for r in reqs if r is not None]))

    def go_node(gk):
        """A GameObject (or a component on it) -> the item or creature it is, else the object itself."""
        if gk in O and gk not in g.GO:
            gk = go_of(gk)
        if gk is None:
            return None
        for c in g.comps(gk):
            if O[c]["_cls"] == "ItemDrop": return ("item", O[c]["m_itemData"]["m_shared"]["m_name"])
            if O[c]["_cls"] == "Character": return ("creature", O[c]["m_name"])
        return ("go", name_of(gk))

    def item_node(k):
        ik = g.item(k) if k else None
        return ("item", shared(g, ik)["m_name"]) if ik else go_node(k)

    def world(n):
        return n if n is None or n[0] != "go" else ("world", n[1])

    def drops(key, table):
        for d in (table or {}).get("m_drops", []) if isinstance(table, dict) else []:
            yield res(key, d.get("m_item"))

    def pptrs(x, key):
        if isinstance(x, dict):
            if "m_PathID" in x and "m_FileID" in x:
                r = res(key, x)
                if r: yield r
            else:
                for v in x.values(): yield from pptrs(v, key)
        elif isinstance(x, list):
            for v in x: yield from pptrs(v, key)

    # what objects yield: as placed in the world (drops, pickables) or when built by a player too (production)
    for k, v in O.items():
        gk, cl = go_of(k), v["_cls"]
        if not gk:
            continue
        outs, made = [], []
        if cl == "Pickable": outs.append(res(k[0], v.get("m_itemPrefab")))
        if cl in ("DropOnDestroyed", "MineRock", "Destructible"):
            for f in ("m_dropWhenDestroyed", "m_dropItems"):
                outs += list(drops(k[0], v.get(f)))
            for f in ("m_spawnWhenDestroyed", "m_logPrefab", "m_subLogPrefab", "m_stubPrefab"):
                if f in v: outs.append(res(k[0], v[f]))
        if cl == "CharacterDrop":
            outs += [res(k[0], d.get("m_prefab")) for d in v.get("m_drops", [])]
        if cl == "SpawnArea":
            outs += [res(k[0], p.get("m_prefab")) for p in v.get("m_prefabs", [])]
        if cl == "Beehive": made.append(res(k[0], v.get("m_honeyItem")))
        if cl == "SapCollector": made.append(res(k[0], v.get("m_spawnItem")))
        if cl in ("EggGrow", "Procreation"): made.append(res(k[0], v.get("m_grownPrefab") or v.get("m_offspring")))
        if cl == "Fish": made.append(res(k[0], v.get("m_pickupItem")))
        if cl == "Plant":
            for p in v.get("m_grownPrefabs", []):
                way(("world", name_of(res(k[0], p))), 0, [("go", name_of(gk))])     # a grown plant is a world object
        me = go_node(gk)
        src = me if me[0] != "go" else ("world", me[1])
        for o in outs:
            way(world(go_node(o)), 0, [src])
        for o in made:
            way(go_node(o), 0, [me])
        way(("go", name_of(gk)), 0, [("world", name_of(gk))])
        here = ("world", name_of(gk))                       # spawners, altars and traders work wherever placed
        if cl == "CreatureSpawner": way(go_node(res(k[0], v["m_creaturePrefab"])), 0, [here])
        if cl == "OfferingBowl":
            way(go_node(res(k[0], v["m_bossPrefab"])), 0, [here, item_node(res(k[0], v.get("m_bossItem")))])
        if cl == "Trader":
            for it in v.get("m_items", []):
                way(go_node(res(k[0], it.get("m_prefab"))), 0, [here])

    # summons: an item (or the items a creature attacks with) whose attack spawns a creature
    spawns = collections.defaultdict(list)
    for k, v in g.all("SpawnAbility"):
        sp = v.get("m_spawnPrefab")
        for p in (sp if isinstance(sp, list) else [sp]):
            spawns[name_of(go_of(k))].append(res(k[0], p))
    def summons(owner, key, data):
        for r in pptrs(data, key):
            for sp in spawns.get(name_of(r), []):
                way(go_node(sp), 0, [owner])
    for k, v in g.all("ItemDrop"):
        summons(("item", shared(g, k)["m_name"]), k[0], shared(g, k))
    for k, v in g.all("Character"):
        for f in ("m_defaultItems", "m_randomWeapon", "m_randomArmor", "m_randomShield", "m_randomSets", "m_randomItems"):
            for r in pptrs(v.get(f), k[0]):                 # carried items are not loot, but their attacks may summon
                for ik in g.comps(r, "ItemDrop"):
                    summons(("creature", v["m_name"]), ik[0], shared(g, ik))

    # world placement: vegetation and spawn lists (spawns behind a world key need the boss that sets it)
    keyboss = {v["m_defeatSetGlobalKey"].lower(): ("creature", v["m_name"])
               for k, v in g.all("Character") if v.get("m_defeatSetGlobalKey")}
    for k, v in g.all("ZoneSystem"):
        for veg in v["m_vegetation"]:
            if veg.get("m_enable"):
                way(world(go_node(res(k[0], veg["m_prefab"]))), rank_of_mask(veg["m_biome"]))
    for k, v in g.all("SpawnSystemList"):
        for s in v["m_spawners"]:
            key = (s.get("m_requiredGlobalKey") or "").lower()
            if s.get("m_enabled", 1) and (not key or key in keyboss):
                way(go_node(res(k[0], s["m_prefab"])), rank_of_mask(s["m_biome"]), [keyboss[key]] if key else [])
    # (world events are left out: their biome masks say where a raid may hit, not where a creature lives)

    # locations: everything in a location's bundle is placed in that location's biome; dungeon rooms via their theme
    locbio = {n: ORDER.index(max(c, key=c.get)) for n, c in locbiome.items()}   # where most copies of a type are
    byfile, gofile = collections.defaultdict(list), collections.defaultdict(set)
    for k in O: byfile[k[0]].append(k)
    for (f, _), d in g.GO.items():
        if d["name"]: gofile[f].add(d["name"])
    scene = {f for f, ks in byfile.items() if any(O[k]["_cls"] in ("ZNetScene", "ObjectDB") for k in ks)}
    theme = collections.defaultdict(lambda: INF)
    def place(f, r):
        for n in gofile[f]:
            way(("world", n), r)
        for k in byfile[f]:
            if O[k]["_cls"] == "Container":
                for o in drops(k[0], O[k].get("m_defaultItems")):
                    way(go_node(o), r)
    for f, ks in byfile.items():
        if f in scene:
            continue
        names = [n for n in (name_of(go_of(k)) for k in ks if O[k]["_cls"] == "Location") if n in locbio]
        if names:
            r = min(locbio[n] for n in names)
            place(f, r)
            for k in ks:
                if O[k]["_cls"] == "DungeonGenerator":
                    for b in range(32):
                        if O[k]["m_themes"] & (1 << b): theme[1 << b] = min(theme[1 << b], r)
    for f, ks in byfile.items():
        for k in ks:
            if O[k]["_cls"] == "Room":
                r = min([theme[b] for b in list(theme) if O[k]["m_theme"] & b] or [INF])
                if r < INF: place(f, r)

    # recipes (materials + station), conversions (input + station), pieces (materials + station + the tool)
    def station(key, p):
        s = res(key, p)
        return ("go", name_of(go_of(s) if s in O else s)) if s else None
    for k, v in g.all("Recipe"):
        way(item_node(res(k[0], v["m_item"])), 0, [item_node(res(k[0], r["m_resItem"])) for r in v.get("m_resources", [])
                                                   if r.get("m_amount", 1) > 0] + [station(k[0], v.get("m_craftingStation"))])
    for k, v in g.all("Conversion"):
        for c in v["m_conversion"]:
            way(item_node(res(k[0], c["m_to"])), 0, [item_node(res(k[0], c["m_from"])), ("go", name_of(go_of(k)))])
    for k, v in g.all("Piece"):
        way(("piece", v["m_name"]), 0, [item_node(res(k[0], r["m_resItem"])) for r in v.get("m_resources", [])
                                        if r.get("m_amount", 1) > 0] + [station(k[0], v.get("m_craftingStation"))])
        way(("go", name_of(go_of(k))), 0, [("piece", v["m_name"])])
    tools = collections.defaultdict(list)
    for k, v in g.all("ItemDrop"):
        t = res(k[0], shared(g, k).get("m_buildPieces")) if shared(g, k).get("m_buildPieces") else None
        if t in O:
            for p in O[t]["m_pieces"]:
                for pk in g.comps(res(t[0], p), "Piece"):
                    tools[O[pk]["m_name"]].append(("item", shared(g, k)["m_name"]))

    for child, parents in links.items():                    # curated/biome_links.json
        node = tuple(child.split(":", 1))
        way(node, ORDER.index(parents)) if isinstance(parents, str) else way(node, 0, [tuple(p.split(":", 1)) for p in parents])

    fixed = {tuple(k.split(":", 1)): ORDER.index(b) for k, b in overrides.items()}   # curated/biome_overrides.json
    rank = dict(fixed)
    r_of = lambda n: rank.get(n, INF)
    changed = True
    while changed:
        changed = False
        for n, ws in ways.items():
            if n in fixed:
                continue
            best = min(max([b] + [r_of(q) for q in reqs]) for b, reqs in ws)
            if n[0] == "piece" and n[1] in tools:
                best = max(best, min(r_of(t) for t in tools[n[1]]))
            if best < r_of(n):
                rank[n], changed = best, True
    return {f"{t}:{n}": ORDER[r] for (t, n), r in rank.items() if r < INF}


# ---------------------------------------------------------------------------------------------------- main
KIND = {"kills": "creature", "crafted": "item", "foods_eaten": "item", "pieces_built": "piece", "picked_up": "item",
        "pickables": "item", "stats": "stat"}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("world", help="a world folder with _main.N.db2 (used for the biome of each location type)")
    ap.add_argument("--game", default=DEFAULT_GAME)
    ap.add_argument("--out", default=os.path.join(ROOT, "gamedata.json"))
    ap.add_argument("--rescan", action="store_true", help="read the game's bundles again instead of the cache")
    a = ap.parse_args()
    cache = os.path.join(ROOT, ".cache", "gamedata_collect.pkl")
    if a.rescan or not os.path.exists(cache):
        print("reading the game's asset bundles (a few minutes) ...")
        os.makedirs(os.path.dirname(cache), exist_ok=True)
        pickle.dump(collect(a.game), open(cache, "wb"))
    g = Game(pickle.load(open(cache, "rb")))
    names = json.load(open(os.path.join(ROOT, "names.json"), encoding="utf8"))
    stat_names = json.load(open(os.path.join(ROOT, "stat_names.json")))
    sections = build_sections(g, build_lists(g), names, stat_names)
    biomes = solve_biomes(g, location_biomes(a.world), json.load(open(os.path.join(ROOT, "curated", "biome_links.json"))),
                          json.load(open(os.path.join(ROOT, "curated", "biome_overrides.json"))))
    missing = []
    for s in sections:
        for e in s["entries"]:
            e["biome"] = biomes.get(f"{KIND[s['source']]}:{e['token']}", "")
            if not e["biome"]:
                missing.append(f"{KIND[s['source']]}:{e['token']}")
    json.dump({"generated_from": "Valheim game files (scripts/extract_game_data.py)", "sections": sections},
              open(a.out, "w", encoding="utf8"), indent=1, ensure_ascii=False)
    print(f"{a.out}: " + ", ".join(f"{s['id']} {len(s['entries'])}" for s in sections))
    if missing:
        print(f"no biome for {len(set(missing))} entries (add a link or override in curated/): {sorted(set(missing))}")


if __name__ == "__main__":
    main()
