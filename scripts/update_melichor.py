#!/usr/bin/env python3
"""Check / pull upstream Melichor/ValheimSaveParser into vendor/ and report what else may need porting.

  python scripts/update_melichor.py                 # dry run: fetch upstream, report drift
  python scripts/update_melichor.py --apply         # also copy achi_hunter.py to vendor/ + write vendor/UPSTREAM.json
  python scripts/update_melichor.py --apply --fch some.fch   # + parity check Go parser vs reference on a real save

Read-only on game files. Needs git. Exit 1 on any FAIL.
"""
import argparse, ast, hashlib, json, os, re, subprocess, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CACHE = os.path.join(ROOT, ".cache", "melichor")
REPO = "https://github.com/Melichor/ValheimSaveParser"
VENDOR = os.path.join(ROOT, "vendor", "achi_hunter.py")
META = os.path.join(ROOT, "vendor", "UPSTREAM.json")
bad = 0


def git(*a, cwd=CACHE):
    return subprocess.check_output(["git", *a], cwd=cwd, text=True, encoding="utf8").strip()


def say(level, msg):
    global bad
    bad += level == "FAIL"
    print(f"[{level:4}] {msg}")


def fetch():
    if not os.path.isdir(os.path.join(CACHE, ".git")):
        os.makedirs(os.path.dirname(CACHE), exist_ok=True)
        subprocess.check_call(["git", "clone", "-q", REPO, CACHE])
    else:
        git("fetch", "-q", "origin")
        git("checkout", "-q", "--detach", "origin/HEAD")
    return git("rev-parse", "HEAD"), git("log", "-1", "--format=%cI")


def facts(src):
    """What the port depends on, read from upstream source without importing it."""
    tree = ast.parse(src)
    const, parse_src, reader_src = {}, "", ""
    for n in tree.body:
        if isinstance(n, ast.Assign) and len(n.targets) == 1 and isinstance(n.targets[0], ast.Name):
            try:
                const[n.targets[0].id] = ast.literal_eval(n.value)
            except Exception:
                pass
        elif isinstance(n, ast.FunctionDef) and n.name == "parse":
            parse_src = ast.get_source_segment(src, n)
        elif isinstance(n, ast.ClassDef) and n.name == "Reader":
            reader_src = ast.get_source_segment(src, n)
    h = lambda s: hashlib.sha256(s.encode()).hexdigest()[:16]
    keep = ("MIN_PROFILE_VERSION", "TESTED_PROFILE_VERSION", "STAT_COUNT", "SET_COUNT", "GAME", "SET_ANY", "SET_NORMAL", "SET_HARD")
    return {"parse_hash": h(parse_src), "reader_hash": h(reader_src),
            "constants": {k: const[k] for k in keep if k in const},
            "death_kinds": const.get("DEATH_KINDS", {}), "boss_tokens": sorted(const.get("BOSSES", {})),
            "sections": sorted(re.findall(r'dict\(id="([a-z-]+)"', src))}


def ours():
    go = open(os.path.join(ROOT, "fchparse", "fchparse.go"), encoding="utf8").read()
    g = lambda n: int(re.search(rf"{n}\s*=\s*(\d+)", go)[1])
    an = ast.parse(open(os.path.join(ROOT, "analyze.py"), encoding="utf8").read())
    sec = {}
    for n in an.body:
        if isinstance(n, ast.Assign) and isinstance(n.targets[0], ast.Name) and n.targets[0].id == "SECTION_ACH":
            sec = ast.literal_eval(n.value)
    sm = json.load(open(os.path.join(ROOT, "stat_map.json"), encoding="utf8"))
    bosses = [t for e in sm.values() for t, _ in e.get("enemies", [])]
    return {"go": {"MIN_PROFILE_VERSION": g("MinVersion"), "STAT_COUNT": g("StatCount"), "SET_COUNT": g("SetCount")},
            "SECTION_ACH": sec, "stat_map": sm, "boss_tokens": bosses}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--apply", action="store_true")
    ap.add_argument("--fch", help="a real .fch for the Go-vs-Python parity check (read-only)")
    a = ap.parse_args()

    sha, date = fetch()
    up_src = open(os.path.join(CACHE, "achi_hunter.py"), encoding="utf8").read()
    up = facts(up_src)
    old = json.load(open(META, encoding="utf8")) if os.path.exists(META) else None

    print(f"upstream {sha[:10]} ({date[:10]}), game {up['constants'].get('GAME')}, profile v{up['constants'].get('MIN_PROFILE_VERSION')}+")
    if old:
        if old["sha"] == sha:
            say("OK", "vendor/ already at this upstream commit")
        else:
            say("INFO", f"upstream moved {old['sha'][:10]} -> {sha[:10]}:")
            try:
                print(git("log", "--format=  %h %ad %s", "--date=short", f"{old['sha']}..{sha}", "--", "achi_hunter.py", "data"))
            except subprocess.CalledProcessError:
                say("WARN", "old sha not in history (force-push?)")
        for k in ("parse_hash", "reader_hash"):
            if old.get(k) != up[k]:
                say("WARN", f"upstream {k[:-5]}() changed -> re-read it and port the difference to fchparse/fchparse.go")
        for k, v in up["constants"].items():
            if old["constants"].get(k) != v:
                say("INFO", f"constant {k}: {old['constants'].get(k)!r} -> {v!r}")
    else:
        say("INFO", "no vendor/UPSTREAM.json yet (first run)")

    # does our code still match upstream's layout and indices?
    me = ours()
    for k, v in me["go"].items():
        if up["constants"].get(k) != v:
            say("FAIL", f"Go parser {k}={v} but upstream {k}={up['constants'].get(k)} -> update fchparse/fchparse.go")
    dk = up["death_kinds"]
    sm = me["stat_map"]
    if sm["DeathByTree"]["stats"][0][0] != dk.get("Tree") or sm["DeathByObliterator"]["stats"][0][0] != dk.get("Incinerator"):
        say("FAIL", "stat_map.json death stat indices differ from upstream DEATH_KINDS -> rerun the stat-map scripts (see skill)")
    if not set(me["boss_tokens"]) <= set(up["boss_tokens"]):
        say("FAIL", f"boss tokens not upstream: {sorted(set(me['boss_tokens']) - set(up['boss_tokens']))} -> rerun the stat-map scripts")
    gone = [s for s in me["SECTION_ACH"].values() if s not in up["sections"]]
    if gone:
        say("FAIL", f"sections no longer upstream: {gone} -> update SECTION_ACH in analyze.py")
    new = [s for s in up["sections"] if s not in me["SECTION_ACH"].values()]
    if new:
        say("INFO", f"upstream sections we do not map to a Steam achievement: {new}")
    if not bad:
        say("OK", "Go constants, stat_map.json and analyze.py mappings match upstream")

    if a.apply:
        open(VENDOR, "w", encoding="utf8", newline="").write(up_src)
        json.dump({"repo": REPO, "sha": sha, "date": date, **up}, open(META, "w", encoding="utf8"), indent=1)
        say("OK", "copied achi_hunter.py to vendor/ and wrote vendor/UPSTREAM.json")
        if a.fch:
            r = subprocess.run([sys.executable, os.path.join(ROOT, "analyze.py"), "--check", a.fch], cwd=ROOT,
                               capture_output=True, text=True, encoding="utf8")
            say("OK" if r.returncode == 0 else "FAIL", "parity: " + (r.stdout.strip() or r.stderr.strip().splitlines()[-1])[:200])
    elif a.fch:
        say("INFO", "--fch only runs together with --apply (parity uses vendor/)")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
