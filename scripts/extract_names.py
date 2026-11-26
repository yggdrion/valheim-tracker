#!/usr/bin/env python3
"""Write names.json: the game's English names for $item_/$piece_/$enemy_/$animal_ tokens, from its localization table.
Read-only on the game install. Needs UnityPy (pip install UnityPy, e.g. in .cache/venv). Run after a game patch.

  python scripts/extract_names.py [--game "<steamapps/common/Valheim>"]
"""
import argparse, csv, io, json, os

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DEFAULT = os.path.expanduser("~/.local/share/Steam/steamapps/common/Valheim")
KEEP = ("item_", "piece_", "enemy_", "animal_", "ship_", "tool_")


def main():
    ap = argparse.ArgumentParser(); ap.add_argument("--game", default=DEFAULT)
    ap.add_argument("--out", default=os.path.join(ROOT, "names.json"))
    a = ap.parse_args()
    import UnityPy
    env = UnityPy.load(os.path.join(a.game, "valheim_Data", "resources.assets"))
    for o in env.objects:
        if o.type.name == "TextAsset" and o.peek_name() == "localization":
            d = o.read(); text = d.m_Script if isinstance(d.m_Script, str) else d.m_Script.decode("utf8")
            break
    else:
        raise SystemExit("no localization TextAsset in resources.assets")
    rows = csv.reader(io.StringIO(text))
    col = next(rows).index("English")
    import re
    clean = lambda t: re.sub(r"</?color[^>]*>", "", t).strip()      # some names carry rich-text colour tags
    out = {"$" + r[0]: clean(r[col]) for r in rows
           if len(r) > col and r[0].startswith(KEEP) and r[col].strip() and not r[0].endswith("_description")}
    with open(a.out, "w", encoding="utf8") as f:
        json.dump(dict(sorted(out.items())), f, indent=0, ensure_ascii=False); f.write("\n")
    print(f"{len(out)} names -> {a.out}")


if __name__ == "__main__":
    main()
