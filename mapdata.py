"""Map data from a .fch: per world the explored area, pins and spawn/bed/death points (fch.worlds finds them).

The map blob: int32 version, byte[] gzip( int32 size, bool[size*size] explored, bool[size*size] explored by others
(shared map), int32 pin count, pins ); pin: string name, vec3 pos, int32 type, bool checked, int64 owner, string author.
Map pixels are 12 m; pixel (size/2, size/2) is the world centre, x east, row = z north.
"""
import base64, gzip, struct, zlib
import fch

PIN_DEATH, PIN_BOSS = 4, 9


def _map(blob):
    _ver, n = struct.unpack_from("<ii", blob, 0)
    z = gzip.decompress(blob[8:8 + n])
    size, = struct.unpack_from("<i", z, 0)
    cells = size * size
    explored = bytes(a | b for a, b in zip(z[4:4 + cells], z[4 + cells:4 + 2 * cells]))  # own + shared map
    bits = bytearray(cells // 8)
    for i in (i for i, v in enumerate(explored) if v):
        bits[i >> 3] |= 1 << (i & 7)
    r = fch.Reader(z, 4 + 2 * cells)
    pins = []
    for _ in range(r.i32()):
        name = r.string()
        x, _y, zz = struct.unpack_from("<3f", z, r.p); r.p += 12
        typ = r.i32(); r.p += 1 + 8; r.string()
        pins.append({"name": name, "x": round(x), "z": round(zz), "type": typ})
    return {"size": size, "explored": base64.b64encode(zlib.compress(bytes(bits), 9)).decode(), "pins": pins}


def worlds(raw, parsed=None):
    """{world uid (str): {spawn, logout, death, home: [x, z] or None, size, explored (b64 zlib bitmask), pins}}"""
    out = {}
    for uid, w in fch.worlds(raw, parsed):
        xz = lambda v: [round(v[0]), round(v[2])] if v else None
        m = {k: xz(w[k]) for k in ("spawn", "logout", "death")}
        m["home"] = xz(w["home"]) if (w["home"][0], w["home"][2]) != (0, 0) else None
        if w["map"]:
            m.update(_map(w["map"]))
        out[str(uid)] = m
    return out

if __name__ == "__main__":
    import sys
    for uid, w in worlds(open(sys.argv[1], "rb").read()).items():
        print(uid, {k: v for k, v in w.items() if k != "explored"}, len(w.get("explored", "")), "b64 bytes")
