#!/usr/bin/env python3
import os, re, socket, sys, time
SOCK = os.path.join(os.path.dirname(os.path.abspath(__file__)), "ser8.sock")
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[()][A-Z0-9]|\r")
seconds = float(sys.argv[1]) if len(sys.argv) > 1 else 5.0
command = " ".join(sys.argv[2:]) if len(sys.argv) > 2 else ""
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
s.connect(SOCK); s.settimeout(0.5)
if command:
    time.sleep(0.6); s.sendall((command + "\n").encode())
end = time.time() + seconds; out = bytearray()
while time.time() < end:
    try:
        chunk = s.recv(65536)
        if not chunk: break
        out += chunk
    except socket.timeout:
        continue
s.close()
sys.stdout.write(ANSI.sub("", out.decode("utf-8", "replace")))
