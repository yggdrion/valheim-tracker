"""Read a Valheim 1.0 character file (.fch). Our own reader, written from the game's PlayerProfile.SavePlayerToDisk.

File: int32 payload length | payload | int32 hash length | hash. Payload (little-endian; strings are 7-bit length
prefixed UTF-8; a "dict" is int32 count + (string key, float32 value) pairs):

    int32 profile version (47 in 1.0.30), int32 stat count (205), int32 set count (10)
    per stat set, indexed by the game's DifficultyRequirement enum
    (0 RawStats, 1 Any, 2 Hammer, 3 Casual, 4 VeryEasy, 5 Easy, 6 Default, 7 Hard, 8 VeryHard, 9 Hardcore):
        float32[stat count]  PlayerStatType values (stat_names.json)
        dict known worlds (seconds played), dict known world keys, dict known commands
        int32 5, then 5 enemy-kill dicts: all, unarmed, magic, ranged, melee
        dict items picked up, dict items crafted, dict pickables, dict foods eaten, dict pieces placed
    bool first spawn
    int32 world count, per world: int64 uid, bool+vec3 custom spawn, bool+vec3 logout, bool+vec3 death,
        vec3 home, bool has map + byte[] map data (see mapdata.py)
    string name, int64 player id, string start seed, bool used cheats, int64 created (unix), bool + byte[] player data
A kill or craft counts in set 0 (raw) and, when achievements are allowed, in set 1 (Any) and every set from Casual up
to the difficulty it happened on: set N means "this difficulty or harder".
"""
import struct

MIN_VERSION = 46          # oldest profile layout with these stat sets (Valheim 1.0)
TESTED_VERSION = 47       # Valheim 1.0.30
STAT_COUNT = 205
SET_COUNT = 10
SET_RAW, SET_ANY, SET_NORMAL, SET_HARD = 0, 1, 6, 7
KILL_KINDS = ("all", "unarmed", "magic", "ranged", "melee")


class SaveError(ValueError):
    """Not a character file this reader understands (the message says why)."""


class Reader:
    def __init__(self, data, pos=0):
        self.d, self.p = data, pos

    def _take(self, fmt):
        v = struct.unpack_from(fmt, self.d, self.p)
        self.p += struct.calcsize(fmt)
        return v

    def i32(self): return self._take("<i")[0]
    def i64(self): return self._take("<q")[0]
    def f32(self): return self._take("<f")[0]
    def vec3(self): return self._take("<3f")
    def bool(self): return self._take("<?")[0]

    def string(self):
        n = shift = 0
        while True:
            b = self.d[self.p]
            self.p += 1
            n |= (b & 0x7F) << shift
            shift += 7
            if b < 0x80:
                break
        s = self.d[self.p:self.p + n].decode("utf8")
        self.p += n
        return s

    def bytes(self):
        n = self.i32()
        b = self.d[self.p:self.p + n]
        self.p += n
        return b

    def dict(self):
        out = {}
        for _ in range(self.i32()):
            k = self.string()
            out[k] = self.f32()
        return out


def payload(raw):
    n, = struct.unpack_from("<i", raw, 0)
    if not 0 < n <= len(raw) - 4:
        raise SaveError("not a Valheim character file (bad length)")
    return raw[4:4 + n]


def parse(raw):
    """{"version", "sets": [ {stats, worlds, world_keys, commands, kills, kills_by, picked_up, crafted, pickables,
    foods_eaten, pieces_built} * 10 ], "rest": offset of the per-world data}"""
    data = payload(bytes(raw))
    r = Reader(data)
    version = r.i32()
    if not 1 <= version <= 1000:
        raise SaveError("not a Valheim character file")
    if version < MIN_VERSION:
        raise SaveError(f"this save uses profile version {version}; the tracker reads version {MIN_VERSION} and newer "
                        f"(Valheim 1.0): load and save the character once in a current game")
    n_stats, n_sets = r.i32(), r.i32()
    if (n_stats, n_sets) != (STAT_COUNT, SET_COUNT):
        raise SaveError(f"this save has {n_stats} stats in {n_sets} sets, the tracker expects {STAT_COUNT} in "
                        f"{SET_COUNT}: the game's save layout changed, the tracker needs an update")
    sets = []
    for _ in range(n_sets):
        s = {"stats": struct.unpack_from(f"<{n_stats}f", data, r.p)}
        r.p += 4 * n_stats
        s["worlds"], s["world_keys"], s["commands"] = r.dict(), r.dict(), r.dict()
        kills = [r.dict() for _ in range(r.i32())]
        s["kills"] = kills[0]
        s["kills_by"] = dict(zip(KILL_KINDS, kills))
        for k in ("picked_up", "crafted", "pickables", "foods_eaten", "pieces_built"):
            s[k] = r.dict()
        sets.append(s)
    return {"version": version, "sets": sets, "rest": r.p}


def worlds(raw, parsed=None):
    """[(uid, {spawn, logout, death: (x, y, z) or None, home: (x, y, z), map: bytes or None}), ...]"""
    data = payload(bytes(raw))
    r = Reader(data, (parsed or parse(raw))["rest"])
    r.bool()                                     # first spawn
    out = []
    for _ in range(r.i32()):
        uid = r.i64()
        w = {}
        for k in ("spawn", "logout", "death"):
            has = r.bool()
            v = r.vec3()
            w[k] = v if has else None
        w["home"] = r.vec3()
        w["map"] = r.bytes() if r.bool() else None
        out.append((uid, w))
    return out


def strip(token):
    """'$item_wood' -> 'wood' (fallback display name)."""
    for pre in ("$enemy_", "$item_", "$piece_"):
        if token.startswith(pre):
            return token[len(pre):]
    return token
