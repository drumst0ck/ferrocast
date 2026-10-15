#!/usr/bin/env python3
"""Generate resources/Ferrocast.icns (pure Python, no dependencies besides macOS iconutil/sips)."""
import os, struct, subprocess, sys, tempfile, zlib

N = 1024
ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "resources")
BOLT = [(0.58, 0.16), (0.30, 0.54), (0.47, 0.54), (0.41, 0.84), (0.70, 0.44), (0.53, 0.44)]  # unit square


def inside(poly, x, y):
    c, j = False, len(poly) - 1
    for i in range(len(poly)):
        xi, yi = poly[i]; xj, yj = poly[j]
        if (yi > y) != (yj > y) and x < (xj - xi) * (y - yi) / (yj - yi) + xi:
            c = not c
        j = i
    return c


def rr_cov(px, py, lo, hi, r):
    """Anti-aliased coverage of a rounded square spanning [lo, hi]."""
    cx = min(max(px, lo + r), hi - r); cy = min(max(py, lo + r), hi - r)
    d = ((px - cx) ** 2 + (py - cy) ** 2) ** 0.5 - r
    return max(0.0, min(1.0, 0.5 - d))


def render():
    lo, hi, r = N * 0.098, N * 0.902, N * 0.18
    top, bot = (255, 112, 112), (206, 40, 62)
    poly = [(x * N, y * N) for x, y in BOLT]
    bx0, bx1 = min(p[0] for p in poly) - 2, max(p[0] for p in poly) + 2
    by0, by1 = min(p[1] for p in poly) - 2, max(p[1] for p in poly) + 2
    rows = []
    for y in range(N):
        row = bytearray([0])
        t = min(max((y - lo) / (hi - lo), 0.0), 1.0)
        base = [top[i] + (bot[i] - top[i]) * t for i in range(3)]
        for x in range(N):
            a = rr_cov(x + 0.5, y + 0.5, lo, hi, r)
            if a <= 0.0:
                row += b"\x00\x00\x00\x00"; continue
            col = base
            if bx0 <= x <= bx1 and by0 <= y <= by1:
                cov = sum(inside(poly, x + sx, y + sy) for sx in (0.25, 0.75) for sy in (0.25, 0.75)) / 4.0
                if cov:
                    col = [col[i] + (255 - col[i]) * cov for i in range(3)]
            row += bytes((int(col[0]), int(col[1]), int(col[2]), int(a * 255)))
        rows.append(bytes(row))
    return b"".join(rows)


def png(raw):
    def chunk(tag, data):
        c = struct.pack(">I", len(data)) + tag + data
        return c + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", N, N, 8, 6, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")


def main():
    os.makedirs(ROOT, exist_ok=True)
    master = os.path.join(ROOT, "icon-1024.png")
    open(master, "wb").write(png(render()))
    with tempfile.TemporaryDirectory() as d:
        iconset = os.path.join(d, "Ferrocast.iconset"); os.mkdir(iconset)
        for size in (16, 32, 128, 256, 512):
            for scale in (1, 2):
                px = size * scale
                out = os.path.join(iconset, f"icon_{size}x{size}{'@2x' if scale == 2 else ''}.png")
                subprocess.run(["sips", "-z", str(px), str(px), master, "--out", out], check=True, capture_output=True)
        subprocess.run(["iconutil", "-c", "icns", iconset, "-o", os.path.join(ROOT, "Ferrocast.icns")], check=True)
    print("wrote", os.path.join(ROOT, "Ferrocast.icns"))


if __name__ == "__main__":
    sys.exit(main())
