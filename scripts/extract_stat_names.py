#!/usr/bin/env python3
"""Write stat_names.json: the game's PlayerStatType names, in enum order (index = position in a stat set).
Read-only on the game install. Needs dnfile (pip install dnfile, e.g. in .cache/venv).

  python scripts/extract_stat_names.py [--dll "<Valheim>/valheim_Data/Managed/assembly_valheim.dll"]

Run it after a game patch; analyze.py checks that the count matches achi_hunter.STAT_COUNT.
"""
import argparse, json, os

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DEFAULT = os.path.expanduser("~/.local/share/Steam/steamapps/common/Valheim/valheim_Data/Managed/assembly_valheim.dll")


def main():
    ap = argparse.ArgumentParser(); ap.add_argument("--dll", default=DEFAULT)
    ap.add_argument("--out", default=os.path.join(ROOT, "stat_names.json"))
    a = ap.parse_args()
    import dnfile
    md = dnfile.dnPE(a.dll).net.mdtables
    t = next(t for t in md.TypeDef.rows if str(t.TypeName) == "PlayerStatType")
    fields = [f.row for f in t.FieldList]
    vals = {str(c.Parent.row.Name): int.from_bytes(c.Value.value, "little", signed=True)
            for c in md.Constant.rows if c.Parent.row in fields}
    names = [k for k, v in sorted(vals.items(), key=lambda kv: kv[1]) if k not in ("Count", "None")]
    assert [vals[n] for n in names] == list(range(len(names))), "PlayerStatType values are not 0..N-1"
    with open(a.out, "w", encoding="utf8") as f:
        json.dump(names, f, indent=0); f.write("\n")
    print(f"{len(names)} stat names -> {a.out}")


if __name__ == "__main__":
    main()
