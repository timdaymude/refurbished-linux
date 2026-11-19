#!/usr/bin/env python3
"""Drive the QEMU test guest through QMP: absolute pointer, keys, screenshots.

Used only for verification of the Refurbished Linux image; not part of it.
"""
import json
import os
import socket
import sys
import time

QMP = os.environ.get("QMP", "/tmp/q.sock")

# Linux evdev key codes, the same numbers the kernel and libinput use.
KEYS = {
    "esc": 1, "1": 2, "2": 3, "3": 4, "4": 5, "5": 6, "6": 7, "7": 8,
    "8": 9, "9": 10, "0": 11, "minus": 12, "equal": 13, "backspace": 14,
    "tab": 15, "q": 16, "w": 17, "e": 18, "r": 19, "t": 20, "y": 21,
    "u": 22, "i": 23, "o": 24, "p": 25, "leftbrace": 26,
    "rightbrace": 27, "ret": 28, "ctrl": 29, "a": 30, "s": 31, "d": 32,
    "f": 33, "g": 34, "h": 35, "j": 36, "k": 37, "l": 38, "semicolon": 39,
    "apostrophe": 40, "grave": 41, "shift": 42, "backslash": 43, "z": 44,
    "x": 45, "c": 46, "v": 47, "b": 48, "n": 49, "m": 50, "comma": 51,
    "dot": 52, "slash": 53, "alt": 56, "spc": 57, "meta": 125,
    "up": 103, "down": 108, "left": 105, "right": 106, "delete": 111,
    "F1": 59, "F2": 60, "F3": 61, "F4": 62, "F5": 63, "F6": 64,
    "F7": 65, "F8": 66, "F9": 67, "F10": 68, "F11": 87, "F12": 88,
    "print": 99,
    "volup": 115, "voldown": 114, "mute": 113,
    "brightup": 225, "brightdown": 224,
}

SHIFTED = {
    "!": "1", "@": "2", "#": "3", "$": "4", "%": "5", "^": "6", "&": "7",
    "*": "8", "(": "9", ")": "0", "_": "minus", "+": "equal", "{": "leftbrace",
    "}": "rightbrace", ":": "semicolon", '"': "apostrophe", "~": "grave",
    "|": "backslash", "<": "comma", ">": "dot", "?": "slash",
    " ": "spc", "-": "minus", "=": "equal", "[": "leftbrace",
    "]": "rightbrace", ";": "semicolon", "'": "apostrophe", "`": "grave",
    "\\": "backslash", ",": "comma", ".": "dot", "/": "slash",
}


class Qmp:
    def __init__(self):
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.sock.connect(QMP)
        self.io = self.sock.makefile("rwb")
        self._read()  # greeting
        self.cmd("qmp_capabilities")

    def _read(self):
        while True:
            line = self.io.readline()
            if not line:
                raise SystemExit("qmp: connection closed")
            message = json.loads(line)
            if "event" in message:
                continue
            return message

    def cmd(self, name, **arguments):
        request = {"execute": name}
        if arguments:
            request["arguments"] = arguments
        self.io.write((json.dumps(request) + "\n").encode())
        self.io.flush()
        reply = self._read()
        if "error" in reply:
            raise SystemExit(f"qmp error: {reply['error']}")
        return reply.get("return")


def send_events(qmp, events):
    qmp.cmd("input-send-event", events=events)


def key(qmp, name, hold=0.02):
    code = KEYS[name]
    send_events(qmp, [{"type": "key", "data": {"down": True, "key": {"type": "number", "data": code}}}])
    time.sleep(hold)
    send_events(qmp, [{"type": "key", "data": {"down": False, "key": {"type": "number", "data": code}}}])
    time.sleep(0.02)


def combo(qmp, *names):
    for name in names:
        code = KEYS[name]
        send_events(qmp, [{"type": "key", "data": {"down": True, "key": {"type": "number", "data": code}}}])
        time.sleep(0.02)
    for name in reversed(names):
        code = KEYS[name]
        send_events(qmp, [{"type": "key", "data": {"down": False, "key": {"type": "number", "data": code}}}])
        time.sleep(0.02)


def type_text(qmp, text):
    for ch in text:
        if ch.isupper():
            # Shift is pressed around the character, as a real keyboard does.
            shift = KEYS["shift"]
            send_events(qmp, [{"type": "key", "data": {"down": True, "key": {"type": "number", "data": shift}}}])
            key(qmp, ch.lower())
            send_events(qmp, [{"type": "key", "data": {"down": False, "key": {"type": "number", "data": shift}}}])
        elif ch in SHIFTED:
            name = SHIFTED[ch]
            if ch in "!@#$%^&*()_+{}:<>?~|\"":
                shift = KEYS["shift"]
                send_events(qmp, [{"type": "key", "data": {"down": True, "key": {"type": "number", "data": shift}}}])
                key(qmp, name)
                send_events(qmp, [{"type": "key", "data": {"down": False, "key": {"type": "number", "data": shift}}}])
            else:
                key(qmp, name)
        elif ch in KEYS:
            key(qmp, ch)
        else:
            raise SystemExit(f"no key for {ch!r}")
        # A human types well under 10 characters per second. Going faster only
        # made the emulated PS/2 controller drop or reorder events, which looked
        # like the guest mangling input that it had never received.
        time.sleep(float(os.environ.get("TYPE_DELAY", "0.12")))


def screen_size(qmp):
    path = "/tmp/refurb-size.ppm"
    qmp.cmd("screendump", filename=path)
    with open(path, "rb") as handle:
        magic = handle.readline().strip()
        if magic != b"P6":
            raise SystemExit("unexpected screendump format")
        width, height = handle.readline().split()
        return int(width), int(height)


_SCREEN = None


def move(qmp, x, y):
    # Cache the size: a screendump per motion event stalls QEMU's main loop and
    # makes an injected drag look like it was processed seconds late.
    global _SCREEN
    if _SCREEN is None:
        _SCREEN = screen_size(qmp)
    width, height = _SCREEN
    ax = max(0, min(32767, int(x * 32767 / max(1, width - 1))))
    ay = max(0, min(32767, int(y * 32767 / max(1, height - 1))))
    send_events(qmp, [
        {"type": "abs", "data": {"axis": "x", "value": ax}},
        {"type": "abs", "data": {"axis": "y", "value": ay}},
    ])
    time.sleep(0.04)


def button(qmp, down):
    send_events(qmp, [{"type": "btn", "data": {"down": down, "button": "left"}}])
    time.sleep(0.05)


def click(qmp, x, y):
    move(qmp, x, y)
    button(qmp, True)
    button(qmp, False)
    time.sleep(0.25)


def drag(qmp, x1, y1, x2, y2, steps=24):
    move(qmp, x1, y1)
    button(qmp, True)
    for i in range(1, steps + 1):
        move(qmp, x1 + (x2 - x1) * i / steps, y1 + (y2 - y1) * i / steps)
    button(qmp, False)
    time.sleep(0.2)


def main():
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    qmp = Qmp()
    command = sys.argv[1]
    args = sys.argv[2:]
    if command == "type":
        type_text(qmp, args[0])
    elif command == "key":
        key(qmp, args[0])
    elif command == "combo":
        combo(qmp, *args)
    elif command == "move":
        move(qmp, float(args[0]), float(args[1]))
    elif command == "click":
        click(qmp, float(args[0]), float(args[1]))
    elif command == "drag":
        drag(qmp, float(args[0]), float(args[1]), float(args[2]), float(args[3]))
    elif command == "shot":
        qmp.cmd("screendump", filename=args[0])
        print(args[0])
    elif command == "size":
        print(*screen_size(qmp))
    elif command == "raw":
        print(qmp.cmd(args[0]))
    else:
        raise SystemExit(f"unknown command {command}")


if __name__ == "__main__":
    main()
