#!/usr/bin/env python3
"""Dump Valheim's own Achievement assets (what each achievement reads) to JSON. Read-only on the game install.

  .cache/venv/Scripts/python.exe scripts/extract_achievements.py [--game "<steamapps/common/Valheim>"] [--out .cache/achievement_defs.json]

Needs UnityPy (pip install UnityPy, in .cache/venv). Takes a few minutes: it scans the asset bundles.
The output is a list of each Achievement asset's full typetree (m_id, m_name, m_statTrigger, goals ...).
scripts/build_stat_map.py turns it into the tables analyze.py uses.
"""
import argparse, glob, json, os, sys, time

DEFAULT = r"C:\Program Files (x86)\Steam\steamapps\common\Valheim"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--game", default=DEFAULT)
    ap.add_argument("--out", default=os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), ".cache", "achievement_defs.json"))
    a = ap.parse_args()
    import UnityPy
    bundles = sorted(glob.glob(os.path.join(a.game, "valheim_Data", "StreamingAssets", "SoftRef", "Bundles", "*")))
    if not bundles:
        sys.exit(f"no bundles under {a.game}")
    found, t0 = [], time.time()
    for n, path in enumerate(bundles, 1):
        try:
            env = UnityPy.load(path)
        except Exception:
            continue
        for o in env.objects:
            if o.type.name != "MonoBehaviour":
                continue
            try:
                tt = o.read_typetree()
            except Exception:
                continue
            if "m_isDynamicallyPopulated" in tt and "m_id" in tt:
                tt["_bundle"] = os.path.basename(path)
                tt["_path_id"] = o.path_id   # other achievements refer to this one by path id
                found.append(tt)
        if n % 20 == 0:
            print(f"{n}/{len(bundles)} bundles, {len(found)} achievements, {int(time.time() - t0)}s", flush=True)
    os.makedirs(os.path.dirname(a.out), exist_ok=True)
    json.dump(found, open(a.out, "w", encoding="utf8"), indent=1, default=str)
    print(f"wrote {len(found)} achievement assets to {a.out}")


if __name__ == "__main__":
    main()
