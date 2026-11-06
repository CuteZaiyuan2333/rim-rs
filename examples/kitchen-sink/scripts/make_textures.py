#!/usr/bin/env python3
"""Generate placeholder textures for the kitchen-sink test example.

Every texture is a small procedural image: a magenta/black checkerboard
(the classic "missing texture" look) with a per-def color tint in one
corner. No real art assets are needed; rim-rs's `assets` module just
copies whatever PNGs it finds.

Run from the example directory:

    python3 scripts/make_textures.py
"""

import pathlib
import struct
import sys
import zlib

ROOT = pathlib.Path(__file__).resolve().parent.parent
TEXTURES = ROOT / "assets" / "textures"
PREVIEW = ROOT / "assets" / "preview.png"

MAGENTA = (255, 0, 220)


def png(path: pathlib.Path, width: int, height: int, pixels) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    raw = bytearray()
    for y in range(height):
        raw.append(0)  # filter type "none" per scanline
        for x in range(width):
            r, g, b = pixels(x, y)
            raw.extend((r, g, b))
    chunk = lambda tag, data: (  # noqa: E731
        struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
    )
    ihdr = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    body = chunk(b"IHDR", ihdr) + chunk(b"IDAT", zlib.compress(bytes(raw), 9)) + chunk(b"IEND", b"")
    path.write_bytes(b"\x89PNG\r\n\x1a\n" + body)


def checker(size: int, tint=(90, 90, 110), square: int = 16):
    """Magenta/black checkerboard plus one tinted corner square (rim-rs marker)."""

    def pixels(x, y):
        on = ((x // square) + (y // square)) % 2 == 0
        if on:
            base = MAGENTA
        else:
            base = (20, 20, 24)
        if x < square and y < square:
            base = tint
        return base

    return pixels


ENTRIES = [
    # (relative path under Textures/, size, tint) - kept in sync with main.rs
    ("Things/Item/RimRsWidget.png", 64, (60, 180, 90)),
    ("Things/Item/Weapons/RimRsShiv.png", 64, (220, 150, 40)),
    ("Things/Item/RimRsOrganItem.png", 64, (120, 60, 200)),
    ("Things/Pawn/Humanlike/RimRsCap/RimRsCap.png", 64, (70, 140, 220)),
    ("Things/Building/RimRsWall.png", 96, (150, 150, 160)),
    ("Things/Building/RimRsWorkbench.png", 256, (190, 120, 60)),
    ("Things/Floor/RimRsFloor.png", 128, (110, 160, 170)),
    ("Pawn/Animal/RimRsCreature/RimRsCreature.png", 128, (210, 60, 70)),
]


def main() -> int:
    for relative, size, tint in ENTRIES:
        png(TEXTURES / relative, size, size, checker(size, tint))
        print(f"wrote {relative}")
    # Square-ish preview; RimWorld accepts any size with black bars.
    png(PREVIEW, 64, 64, checker(64, (30, 30, 60), 16))
    print("wrote preview.png")
    return 0


if __name__ == "__main__":
    sys.exit(main())
