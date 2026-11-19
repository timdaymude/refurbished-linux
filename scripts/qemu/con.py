#!/usr/bin/env python3
"""Talk to the guest's serial console: log in and run commands.

    con.py --login "labwc --version" "ls -l /etc/xdg/labwc"
    con.py "wpctl get-volume @DEFAULT_AUDIO_SINK@"
"""
import os
import socket
import sys
import time

SOCK = os.environ.get("SERIAL", "/tmp/c.sock")


def main():
    args = sys.argv[1:]
    login = False
    if args and args[0] == "--login":
        login = True
        args = args[1:]
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.connect(SOCK)
    sock.settimeout(0.4)

    def drain(seconds):
        end = time.time() + seconds
        out = b""
        while time.time() < end:
            try:
                chunk = sock.recv(65536)
                if not chunk:
                    break
                out += chunk
            except socket.timeout:
                pass
        return out.decode("utf-8", "replace")

    print(drain(2.0), end="")
    if login:
        sock.sendall(b"\n")
        print(drain(1.5), end="")
        sock.sendall(b"refurb\n")
        print(drain(2.0), end="")
        sock.sendall(b"\n")
        print(drain(3.0), end="")
    for command in args:
        sock.sendall(b"echo ===MARK===\n")
        time.sleep(0.3)
        sock.sendall(command.encode() + b"\n")
        time.sleep(1.2)
        print(drain(6.0), end="")
    sock.close()


if __name__ == "__main__":
    main()
