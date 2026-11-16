#!/usr/bin/env python3
"""Drive the guest's serial console: log in and run a command."""
import socket, sys, time

sock = ".build/ser.sock"
cmds = sys.argv[1:]

s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
s.connect(sock)
s.settimeout(1)

def drain(seconds):
    end = time.time() + seconds
    buf = b""
    while time.time() < end:
        try:
            d = s.recv(65536)
            if not d:
                break
            buf += d
        except socket.timeout:
            continue
    return buf.decode("utf-8", "replace")

s.sendall(b"\n")
out = drain(3)
if "login:" in out:
    s.sendall(b"user\n")
    out += drain(4)
    if "assword" in out:
        s.sendall(b"\n")
        out += drain(3)
    out += drain(2)
    if "root@" in out or "#" in out[-300:]:
        for c in cmds:
            s.sendall((c + "\n").encode())
            out += "\n$ " + c + "\n" + drain(6)
print(out)
