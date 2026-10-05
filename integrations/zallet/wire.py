"""Wire a pinned, patched librustzcash cohort into a source-only Zallet checkout."""
import json
import re
import subprocess
import sys
from pathlib import Path

PINS = ("987382f67e622915228686e9f956c6a9c9a7514c", "1f6bb2072e7fcb142b0d90ff7b267a8699a84818")
CRATES = {"equihash": "components/equihash", "f4jumble": "components/f4jumble",
          "zcash_address": "components/zcash_address", "zcash_protocol": "components/zcash_protocol",
          "zip321": "components/zip321"}
for name in ("pczt", "zcash_client_backend", "zcash_client_sqlite", "zcash_keys",
             "zcash_pool_migration", "zcash_primitives", "zcash_proofs", "zcash_transparent"):
    CRATES[name] = name

def wire(zallet, library):
    zallet, library = Path(zallet).resolve(), Path(library).resolve()
    for root, pin in zip((zallet, library), PINS):
        actual = subprocess.check_output(["git", "-C", str(root), "rev-parse", "HEAD"], text=True).strip()
        if actual != pin:
            raise ValueError(f"Source revision mismatch: {root}")
    manifest = zallet / "Cargo.toml"
    text = manifest.read_text()
    start = text.index("[patch.crates-io]")
    end = text.find("\n[", start + 1)
    end = len(text) if end < 0 else end
    section = text[start:end]
    for name, relative in CRATES.items():
        path = library / relative
        if not (path / "Cargo.toml").is_file():
            raise ValueError(f"Missing crate: {name}")
        section, count = re.subn(r"^" + re.escape(name) + r"\s*=.*$",
                                lambda _: name + " = { path = " + json.dumps(str(path)) + " }",
                                section, flags=re.M)
        if count != 1:
            raise ValueError(f"Expected exactly one override: {name}")
    manifest.write_text(text[:start] + section + text[end:])

if __name__ == "__main__":
    if len(sys.argv) != 3:
        raise SystemExit("Usage: wire.py ZALLET_SOURCE LIBRUSTZCASH_SOURCE")
    wire(*sys.argv[1:])
