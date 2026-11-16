#!/usr/bin/env python3
"""Convert a binary PPM (P6) to PNG. QEMU's screendump always writes PPM in
this build even when the filename ends in .png, and the workspace has no
image library, so encode the PNG directly."""
import struct, sys, zlib

src, dst = sys.argv[1], sys.argv[2]
data = open(src, "rb").read()
# Header: P6\n<width> <height>\n<maxval>\n
parts, idx = [], 2
while len(parts) < 3:
    while idx < len(data) and data[idx : idx + 1].isspace():
        idx += 1
    if data[idx : idx + 1] == b"#":
        while data[idx : idx + 1] not in (b"\n", b""):
            idx += 1
        continue
    start = idx
    while idx < len(data) and not data[idx : idx + 1].isspace():
        idx += 1
    parts.append(int(data[start:idx]))
idx += 1
w, h, _ = parts
pix = data[idx : idx + w * h * 3]
assert len(pix) == w * h * 3, f"truncated: {len(pix)} of {w*h*3}"

raw = bytearray()
for y in range(h):
    raw.append(0)  # filter: none
    raw += pix[y * w * 3 : (y + 1) * w * 3]

def chunk(tag, payload):
    return (struct.pack(">I", len(payload)) + tag + payload
            + struct.pack(">I", zlib.crc32(tag + payload) & 0xFFFFFFFF))

png = b"\x89PNG\r\n\x1a\n"
png += chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
png += chunk(b"IDAT", zlib.compress(bytes(raw), 6))
png += chunk(b"IEND", b"")
open(dst, "wb").write(png)
print(f"{dst}: {w}x{h} ({len(png)} bytes)")
