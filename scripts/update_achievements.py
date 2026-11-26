#!/usr/bin/env python3
"""Refresh achievements.json (names, descriptions, icon hashes) from Steam's public achievements page.

  python scripts/update_achievements.py            # dry run: show what changed
  python scripts/update_achievements.py --apply    # write achievements.json

Icons are NOT downloaded; the dashboard hotlinks them from Steam's CDN.
"""
import argparse, json, os, sys, urllib.request

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "achievements.json")
URL = "https://steamcommunity.com/stats/892970/achievements?l=english"
KEYS = ("api_name", "name", "description", "hidden", "icon_achieved", "icon_unachieved", "max_progress")


def fetch():
    req = urllib.request.Request(URL, headers={"User-Agent": "Mozilla/5.0 valheim-tracker"})
    html = urllib.request.urlopen(req, timeout=30).read().decode("utf8", "replace")
    t = html.replace('\\"', '"')                      # the page embeds its data as an escaped JSON string
    i = t.find('"achievements":[')
    if i < 0:
        sys.exit("FAIL: no achievements array in the page (Steam changed its markup) -> adjust fetch()")
    arr, _ = json.JSONDecoder().raw_decode(t[i + len('"achievements":'):])
    return [{k: a.get(k) for k in KEYS} for a in arr if not a.get("archived")]


def tracked():
    sys.path.insert(0, ROOT)
    import analyze
    return set(analyze.SECTION_ACH) | set(analyze.STAT_MAP)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--apply", action="store_true")
    args = ap.parse_args()
    new = fetch()
    old = {a["api_name"]: a for a in json.load(open(OUT, encoding="utf8"))} if os.path.exists(OUT) else {}
    cur = {a["api_name"]: a for a in new}
    print(f"steam lists {len(new)} achievements (we have {len(old)})")
    for k in cur.keys() - old.keys():
        print("  + new:", k, "-", cur[k]["name"])
    for k in old.keys() - cur.keys():
        print("  - gone:", k, "-", old[k]["name"])
    for k in cur.keys() & old.keys():
        d = [f for f in KEYS if cur[k][f] != old[k].get(f)]
        if d:
            print(f"  ~ changed {k}: {', '.join(d)}")
    t = tracked()
    print("  trackable:", len(t & cur.keys()), "of", len(cur), "| mapped but unknown to Steam:", sorted(t - cur.keys()) or "none")
    print("  new and not yet trackable:", sorted(cur.keys() - old.keys() - t) or "none")
    if args.apply:
        json.dump(new, open(OUT, "w", encoding="utf8", newline=""), indent=1, ensure_ascii=False)
        print("wrote", OUT)


if __name__ == "__main__":
    main()
