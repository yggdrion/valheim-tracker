"""Dump golden outputs of the Python runtime for the Rust parity tests.
usage: python3 scripts/parity.py dump <characters dir> <world dir> [--out testdata/golden]
Writes manifest.json, <name>.analysis.json, <name>.map.json, world.json, locations.json (testdata/ is gitignored)."""
import argparse, glob, json, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
import analyze, fch, mapdata, worlds


def dump(chars, world, out):
    os.makedirs(out, exist_ok=True)
    write = lambda fn, obj: json.dump(obj, open(os.path.join(out, fn), "w", encoding="utf8"))
    manifest = {"characters": [], "world": os.path.abspath(world)}
    for path in sorted(glob.glob(os.path.join(chars, "*.fch"))):
        if "_backup_" in os.path.basename(path):
            continue
        raw = open(path, "rb").read()
        try:
            d = fch.parse(raw)
        except Exception as e:
            print(f"skip {path}: {e}", file=sys.stderr)
            continue
        if d["version"] < 46:
            continue
        name = os.path.basename(path)[:-4]
        secs = analyze.analyze_parsed(d)
        write(f"{name}.analysis.json", {"version": d["version"], "worlds": sorted(d["sets"][0]["worlds"]), "sections": secs,
                                        "achievements": analyze.achievements(d, secs), "meta": analyze.meta(d)})
        write(f"{name}.map.json", mapdata.worlds(raw, d))
        manifest["characters"].append({"name": name, "path": os.path.abspath(path)})
    write("world.json", worlds.read_world(world))
    write("locations.json", worlds.locations(world))
    write("manifest.json", manifest)
    print(f"{len(manifest['characters'])} characters: " + ", ".join(c["name"] for c in manifest["characters"]))


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["dump"])
    ap.add_argument("characters"); ap.add_argument("world")
    ap.add_argument("--out", default=os.path.join(os.path.dirname(HERE), "testdata", "golden"))
    a = ap.parse_args()
    dump(os.path.expanduser(a.characters), os.path.expanduser(a.world), a.out)
