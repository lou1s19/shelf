#!/usr/bin/env python3
"""Draws Shelf's menu bar icons: one shelf, a different thing standing on it.

The shelf line is the constant, so every mode is recognisably the same app,
and what stands on it says which mode is active:

    studio      two cards side by side
    instant     a bolt
    screenshot  an open frame
    recording   a filled dot

macOS scales these to 18 points high and, because the tray icon is a template
image, paints them in the menu bar's own colour. So the shapes are drawn in
plain black with alpha, and they have to survive being that small: few parts,
fat strokes, real gaps.

Pure geometry with 4x4 supersampling, no dependencies. Writes 128x128 RGBA PNGs.

    python3 make-tray-icons.py <output-dir>
"""

import math
import os
import struct
import sys
import zlib

SIZE = 128
SAMPLES = 4  # per axis

# The shelf itself, shared by every icon.
BAR = (18.0, 98.0, 110.0, 107.0, 4.5)
BAR_TOP = 98.0


def rounded_rect(x0, y0, x1, y1, r):
    """Filled rounded rectangle."""

    def inside(px, py):
        cx, cy = (x0 + x1) / 2.0, (y0 + y1) / 2.0
        hx, hy = (x1 - x0) / 2.0 - r, (y1 - y0) / 2.0 - r
        dx = abs(px - cx) - hx
        dy = abs(py - cy) - hy
        outside = math.hypot(max(dx, 0.0), max(dy, 0.0))
        return outside + min(max(dx, dy), 0.0) - r <= 0.0

    return inside


def rounded_frame(x0, y0, x1, y1, r, stroke):
    """Rounded rectangle outline."""
    outer = rounded_rect(x0, y0, x1, y1, r)
    inner = rounded_rect(
        x0 + stroke, y0 + stroke, x1 - stroke, y1 - stroke, max(r - stroke, 1.0)
    )

    def inside(px, py):
        return outer(px, py) and not inner(px, py)

    return inside


def circle(cx, cy, r):
    def inside(px, py):
        return math.hypot(px - cx, py - cy) <= r

    return inside


def polygon(points):
    """Even-odd fill, good enough for a convex-ish bolt."""

    def inside(px, py):
        hit = False
        count = len(points)
        for i in range(count):
            x0, y0 = points[i]
            x1, y1 = points[(i + 1) % count]
            if (y0 > py) != (y1 > py):
                cross = x0 + (py - y0) * (x1 - x0) / (y1 - y0)
                if px < cross:
                    hit = not hit
        return hit

    return inside


def bolt(x0, y0, x1, y1):
    """A lightning bolt inside the given box."""
    w, h = x1 - x0, y1 - y0
    unit = [
        (0.58, 0.00),
        (0.04, 0.58),
        (0.42, 0.58),
        (0.34, 1.00),
        (0.96, 0.38),
        (0.54, 0.38),
    ]
    return polygon([(x0 + ux * w, y0 + uy * h) for ux, uy in unit])


def render(shapes):
    step = 1.0 / SAMPLES
    offset = step / 2.0
    rows = []
    for y in range(SIZE):
        row = bytearray()
        for x in range(SIZE):
            hits = 0
            for sy in range(SAMPLES):
                py = y + offset + sy * step
                for sx in range(SAMPLES):
                    px = x + offset + sx * step
                    if any(shape(px, py) for shape in shapes):
                        hits += 1
            alpha = int(round(255.0 * hits / (SAMPLES * SAMPLES)))
            row += bytes((0, 0, 0, alpha))
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


def shelf():
    return rounded_rect(*BAR)


ICONS = {
    # Two cards, the taller one first, the same mark the app icon uses.
    "tray-default-icon-studio.png": lambda: [
        shelf(),
        rounded_rect(36.0, 38.0, 58.0, BAR_TOP, 6.0),
        rounded_rect(70.0, 58.0, 92.0, BAR_TOP, 6.0),
    ],
    "tray-default-icon-instant.png": lambda: [
        shelf(),
        bolt(38.0, 26.0, 90.0, BAR_TOP),
    ],
    "tray-default-icon-screenshot.png": lambda: [
        shelf(),
        rounded_frame(39.0, 45.0, 89.0, BAR_TOP, 9.0, 10.0),
    ],
    # While recording the icon is the state, not the action: a plain dot.
    "tray-stop-icon.png": lambda: [
        shelf(),
        circle(64.0, 63.0, 25.0),
    ],
}


def main():
    out_dir = sys.argv[1] if len(sys.argv) > 1 else os.path.dirname(__file__)
    for name, build in ICONS.items():
        write_png(os.path.join(out_dir, name), render(build()))
        print("wrote", name)

    # The default icon is the studio mark; Windows uses it for every mode.
    default = render(ICONS["tray-default-icon-studio.png"]())
    write_png(os.path.join(out_dir, "tray-default-icon.png"), default)
    print("wrote tray-default-icon.png")


if __name__ == "__main__":
    main()
