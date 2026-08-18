#!/usr/bin/env python3
"""Draws the Shelf app icon: a shelf bar with two cards standing on it.

Pure geometry, analytic anti-aliasing via signed distance fields, no
dependencies. Writes a 1024x1024 RGBA PNG.
"""

import math
import struct
import sys
import zlib

SIZE = 1024

SLATE = (0x1C, 0x22, 0x2E)
BLUE = (0x3B, 0x82, 0xF6)
BLUE_LIGHT = (0x7D, 0xAD, 0xFF)


def rounded_rect_sdf(px, py, x0, y0, x1, y1, r):
    cx, cy = (x0 + x1) / 2.0, (y0 + y1) / 2.0
    hx, hy = (x1 - x0) / 2.0 - r, (y1 - y0) / 2.0 - r
    dx = abs(px - cx) - hx
    dy = abs(py - cy) - hy
    ox, oy = max(dx, 0.0), max(dy, 0.0)
    return math.hypot(ox, oy) + min(max(dx, dy), 0.0) - r


# geometry, in a 1024 box
BAR = (128, 668, 896, 740, 36)
CARD_LEFT = (232, 292, 512, 668, 40)
CARD_RIGHT = (552, 412, 792, 668, 40)

SHAPES = [
    (CARD_LEFT, BLUE),
    (CARD_RIGHT, BLUE_LIGHT),
    (BAR, SLATE),
]


def render():
    rows = []
    for y in range(SIZE):
        py = y + 0.5
        row = bytearray()
        for x in range(SIZE):
            px = x + 0.5
            r = g = b = 0.0
            a = 0.0
            for (x0, y0, x1, y1, rad), colour in SHAPES:
                d = rounded_rect_sdf(px, py, x0, y0, x1, y1, rad)
                cov = min(max(0.5 - d, 0.0), 1.0)
                if cov <= 0.0:
                    continue
                # source-over, straight alpha
                sr, sg, sb = colour
                na = cov + a * (1.0 - cov)
                if na <= 0.0:
                    continue
                r = (sr * cov + r * a * (1.0 - cov)) / na
                g = (sg * cov + g * a * (1.0 - cov)) / na
                b = (sb * cov + b * a * (1.0 - cov)) / na
                a = na
            row += bytes(
                (int(round(r)), int(round(g)), int(round(b)), int(round(a * 255)))
            )
        rows.append(bytes(row))
    return rows


def write_png(path, rows):
    raw = b"".join(b"\x00" + row for row in rows)

    def chunk(tag, data):
        payload = tag + data
        return (
            struct.pack(">I", len(data))
            + payload
            + struct.pack(">I", zlib.crc32(payload) & 0xFFFFFFFF)
        )

    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", SIZE, SIZE, 8, 6, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(raw, 9))
    png += chunk(b"IEND", b"")
    with open(path, "wb") as fh:
        fh.write(png)


if __name__ == "__main__":
    write_png(sys.argv[1], render())
    print("wrote", sys.argv[1])
