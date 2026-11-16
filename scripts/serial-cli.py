#!/usr/bin/env python3
"""Talk to a VM's serial console over a unix socket.

    serial-cli.py <socket> <wait-seconds> [line ...]

Sends each line, then prints everything the guest wrote. Reading and writing a
socket keeps the console bidirectional (a plain -serial file is write-only).
"""
import socket, sys, time

path, wait = sys.argv[1], float(sys.argv[2])
lines = [a for a in sys.argv[3:] if a != "--clean"]
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
s.settimeout(1.0)
s.connect(path)
buf = b""

def drain(seconds):
    global buf
    end = time.time() + seconds
    while time.time() < end:
        try:
            chunk = s.recv(65536)
            if not chunk:
                break
            buf += chunk
        except socket.timeout:
            continue
        except OSError:
            break

drain(1.0)
for line in lines:
    s.sendall((line + "\n").encode())
    drain(wait)
text = buf.decode(errors="replace")
# The console echoes ANSI cursor movement; strip the noisy control sequences.
import re
text = re.sub(r"\x1b\[[0-9;?]*[a-zA-Z]", "", text)
if "--clean" in sys.argv:
    # Drop kernel/journald lines so command output is readable on a busy console.
    keep = [ln for ln in text.splitlines()
            if not re.match(r"^\[\s*\d+\.\d+\]", ln) and ln.strip()]
    text = "\n".join(keep)
print(text[-12000:])
s.close()
