#!/usr/bin/env python3
"""Regenerate Tauri icon derivatives from the checked-in canonical source."""

from __future__ import annotations

import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
ROOT = REPO / "src-tauri" / "icons"
SOURCE = ROOT / "source.png"
TAURI = REPO / "node_modules" / ".bin" / ("tauri.cmd" if sys.platform == "win32" else "tauri")
CANONICAL_ICNS_SHA256 = "66461832594db0b4d224d6186f8e72fe1ce0996c54bbec5ca372f5966d131658"
DERIVATIVES = (
    "32x32.png",
    "128x128.png",
    "128x128@2x.png",
    "icon.png",
    "icon.ico",
)


def main() -> None:
    if not SOURCE.exists():
        raise SystemExit(f"missing canonical icon: {SOURCE}")
    if not TAURI.exists():
        raise SystemExit("missing local Tauri CLI; run npm install before generating icons")
    with tempfile.TemporaryDirectory(prefix="keysmith-icons-") as directory:
        generated = Path(directory)
        subprocess.run(
            [str(TAURI), "icon", str(SOURCE), "-o", str(generated)],
            check=True,
        )
        for name in DERIVATIVES:
            (ROOT / name).write_bytes((generated / name).read_bytes())
    print(f"wrote {len(DERIVATIVES)} icon derivatives under {ROOT} from {SOURCE}")
    print(
        "icon.icns is preserved as the canonical byte-for-byte asset "
        f"({CANONICAL_ICNS_SHA256})"
    )


if __name__ == "__main__":
    main()
