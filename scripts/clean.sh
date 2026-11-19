#!/bin/sh
# Reclaim disk space without touching anything that cannot be rebuilt.
#
#   ./scripts/clean.sh --dry-run    # show what would go
#   ./scripts/clean.sh              # build output + toolchain; newest ISO stays
#   ./scripts/clean.sh --keep-isos  # same, but never touch a local ISO
#
# Removed: .build/{rootfs,iso,shots,shots7}, build logs, QEMU screenshots and
# state, .toolbox/ (the host toolchain) and shell/refurb-shell/target/.
# Older ISOs go too, but only when releases/*.sha256 already records the
# checksum, so no published image loses its last known hash.
#
# Never removed: tracked files, .git, releases/, docs/, the newest ISO and its
# .sha256, scripts/qemu/. Every path below is checked against .gitignore first.
set -eu

ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"

DRY=0
KEEP_ISOS=0
for arg in "$@"; do
    case $arg in
        --dry-run) DRY=1 ;;
        --keep-isos) KEEP_ISOS=1 ;;
        -h|--help) sed -n '2,18p' "$0"; exit 0 ;;
        *) echo "clean.sh: unknown option: $arg" >&2; exit 2 ;;
    esac
done

if [ ! -d .git ] && [ ! -f .git ]; then
    echo "clean.sh: $ROOT is not a git checkout; refusing to delete" >&2
    exit 1
fi

# Never pull the floor out from under a build that is still running.
if pgrep -f '[m]kiso\.sh|[m]ksquashfs|[d]ebootstrap|[c]argo build' >/dev/null 2>&1; then
    echo "clean.sh: a build is running (mkiso/debootstrap/mksquashfs/cargo); stop it first" >&2
    exit 1
fi

before=$(du -sk . | cut -f1)
removed=0

remove() {
    for path in "$@"; do
        [ -e "$path" ] || continue
        if ! git check-ignore -q -- "$path"; then
            echo "clean.sh: keeping $path (not ignored by git, so possibly source)" >&2
            continue
        fi
        sz=$(du -sk "$path" 2>/dev/null | cut -f1)
        removed=$((removed + sz))
        if [ "$DRY" = 1 ]; then
            echo "would remove  $path  ($((sz / 1024)) MB)"
        else
            echo "removing      $path  ($((sz / 1024)) MB)"
            rm -rf -- "$path"
        fi
    done
}

# Regenerable build trees.
remove .build/rootfs .build/iso .build/shots .build/shots7 .build/__pycache__ \
       .toolbox shell/refurb-shell/target

# Loose build output: squashfs, EFI image, bootloader, logs, screendumps, QEMU state.
for f in .build/*.squashfs .build/efi.img .build/*.efi .build/*.log \
         .build/*.png .build/*.ppm .build/*.fd .build/*.sock; do
    remove "$f"
done

# Older ISOs, newest kept. A checksum in releases/ must already exist.
if [ "$KEEP_ISOS" = 0 ]; then
    newest=$(ls -1t refurbished-linux-*.iso 2>/dev/null | head -1 || true)
    for iso in refurbished-linux-*.iso; do
        [ -e "$iso" ] || continue
        if [ "$iso" = "$newest" ]; then
            echo "keeping       $iso (newest; local recovery copy)"
            continue
        fi
        if ! grep -rqsF -- "$iso" releases/; then
            echo "clean.sh: keeping $iso (no recorded checksum in releases/)" >&2
            continue
        fi
        remove "$iso"
    done
fi

after=$(du -sk . | cut -f1)
echo
if [ "$DRY" = 1 ]; then
    echo "would reclaim ~$((removed / 1024)) MB; workspace $((before / 1024)) MB now"
else
    echo "reclaimed ~$((removed / 1024)) MB; workspace $((before / 1024)) MB -> $((after / 1024)) MB"
fi
