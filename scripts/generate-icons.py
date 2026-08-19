#!/usr/bin/env python3
"""Generate simple PNG/ICNS/ICO assets without third-party deps."""

from __future__ import annotations

import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / "src-tauri" / "icons"


def png(width: int, height: int, rgba_fn) -> bytes:
    raw = bytearray()
    for y in range(height):
        raw.append(0)
        for x in range(width):
            raw.extend(rgba_fn(x, y, width, height))

    def chunk(tag: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    return b"".join(
        [
            b"\x89PNG\r\n\x1a\n",
            chunk(b"IHDR", ihdr),
            chunk(b"IDAT", zlib.compress(bytes(raw), 9)),
            chunk(b"IEND", b""),
        ]
    )


def pixel(x: int, y: int, w: int, h: int) -> bytes:
    # teal key-switch mark on dark slate
    nx = x / (w - 1)
    ny = y / (h - 1)
    r, g, b = 18, 24, 30
    # rounded-ish panel
    inset = 0.12
    if inset < nx < 1 - inset and inset < ny < 1 - inset:
        r, g, b = 28, 42, 48
    # horizontal bar
    if 0.32 < ny < 0.46 and 0.22 < nx < 0.78:
        r, g, b = 15, 118, 110
    # key head
    cx, cy, rad = 0.34, 0.39, 0.13
    if (nx - cx) ** 2 + (ny - cy) ** 2 < rad**2:
        r, g, b = 13, 148, 136
    # stem
    if 0.46 < ny < 0.74 and 0.46 < nx < 0.58:
        r, g, b = 13, 148, 136
    # teeth
    if 0.58 < ny < 0.70 and 0.58 < nx < 0.70:
        r, g, b = 45, 212, 191
    return bytes((r, g, b, 255))


def write_ico(path: Path, png_bytes: bytes, size: int) -> None:
    # PNG-in-ICO
    header = struct.pack("<HHH", 0, 1, 1)
    entry = struct.pack(
        "<BBBBHHII",
        size if size < 256 else 0,
        size if size < 256 else 0,
        0,
        0,
        1,
        32,
        len(png_bytes),
        22,
    )
    path.write_bytes(header + entry + png_bytes)


def main() -> None:
    ROOT.mkdir(parents=True, exist_ok=True)
    sizes = {
        "32x32.png": 32,
        "128x128.png": 128,
        "henry.w@example.net": 256,
        "128x128@2x.png": 256,
        "icon.png": 512,
    }
    pngs = {}
    for name, size in sizes.items():
        data = png(size, size, pixel)
        (ROOT / name).write_bytes(data)
        pngs[size] = data
    write_ico(ROOT / "icon.ico", pngs[256], 256)
    # icon.icns is generated later on macOS if iconutil is available;
    # keep a PNG fallback copy named icon.icns only if conversion fails.
    print(f"wrote icons under {ROOT}")


if __name__ == "__main__":
    main()
