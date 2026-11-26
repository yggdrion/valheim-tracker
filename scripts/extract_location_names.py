#!/usr/bin/env python3
"""Update location_names.json: the names behind the location hashes in a world save, found by hashing every
identifier in the game's data files. Read-only on the game install and the world. Run after a game patch,
on a world created with the new version (it only names hashes that occur in that world).

  python scripts/extract_location_names.py <world folder> [--game "<steamapps/common/Valheim>"]
"""
import argparse, glob, json, os, re, struct, sys, zlib

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, ROOT)
import worlds  # noqa: E402

DEFAULT = os.path.expanduser("~/.local/share/Steam/steamapps/common/Valheim")


def main():
    ap = argparse.ArgumentParser(); ap.add_argument("world"); ap.add_argument("--game", default=DEFAULT)
    a = ap.parse_args()
    hashes = {int(n[1:], 16) for n, _x, _z in worlds.locations(a.world) if n.startswith("#")}
    path = os.path.join(ROOT, "location_names.json")
    names = set(json.load(open(path)))
    if not hashes:
        return print(f"all {len(names)} location names already known for this world")
    for f in glob.glob(os.path.join(a.game, "valheim_Data", "**", "*"), recursive=True):
        if os.path.isfile(f) and os.path.getsize(f) < 400_000_000:
            for m in set(re.findall(rb"[A-Za-z][A-Za-z0-9_]{3,40}", open(f, "rb").read())):
                s = m.decode()
                if worlds.stable_hash(s) in hashes:
                    names.add(s); hashes.discard(worlds.stable_hash(s))
    json.dump(sorted(names), open(path, "w"), indent=0)
    print(f"{len(names)} names -> {path}; still unknown: {len(hashes)}")


if __name__ == "__main__":
    main()
