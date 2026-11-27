"""Shared helpers for the scripts: read a world folder through the Rust core (tracker-server world DIR)."""
import json, os, subprocess, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BIN = os.environ.get("TRACKER_BIN") or os.path.join(ROOT, "client", "target", "release", "tracker-server")


def world_json(path):
    """{"world": ..., "locations": [[name, x, z]], "hashes": [[hash_u32, x, z]]} for a world folder."""
    if not os.path.isfile(BIN):
        sys.exit("tracker-server not found at %s; build it: cd client && cargo build --release -p tracker-server" % BIN)
    r = subprocess.run([BIN, "world", path], capture_output=True, text=True)
    if r.returncode:
        sys.exit("tracker-server world failed: " + (r.stderr.strip() or r.stdout.strip()))
    return json.loads(r.stdout)


def stable_hash(s):
    """The game's string.GetStableHashCode (uint32)."""
    h1 = h2 = 5381
    for i in range(0, len(s), 2):
        h1 = ((h1 << 5) + h1 ^ ord(s[i])) & 0xFFFFFFFF
        if i + 1 < len(s):
            h2 = ((h2 << 5) + h2 ^ ord(s[i + 1])) & 0xFFFFFFFF
    return (h1 + h2 * 1566083941) & 0xFFFFFFFF
