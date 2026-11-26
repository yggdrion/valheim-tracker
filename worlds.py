"""Read-only parser for a Valheim 1.0 world folder (_main.N.fwl2 / .db2 / .ok).
Gives: name, seed, in-game day, global keys (boss kills etc.), world modifiers."""
import glob, os, re, struct, zlib

DAY_SEC = 1800  # ponytail: default Valheim day length, not read from the save
BOSSES = [  # (global key, label)
    ("defeated_eikthyr", "Eikthyr"), ("defeated_gdking", "The Elder"), ("defeated_bonemass", "Bonemass"),
    ("defeated_dragon", "Moder"), ("defeated_goblinking", "Yagluth"), ("defeated_queen", "The Queen"),
    ("defeated_fader", "Fader"),
]
_STR = re.compile(rb"^[A-Za-z0-9_ .:-]{1,80}$")


def latest_save(folder):
    """Highest N that has an .ok marker (the game writes .ok last, so it is a complete save)."""
    ns = [int(m[1]) for f in glob.glob(os.path.join(folder, "_main.*.ok"))
          if (m := re.search(r"_main\.(\d+)\.ok$", f))]
    return max(ns) if ns else None


def _strings(raw, pos):
    """Length-prefixed (<128) strings from pos: return list, or None if not a clean list."""
    n, = struct.unpack_from("<i", raw, pos)
    if not 0 < n < 500:
        return None
    pos += 4
    out = []
    for _ in range(n):
        ln = raw[pos]
        s = raw[pos + 1:pos + 1 + ln]
        if ln >= 128 or not _STR.match(s):
            return None
        out.append(s.decode())
        pos += 1 + ln
    return out


def global_keys(raw):
    """The ZoneSystem key list: int32 count + strings, always containing 'activebosses N'."""
    i = raw.find(b"activebosses ")
    while i >= 0:
        for start in range(max(0, i - 4 - 1), i):  # count sits right before the first string's length byte
            for pos in range(start - 3, start + 1):
                try:
                    keys = _strings(raw, pos) if pos >= 0 else None
                except (struct.error, IndexError):
                    keys = None
                if keys and any(k.startswith("activebosses") for k in keys):
                    return keys
        i = raw.find(b"activebosses ", i + 1)
    return []


def read_fwl2(path):
    d = open(path, "rb").read()
    n, ver = struct.unpack_from("<ii", d, 0)
    pos = 8

    def s():
        nonlocal pos
        ln = d[pos]; pos += 1
        v = d[pos:pos + ln].decode("utf8", "replace"); pos += ln
        return v
    name, seed = s(), s()
    mods = {}
    for m in re.finditer(rb"[\x05-\x40]([a-zA-Z][a-zA-Z_]{2,30}) ([a-z0-9_.:]{1,80})", d[pos:]):
        if m.group(0)[0] == len(m.group(0)) - 1:
            mods[m.group(1).decode().lower()] = m.group(2).decode()
    preset = re.search(rb"preset ([a-z_:]+)", d)
    if preset:
        mods["preset"] = preset.group(1).decode()
    return {"version": ver, "name": name, "seed": seed, "modifiers": mods}


def read_world(folder):
    n = latest_save(folder)
    if n is None:
        raise FileNotFoundError(f"no complete _main.N.ok save in {folder}")
    meta = read_fwl2(os.path.join(folder, f"_main.{n}.fwl2"))
    d = open(os.path.join(folder, f"_main.{n}.db2"), "rb").read()
    net, = struct.unpack_from("<d", d, 4)
    raw = zlib.decompressobj(31).decompress(d[16:])
    keys = global_keys(raw)
    kset = {k.lower() for k in keys}
    bosses = [{"key": k, "name": nm, "defeated": k in kset} for k, nm in BOSSES]
    known = {k for k, _ in BOSSES}
    return {**meta, "save": n, "save_mtime": os.path.getmtime(os.path.join(folder, f"_main.{n}.db2")),
            "day": int(net // DAY_SEC) + 1, "net_seconds": net, "bosses": bosses,
            "keys": sorted(k for k in keys if k.lower() not in known)}


if __name__ == "__main__":
    import json, sys
    print(json.dumps(read_world(sys.argv[1]), indent=1))
