#!/usr/bin/env python3
"""Log in on the test VM's serial console and run commands (test-only)."""
import os, re, socket, sys, time
SOCK = os.environ.get("SERIAL", os.path.join(os.path.dirname(os.path.abspath(__file__)), "ser8.sock"))
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[()][A-Z0-9]|\r")
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
s.connect(SOCK); s.settimeout(0.4)
def drain(sec):
    end = time.time() + sec; out = b""
    while time.time() < end:
        try:
            c = s.recv(65536)
            if not c: break
            out += c
        except socket.timeout:
            pass
    return ANSI.sub("", out.decode("utf-8", "replace"))
print(drain(1.5), end="")
if "login:" in "":
    pass
s.sendall(b"refurb\n")
boot = drain(2.5)
print(boot, end="")
if "assword" in boot:
    s.sendall(b"\n")
    print(drain(2.0), end="")
for cmd in sys.argv[1:]:
    s.sendall(b"echo ===MARK===\n"); time.sleep(0.4)
    s.sendall(cmd.encode() + b"\n"); time.sleep(1.0)
    print(drain(6.0), end="")
s.close()
