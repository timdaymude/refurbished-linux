#!/usr/bin/env python3
"""Take a screenshot of the running QEMU VM through its monitor socket.

    ./scripts/shot.py .build/shot-1.png
"""
import socket, sys, time, zlib, struct
from pathlib import Path

def shoot(out):
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.connect(".build/mon.sock")
    s.settimeout(20)
    time.sleep(0.3)
    s.recv(65536)
    ppm = ".build/shot.ppm"
    s.sendall(f"screendump {ppm}\n".encode())
    time.sleep(2.0)
    try:
        s.recv(65536)
    except Exception:
        pass
    s.sendall(b"info status\n")
    time.sleep(0.3)
    try:
        status = s.recv(65536).decode("utf-8", "replace")
    except Exception:
        status = ""
    s.close()

    data = Path(ppm).read_bytes()
    # P6 <w> <h> <maxval>\n<rgb bytes>
    parts, idx = [], 0
    while len(parts) < 4:
        while data[idx:idx + 1].isspace():
            idx += 1
        if data[idx:idx + 1] == b"#":
            while data[idx:idx + 1] != b"\n":
                idx += 1
            continue
        start = idx
        while not data[idx:idx + 1].isspace():
            idx += 1
        parts.append(data[start:idx])
    idx += 1
    w, h = int(parts[1]), int(parts[2])
    px = data[idx:idx + w * h * 3]
    raw = b"".join(b"\x00" + px[y * w * 3:(y + 1) * w * 3] for y in range(h))

    def chunk(tag, payload):
        return (struct.pack(">I", len(payload)) + tag + payload
                + struct.pack(">I", zlib.crc32(tag + payload) & 0xFFFFFFFF))

    png = (b"\x89PNG\r\n\x1a\n"
           + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
           + chunk(b"IDAT", zlib.compress(raw, 6))
           + chunk(b"IEND", b""))
    Path(out).write_bytes(png)
    print(f"{out}: {w}x{h}", [l for l in status.splitlines() if "status" in l.lower()][:1])

if __name__ == "__main__":
    shoot(sys.argv[1] if len(sys.argv) > 1 else ".build/shot.png")
