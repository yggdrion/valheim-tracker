"""Map data from a .fch (what achi_hunter does not read): per world the explored area, pins and spawn/bed/death points.

Layout after the stat sets (Valheim 1.0, profile version 46):
    byte ?, int32 world count, per world:
        int64 world uid | bool+vec3 custom spawn | bool+vec3 logout | bool+vec3 death | vec3 home
        bool has map | byte[] map: int32 version, byte[] gzip( int32 size, bool[size*size] explored,
                                   bool[size*size] explored by others (shared map), int32 pins, pins... )
    pin: string name, vec3 pos, int32 type, bool checked, int64 owner, string author
Map pixels are 12 m; pixel (size/2, size/2) is the world centre, x east, row = z north.
"""
import base64, gzip, struct, zlib
import achi_hunter as ah

PIN_DEATH, PIN_BOSS = 4, 9


def _after_sets(raw):
    data = raw[4:4 + struct.unpack_from("<i", raw, 0)[0]]
    r = ah.Reader(data)
    r.i32(); n_stats = r.i32(); n_sets = r.i32()
    for _ in range(n_sets):
        r.p += 4 * n_stats
        r.sdict(); r.sdict(); r.sdict()
        for _ in range(r.i32()):
            r.sdict()
        for _ in range(5):
            r.sdict()
    return data, r


def _map(blob):
    _ver, n = struct.unpack_from("<ii", blob, 0)
    z = gzip.decompress(blob[8:8 + n])
    size, = struct.unpack_from("<i", z, 0)
    cells = size * size
    explored = bytes(a | b for a, b in zip(z[4:4 + cells], z[4 + cells:4 + 2 * cells]))  # own + shared map
    bits = bytearray(cells // 8)
    for i in (i for i, v in enumerate(explored) if v):
        bits[i >> 3] |= 1 << (i & 7)
    r = ah.Reader(z, 4 + 2 * cells)
    pins = []
    for _ in range(r.i32()):
        name = r.string()
        x, _y, zz = struct.unpack_from("<3f", z, r.p); r.p += 12
        typ = r.i32(); r.p += 1 + 8; r.string()
        pins.append({"name": name, "x": round(x), "z": round(zz), "type": typ})
    return {"size": size, "explored": base64.b64encode(zlib.compress(bytes(bits), 9)).decode(), "pins": pins}


def worlds(raw):
    """{world uid (str): {spawn, logout, death, home: [x, z] or None, size, explored (b64 zlib bitmask), pins}}"""
    data, r = _after_sets(raw)
    p = r.p + 1
    n, = struct.unpack_from("<i", data, p); p += 4
    out = {}
    for _ in range(n):
        uid, = struct.unpack_from("<q", data, p); p += 8
        w = {}
        for k in ("spawn", "logout", "death"):
            has = data[p]; x, _y, z = struct.unpack_from("<3f", data, p + 1); p += 13
            w[k] = [round(x), round(z)] if has else None
        x, _y, z = struct.unpack_from("<3f", data, p); p += 12
        w["home"] = [round(x), round(z)] if (x, z) != (0, 0) else None
        has = data[p]; p += 1
        if has:
            ln, = struct.unpack_from("<i", data, p); p += 4
            w.update(_map(data[p:p + ln])); p += ln
        out[str(uid)] = w
    return out


if __name__ == "__main__":
    import sys
    for uid, w in worlds(open(sys.argv[1], "rb").read()).items():
        print(uid, {k: v for k, v in w.items() if k != "explored"}, len(w.get("explored", "")), "b64 bytes")
