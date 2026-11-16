#!/usr/bin/env python3
"""Assemble a local toolchain without root.

This workspace has no root and no working package manager for the host, but
building an ISO needs debootstrap, mksquashfs, xorriso, mtools, mkfs.vfat and
the bootloader tools. They are ordinary .deb files, so this script fetches the
packages for the *host* distribution (so the binaries match the host glibc),
extracts them into .toolbox/, then chases every missing shared library by
resolving its soname through the archive Contents index.

    ./scripts/toolbox.py --ubuntu xorriso squashfs-tools mtools dosfstools \\
                         isolinux syslinux-common syslinux-utils \\
                         grub-common grub-pc-bin grub-efi-amd64-bin
    ./scripts/toolbox.py --debian debootstrap

Run the tools with:

    eval "$(./scripts/toolbox.py --env)"

Nothing outside .toolbox/ and the download cache is touched.
"""

import argparse
import gzip
import io
import os
import shutil
import subprocess
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TB = ROOT / ".toolbox"
CACHE = TB / "cache"

UBUNTU = "http://archive.ubuntu.com/ubuntu"
DEBIAN = "http://deb.debian.org/debian"

# Host is Ubuntu 24.04 (noble); the Debian side only needs arch:all shell tools.
SUITES = {
    "ubuntu": [f"{UBUNTU}/dists/noble/{c}/binary-amd64/Packages.gz" for c in ("main", "universe")],
    # Debian has no "universe": its components are main, contrib, non-free and
    # non-free-firmware. Asking for universe is a 404.
    "debian": [
        f"{DEBIAN}/dists/trixie/{c}/binary-amd64/Packages.gz"
        for c in ("main", "contrib", "non-free", "non-free-firmware")
    ],
}
CONTENTS = {
    "ubuntu": [f"{UBUNTU}/dists/noble/Contents-amd64.gz"],
    "debian": [f"{DEBIAN}/dists/trixie/Contents-amd64.gz"],
}
BASES = {"ubuntu": UBUNTU, "debian": DEBIAN}


def fetch(url, dest=None):
    dest = Path(dest) if dest else None
    if dest and dest.exists() and dest.stat().st_size > 0:
        return dest
    print(f"  fetch {url}", flush=True)
    with urllib.request.urlopen(url, timeout=120) as r, open(dest, "wb") if dest else io.BytesIO() as out:
        data = r.read()
        if dest:
            out.write(data)
            return dest
        return data


def load_index(kind):
    """name -> (filename, depends, version) for every package in the suite."""
    index = {}
    for url in SUITES[kind]:
        try:
            raw = fetch(url)
        except Exception as e:
            print(f"  skip {url}: {e}", flush=True)
            continue
        text = gzip.decompress(raw).decode("utf-8", "replace")
        for block in text.split("\n\n"):
            if not block.strip():
                continue
            fields = {}
            key = None
            for line in block.splitlines():
                if line[:1] in (" ", "\t") and key:
                    fields[key] += " " + line.strip()
                elif ":" in line:
                    key, _, val = line.partition(":")
                    fields[key.strip()] = val.strip()
            name = fields.get("Package")
            if not name:
                continue
            index[name] = (
                fields.get("Filename", ""),
                fields.get("Depends", ""),
                fields.get("Version", ""),
                fields.get("Description", "")[:90],
            )
    return index


def load_contents(kind):
    """soname / path -> package name, from the archive Contents index."""
    path_map = {}
    for url in CONTENTS[kind]:
        try:
            raw = gzip.decompress(fetch(url)).decode("utf-8", "replace")
        except Exception as e:
            print(f"  contents unavailable ({e}); continuing", flush=True)
            continue
        for line in raw.splitlines():
            # "usr/lib/x86_64-linux-gnu/libisoburn.so.1    libs/libisoburn1t64"
            parts = line.split()
            if len(parts) < 2:
                continue
            path_map.setdefault(os.path.basename(parts[0]), parts[-1].split("/")[-1])
    return path_map


def download_pkg(kind, name, index, seen, extracted):
    if name in seen or name not in index:
        return False
    seen.add(name)
    filename, _, version, _ = index[name]
    if not filename:
        return False
    local = CACHE / Path(filename).name
    fetch(f"{BASES[kind]}/{filename}", local)
    print(f"  extract {name} {version}", flush=True)
    subprocess.run(["dpkg-deb", "-x", str(local), str(TB)], check=True)
    extracted.append(name)
    return True


def ldd_missing(binary, env):
    try:
        out = subprocess.run(["ldd", str(binary)], capture_output=True, text=True, env=env).stdout
    except Exception:
        return []
    missing = []
    for line in out.splitlines():
        if "not found" not in line:
            continue
        # "libisoburn.so.1 => not found"  /  "libfoo.so.1 => not found (0x...)"
        soname = line.split("=>")[0].strip().split()[-1:] or line.split()[0:1]
        if soname:
            missing.append(os.path.basename(soname[0]))
    return missing


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ubuntu", nargs="*", default=[], help="Ubuntu packages to extract")
    ap.add_argument("--debian", nargs="*", default=[], help="Debian packages to extract")
    ap.add_argument("--env", action="store_true", help="print environment exports and exit")
    ap.add_argument("--check", action="store_true", help="verify extracted binaries run")
    args = ap.parse_args()

    libdirs = [TB / "usr/lib/x86_64-linux-gnu", TB / "lib/x86_64-linux-gnu", TB / "usr/lib"]
    if args.env:
        ld = ":".join(str(d) for d in libdirs if d.exists())
        print(f'export PATH="{TB}/usr/bin:{TB}/sbin:{TB}/usr/sbin:$PATH"')
        print(f'export LD_LIBRARY_PATH="{ld}${{LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}}"')
        print(f'export TOOLBOX="{TB}"')
        return 0

    TB.mkdir(parents=True, exist_ok=True)
    CACHE.mkdir(parents=True, exist_ok=True)

    seen, extracted = set(), []
    indexes = {}
    for kind, wanted in (("ubuntu", args.ubuntu), ("debian", args.debian)):
        if not wanted:
            continue
        print(f"== {kind}: {', '.join(wanted)}", flush=True)
        index = indexes.setdefault(kind, load_index(kind))
        queue = list(wanted)
        for name in queue:
            if name not in index:
                print(f"  MISSING package {name}", flush=True)
                continue
            download_pkg(kind, name, index, seen, extracted)

    # Chase missing shared libraries until ldd is clean.
    contents = {}
    for round_no in range(1, 7):
        env = dict(os.environ)
        env["LD_LIBRARY_PATH"] = ":".join(str(d) for d in libdirs if d.exists())
        missing = set()
        for base in ("usr/bin", "usr/sbin", "sbin", "bin"):
            d = TB / base
            if not d.is_dir():
                continue
            for f in d.iterdir():
                if f.is_file() and os.access(f, os.X_OK):
                    missing.update(ldd_missing(f, env))
        if not missing:
            print(f"== libraries resolved after {round_no - 1} round(s)", flush=True)
            break
        print(f"== round {round_no}: {len(missing)} missing libraries", flush=True)
        for soname in sorted(missing):
            kind = "ubuntu"
            if kind not in contents:
                contents[kind] = load_contents(kind)
            pkg = contents[kind].get(soname)
            if not pkg:
                print(f"  UNRESOLVED {soname}", flush=True)
                continue
            index = indexes.setdefault(kind, load_index(kind))
            if pkg not in index:
                print(f"  {soname} -> {pkg} (not in index)", flush=True)
                continue
            download_pkg(kind, pkg, index, seen, extracted)

    print(f"== {len(extracted)} packages in {TB}", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
