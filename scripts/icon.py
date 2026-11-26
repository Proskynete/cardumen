#!/usr/bin/env python3
"""Write the Cardumen iconset (PNG, stdlib only) from the Tiburoncín grid in src/welcome.rs.

usage: icon.py <repo root> <output .iconset dir>
"""
import re
import struct
import sys
import zlib
from pathlib import Path

COLORS = {"k": (0x1A, 0x3A, 0x57), "b": (0x3E, 0x7C, 0xB1), "w": (0xED, 0xF4, 0xF3), "r": (0xF4, 0x73, 0x6B)}
BACKGROUND = (0x09, 0x13, 0x19)


def grid(root: Path) -> list[str]:
    src = (root / "src/welcome.rs").read_text()
    block = src[src.index("const PIXELS") : src.index("];", src.index("const PIXELS"))]
    rows = re.findall(r'"([.kbwr]{22})"', block)
    assert len(rows) == 22, f"expected 22 rows, got {len(rows)}"
    return rows


def render(rows: list[str], size: int) -> bytes:
    # macOS icon grid: the rounded square fills ~80% of the canvas.
    inset = round(size * 0.1)
    side = size - 2 * inset
    radius = side * 0.225
    art = side * 0.72
    cell = art / 22
    ox = oy = inset + (side - art) / 2
    out = bytearray()
    for y in range(size):
        out.append(0)  # PNG filter: none
        for x in range(size):
            px = (0, 0, 0, 0)
            # Rounded-square mask.
            lx, ly = x + 0.5 - inset, y + 0.5 - inset
            if 0 <= lx < side and 0 <= ly < side:
                cx = min(max(lx, radius), side - radius)
                cy = min(max(ly, radius), side - radius)
                if (lx - cx) ** 2 + (ly - cy) ** 2 <= radius ** 2:
                    px = (*BACKGROUND, 255)
                    gx, gy = int((x + 0.5 - ox) // cell), int((y + 0.5 - oy) // cell)
                    if 0 <= gx < 22 and 0 <= gy < 22 and rows[gy][gx] in COLORS:
                        px = (*COLORS[rows[gy][gx]], 255)
            out.extend(px)
    return bytes(out)


def png(size: int, rgba: bytes) -> bytes:
    def chunk(kind: bytes, data: bytes) -> bytes:
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)

    header = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(rgba, 9)) + chunk(b"IEND", b"")


def main() -> None:
    root, out = Path(sys.argv[1]), Path(sys.argv[2])
    out.mkdir(parents=True, exist_ok=True)
    rows = grid(root)
    for base in (16, 32, 128, 256, 512):
        for scale in (1, 2):
            size = base * scale
            name = f"icon_{base}x{base}{'@2x' if scale == 2 else ''}.png"
            (out / name).write_bytes(png(size, render(rows, size)))


if __name__ == "__main__":
    main()
