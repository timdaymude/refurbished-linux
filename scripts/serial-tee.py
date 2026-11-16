#!/usr/bin/env python3
"""Record everything the guest writes to its serial console."""
import socket, sys, time
path = sys.argv[1] if len(sys.argv) > 1 else ".build/serial-full.log"
for _ in range(60):
    try:
        s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        s.connect(".build/ser.sock")
        break
    except OSError:
        time.sleep(1)
else:
    sys.exit("no serial socket")
s.settimeout(5)
with open(path, "ab", buffering=0) as f:
    end = time.time() + int(sys.argv[2] if len(sys.argv) > 2 else 400)
    while time.time() < end:
        try:
            data = s.recv(65536)
        except socket.timeout:
            continue
        if not data:
            break
        f.write(data)
