#!/usr/bin/env python3
"""Send commands to a QEMU monitor socket. Each argument is one command.

Used by the boot tests so the VM is driven over a socket rather than inheriting
the caller's stdio: a monitor that owns our pipes is what exhausts descriptors.
"""
import socket, sys, time

sock_path = sys.argv[1]
commands = sys.argv[2:]
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
s.settimeout(5)
s.connect(sock_path)
time.sleep(0.2)
try:
    s.recv(65536)
except socket.timeout:
    pass
for cmd in commands:
    s.sendall((cmd + "\n").encode())
    time.sleep(0.25)
    try:
        data = s.recv(65536).decode(errors="replace")
    except socket.timeout:
        data = ""
    print(f"{cmd}: {' '.join(data.split())[:160]}")
s.close()
