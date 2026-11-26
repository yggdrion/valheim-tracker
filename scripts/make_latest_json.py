"""Builds latest.json for the Tauri updater from the signed bundles the app jobs produced.

usage: make_latest_json.py VERSION ARTIFACTS_DIR OUT_DIR DOWNLOAD_BASE_URL
Copies each updater bundle + .sig into OUT_DIR under a space-free name and writes OUT_DIR/latest.json.
"""
import json, shutil, sys
from datetime import datetime, timezone
from pathlib import Path

version, art, out, base = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3]), sys.argv[4].rstrip("/")

# (suffix of the bundle, name in the release, updater platforms)
KINDS = [
    ("-setup.exe", "update-windows-x64-setup.exe", ["windows-x86_64"]),
    (".app.tar.gz", "update-macos-universal.app.tar.gz", ["darwin-aarch64", "darwin-x86_64"]),
    (".AppImage", "update-linux-x64.AppImage", ["linux-x86_64"]),
]
platforms = {}
for suffix, name, plats in KINDS:
    bundle = next((p for p in art.rglob("*" + suffix) if p.is_file()), None)
    sig = Path(str(bundle) + ".sig") if bundle else None
    if not bundle or not sig.is_file():
        sys.exit(f"missing signed updater bundle for {suffix} (is TAURI_SIGNING_PRIVATE_KEY set?)")
    shutil.copy(bundle, out / name)
    shutil.copy(sig, out / (name + ".sig"))
    for p in plats:
        platforms[p] = {"signature": sig.read_text().strip(), "url": f"{base}/{name}"}

(out / "latest.json").write_text(json.dumps({
    "version": version,
    "pub_date": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "platforms": platforms,
}, indent=2))
