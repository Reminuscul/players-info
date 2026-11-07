"""Packs the bundle into players_info-<version>.hnmod: horizon-native.json and mixins/players_info/.

The archive is reproducible (sorted entries, fixed dates) and holds only what Horizon's
`horizon_mods::index::native::Bundle` accepts under `mixins/<id>/`: Rust and TOML files,
README and LICENSE.

usage: python3 tools/package.py [out-dir]
"""
import json
import re
import sys
import tomllib
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BUNDLE = ROOT / "bundle"
# The horizon-mixins commit the launcher of the pinned Nightly builds with
# (launcher/src/native_mods.rs, MIXINS_REV, at f319701).
MIXINS_REV = "e8cd0aa38cffef49cb2e088ef716baa107148c02"
GAME = "26.3"

manifest = tomllib.loads((BUNDLE / "horizon-mixin.toml").read_text())
mod_id = manifest["mod"]["id"]
version = manifest["mod"]["version"]
horizon = manifest["compatibility"]["horizon_revision"]
assert re.fullmatch(r"[a-z0-9_]{1,64}", mod_id), mod_id
assert re.fullmatch(r"[0-9a-f]{40}", horizon), horizon
cargo = tomllib.loads((BUNDLE / "api/Cargo.toml").read_text())
assert cargo["package"]["version"] == version, "api/Cargo.toml and horizon-mixin.toml disagree"


def allowed(rel: Path) -> bool:
    if rel.suffix in (".rs", ".toml"):
        return True
    top_level = len(rel.parts) <= 2
    return top_level and re.match(r"(README|LICENSE|LICENCE|COPYING|NOTICE)", rel.name) is not None


files = []
for path in sorted(BUNDLE.rglob("*")):
    rel = path.relative_to(BUNDLE)
    if path.is_dir() or any(p.startswith(".") or p == "target" for p in rel.parts):
        continue
    if not allowed(rel):
        sys.exit(f"{rel}: a native bundle may not hold this file")
    files.append((rel.as_posix(), path.read_bytes()))

native = {
    "schema": 1,
    "id": mod_id,
    "version": version,
    "game": GAME,
    "horizon": horizon,
    "mixins": MIXINS_REV,
}
out_dir = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "dist"
out_dir.mkdir(parents=True, exist_ok=True)
out = out_dir / f"{mod_id}-{version}.hnmod"
with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
    entries = [("horizon-native.json", (json.dumps(native, indent=2) + "\n").encode())]
    entries += [(f"mixins/{mod_id}/{name}", data) for name, data in files]
    for name, data in entries:
        info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
        info.compress_type = zipfile.ZIP_DEFLATED
        info.external_attr = 0o100644 << 16
        z.writestr(info, data)
print(out)
for name, data in entries:
    print(f"  {len(data):7} {name}")
