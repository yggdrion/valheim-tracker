"""Tracker receiver + dashboard. stdlib only.
  TRACKER_TOKEN=secret python server.py [--port 8080] [--state state]
POST /upload?name=X (X-Token header, body = raw .fch bytes) -> stores analysed progress
GET  /                -> dashboard      GET /api/characters, /api/character/<name>
"""
import argparse, hashlib, hmac, json, os, re, struct, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse, parse_qs

import analyze, mapdata, worlds
import fch

HERE = os.path.dirname(os.path.abspath(__file__))
TOKEN = os.environ.get("TRACKER_TOKEN", "")
MAX_BODY = 5_000_000
NAME_OK = re.compile(r"^[A-Za-z0-9_. -]{1,64}$")
STATE = "state"
STALE = float(os.environ.get("STALE_MINUTES", "10")) * 60   # a character is 'resting' (offline) after this long without contact
# version of the analysis code+tables: a stored character from an older version is re-uploaded by the client
AV = hashlib.sha1(b"".join(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), f), "rb").read()
                           for f in ("analyze.py", "stat_map.json", "stat_names.json", "names.json", "mapdata.py", "fch.py", "gamedata.json"))).hexdigest()[:12]
SEEN = {}   # name -> last contact (upload or heartbeat), in memory
WORLD = os.environ.get("WORLD_DIR", "")   # folder of ONE world, e.g. /config/worlds_local/roota (mounted read-only)
_wcache = (None, None)

def world():
    global _wcache
    n = worlds.latest_save(WORLD)
    if _wcache[0] != n:
        _wcache = (n, worlds.read_world(WORLD))
    return _wcache[1]

_lcache = (None, None)


def world_locations():
    global _lcache
    n = worlds.latest_save(WORLD)
    if _lcache[0] != n:
        _lcache = (n, worlds.locations(WORLD))
    return _lcache[1]


def world_map():
    """Everything the map page needs for the shown world: seed, locations, and each character's map data in it."""
    w = world()
    locs = world_locations()
    names = sorted({n for n, _x, _z in locs})
    idx = {n: i for i, n in enumerate(names)}
    players = []
    for fn in sorted(os.listdir(STATE)):
        if fn.endswith(".map.json"):
            name = fn[:-9]
            with open(os.path.join(STATE, fn), encoding="utf8") as f:
                m = json.load(f).get(w["uid"])
            if m and m.get("explored"):
                players.append({"name": name} | m)
    return {"world": {k: w[k] for k in ("name", "seed", "seed_int", "uid", "gen_version")},
            "location_names": names, "locations": [[idx[n], x, z] for n, x, z in locs], "players": players}


def load(name):
    with open(os.path.join(STATE, name + ".json"), encoding="utf8") as f:
        return json.load(f)

class H(BaseHTTPRequestHandler):
    def send(self, code, body, ctype="application/json"):
        b = body if isinstance(body, bytes) else json.dumps(body).encode()
        self.send_response(code); self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(b))); self.end_headers(); self.wfile.write(b)

    def do_POST(self):
        u = urlparse(self.path)
        if u.path not in ("/upload", "/heartbeat"): return self.send(404, {"error": "not found"})
        if not TOKEN or not hmac.compare_digest(self.headers.get("X-Token", ""), TOKEN):
            return self.send(403, {"error": "bad token"})
        name = parse_qs(u.query).get("name", [""])[0]
        if u.path == "/heartbeat":
            if not NAME_OK.match(name) or not os.path.exists(os.path.join(STATE, name + ".json"))                     or load(name).get("av") != AV:
                return self.send(404, {"error": "unknown or outdated, upload again"})
            SEEN[name] = time.time()
            return self.send(200, {"ok": True})
        n = int(self.headers.get("Content-Length", 0))
        if not NAME_OK.match(name) or n <= 0 or n > MAX_BODY:
            return self.send(400, {"error": "bad name or size"})
        raw = self.rfile.read(n)
        try:
            d = fch.parse(raw)
            secs = analyze.analyze_parsed(d)
            ach = analyze.achievements(d, secs)
        except fch.SaveError as e:
            return self.send(400, {"error": str(e)})
        except (struct.error, IndexError, KeyError, UnicodeDecodeError, AssertionError, ValueError, MemoryError, OverflowError):
            return self.send(400, {"error": "not a readable Valheim .fch file"})
        try:
            maps = mapdata.worlds(raw, d)   # the map is a bonus: a layout it cannot read must not block the upload
        except Exception:
            maps = {}
        worlds = sorted(d["sets"][0]["worlds"])
        tmp = os.path.join(STATE, name + ".tmp")
        with open(tmp, "w", encoding="utf8") as f:
            json.dump({"name": name, "updated": time.time(), "av": AV, "version": d["version"], "worlds": worlds, "sections": secs,
                       "achievements": ach, "meta": analyze.meta(d)}, f)
        with open(tmp + "m", "w", encoding="utf8") as f:
            json.dump(maps, f)
        os.replace(tmp + "m", os.path.join(STATE, name + ".map.json"))
        SEEN[name] = time.time()
        os.replace(tmp, os.path.join(STATE, name + ".json"))
        self.send(200, {"ok": True})

    def do_GET(self):
        p = urlparse(self.path).path
        if p == "/":
            return self.send(200, open(os.path.join(HERE, "dashboard.html"), "rb").read(), "text/html; charset=utf-8")
        if p == "/api/characters":
            out = []
            for fn in sorted(os.listdir(STATE)):
                if fn.endswith(".json") and not fn.endswith(".map.json"):
                    c = load(fn[:-5])
                    seen = SEEN.get(c["name"]) or c["updated"]   # after a restart: the last upload
                    out.append({k: c[k] for k in ("name", "updated", "version", "worlds")} |
                               {"seen": seen, "online": time.time() - SEEN.get(c["name"], 0) <= STALE} |
                               {"sections": [{k: s[k] for k in ("id", "title", "done", "total")} for s in c["sections"]]})
            return self.send(200, {"characters": out, "stale_minutes": STALE / 60})
        m = re.fullmatch(r"/fonts/([a-z0-9-]+\.woff2)", p)
        if m and os.path.exists(os.path.join(HERE, "fonts", m[1])):
            b = open(os.path.join(HERE, "fonts", m[1]), "rb").read()
            self.send_response(200); self.send_header("Content-Type", "font/woff2")
            self.send_header("Cache-Control", "public, max-age=86400"); self.send_header("Content-Length", str(len(b)))
            self.end_headers(); return self.wfile.write(b)
        if p == "/api/achievements":
            return self.send(200, open(os.path.join(HERE, "achievements.json"), "rb").read())
        if p == "/worldgen.js":
            return self.send(200, open(os.path.join(HERE, "worldgen.js"), "rb").read(), "text/javascript; charset=utf-8")
        if p == "/api/map":
            try: return self.send(200, world_map())
            except Exception as e: return self.send(200, {"error": str(e)})
        if p == "/api/world":
            try: return self.send(200, world())
            except Exception as e: return self.send(200, {"error": str(e)})
        m = re.fullmatch(r"/api/character/([^/]+)", p)
        if m and NAME_OK.match(m[1]) and os.path.exists(os.path.join(STATE, m[1] + ".json")):
            return self.send(200, load(m[1]))
        self.send(404, {"error": "not found"})

    def log_message(self, *a): pass

if __name__ == "__main__":
    ap = argparse.ArgumentParser(); ap.add_argument("--port", type=int, default=8080)
    ap.add_argument("--host", default="0.0.0.0"); ap.add_argument("--state", default="state")
    a = ap.parse_args(); STATE = a.state; os.makedirs(STATE, exist_ok=True)
    if not TOKEN: raise SystemExit("set TRACKER_TOKEN")
    print(f"listening on {a.host}:{a.port}"); ThreadingHTTPServer((a.host, a.port), H).serve_forever()
