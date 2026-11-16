#!/bin/sh
# Build the Refurbished Linux ISO without root on the build machine.
#
#   ./scripts/mkiso.sh toolbox     # assemble the build toolchain (once)
#   ./scripts/mkiso.sh bootstrap   # debootstrap Debian trixie, extract phase
#   ./scripts/mkiso.sh bootstrap2  # debootstrap second stage (configure)
#   ./scripts/mkiso.sh packages    # install the package lists + Brave (long)
#   ./scripts/mkiso.sh configure   # shell package, shipped files, hooks
#   ./scripts/mkiso.sh squashfs    # filesystem.squashfs
#   ./scripts/mkiso.sh iso         # bootable hybrid ISO
#   ./scripts/mkiso.sh all         # everything, in order
#
# Why "without root": this build host has no root account, but it does allow
# unprivileged user namespaces. Every privileged step therefore runs inside
# `unshare -r`, where the build user is uid 0 in its own namespace. That is also
# how the CI build of this project works, and it means the build never touches
# the host system.
set -eu

ROOT=$(cd "$(dirname "$0")/.." && pwd)
DISTRO="$ROOT/distro"
WORK=${WORK:-$ROOT/.build}
ROOTFS="$WORK/rootfs"
ISODIR="$WORK/iso"
TOOLBOX=${TOOLBOX:-$ROOT/.toolbox}
VERSION=$(sed -n 's/^version *= *"\(.*\)"/\1/p' "$ROOT/shell/refurb-shell/Cargo.toml" | head -1)
ISO_NAME="refurbished-linux-${VERSION}-amd64.iso"
MIRROR=${MIRROR:-http://deb.debian.org/debian}
SUITE=${SUITE:-trixie}

PATH="$TOOLBOX/usr/bin:$TOOLBOX/usr/sbin:$TOOLBOX/sbin:$TOOLBOX/usr/sbin:$PATH"
export PATH
LD_LIBRARY_PATH="$TOOLBOX/usr/lib/x86_64-linux-gnu:$TOOLBOX/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export LD_LIBRARY_PATH

say() { printf '\n=== %s\n' "$*"; }

# Run a command as uid 0 inside a user namespace, with /proc available.
as_root() {
    unshare -r --map-root-user --mount --pid --fork --mount-proc -- "$@"
}

# ---------------------------------------------------------------- toolbox ---
stage_toolbox() {
    say "toolchain into $TOOLBOX"
    python3 "$ROOT/scripts/toolbox.py" \
        --ubuntu xorriso squashfs-tools mtools dosfstools isolinux \
                 syslinux-common syslinux-utils grub-common grub-pc-bin \
                 grub-efi-amd64-bin \
        --debian debootstrap
}

# -------------------------------------------------------------- bootstrap ---
stage_bootstrap() {
    say "debootstrap $SUITE into $ROOTFS"
    if [ -x "$ROOTFS/bin/sh" ]; then
        echo "already bootstrapped"
        return 0
    fi
    DEB="$TOOLBOX/usr/sbin/debootstrap"
    [ -x "$DEB" ] || DEB=$(command -v debootstrap)
    # debootstrap hard-codes EXTRACT_DEB_TAR_OPTIONS= (empty) in functions, and
    # tar preserves ownership by default, which cannot work with one mapped uid.
    FUNCS="$TOOLBOX/usr/share/debootstrap/functions"
    if [ -f "$FUNCS" ] && ! grep -q 'EXTRACT_DEB_TAR_OPTIONS="--no-same-owner"' "$FUNCS"; then
        sed -i 's/^EXTRACT_DEB_TAR_OPTIONS=$/EXTRACT_DEB_TAR_OPTIONS="--no-same-owner"/' "$FUNCS"
    fi
    mkdir -p "$WORK"
    # Only one uid is mapped in the user namespace, so nothing may chown to a
    # service group. Ownership is normalised to root when the squashfs is built
    # (-all-root), which is what the live system needs anyway.
    as_root env DEBOOTSTRAP_DIR="$TOOLBOX/usr/share/debootstrap" \
        EXTRACT_DEB_TAR_OPTIONS="--no-same-owner" TAR_OPTIONS="--no-same-owner" \
        "$DEB" --foreign --variant=minbase --arch=amd64 --no-check-gpg \
        --components=main,contrib,non-free,non-free-firmware \
        "$SUITE" "$ROOTFS" "$MIRROR"
}

# chown cannot work here (one mapped uid), so the second stage runs with a shim
# that makes chown a no-op. Ownership is normalised by mksquashfs -all-root.
stage_bootstrap2() {
    say "debootstrap second stage"
    [ -f "$WORK/libchownshim.so" ] || {
        echo "build the shim first: rustc --crate-type cdylib -O -o $WORK/libchownshim.so scripts/chown-shim.rs" >&2
        exit 1
    }
    install -m 644 "$WORK/libchownshim.so" "$ROOTFS/lib/libchownshim.so"
    as_root env LD_PRELOAD=/lib/libchownshim.so TAR_OPTIONS="--no-same-owner" \
        DEBIAN_FRONTEND=noninteractive chroot "$ROOTFS" /debootstrap/debootstrap --second-stage
}

# --------------------------------------------------------------- packages ---
stage_packages() {
    say "installing the Refurbished Linux package set"
    install -d "$ROOTFS/etc/apt/apt.conf.d" "$ROOTFS/usr/share/keyrings"
    cp /etc/resolv.conf "$ROOTFS/etc/resolv.conf"

    cat > "$ROOTFS/etc/apt/sources.list" <<EOF
deb $MIRROR $SUITE main contrib non-free non-free-firmware
deb $MIRROR ${SUITE}-updates main contrib non-free non-free-firmware
deb http://security.debian.org/debian-security ${SUITE}-security main contrib non-free non-free-firmware
EOF

    # Brave from its own archive, with the key this repository already ships.
    cp "$DISTRO/config/archives/brave.key.chroot" \
       "$ROOTFS/usr/share/keyrings/brave-browser-archive-keyring.gpg"
    cp "$DISTRO/config/archives/brave.list.chroot" \
       "$ROOTFS/etc/apt/sources.list.d/brave.list"
    install -d "$ROOTFS/etc/apt/sources.list.d"

    # Nothing may start services while the image is being built.
    printf '#!/bin/sh\nexit 101\n' > "$ROOTFS/usr/sbin/policy-rc.d"
    chmod 755 "$ROOTFS/usr/sbin/policy-rc.d"

    # dpkg is running as root inside a user namespace where only uid 0 is
    # mapped, so attempts to chown files to daemon users cannot succeed. The
    # chown/chgrp shims let maintainer scripts finish; real ownership is set
    # when the squashfs is created.
    install -d "$ROOTFS/usr/local/sbin"
    for shim in chown chgrp; do
        printf '#!/bin/sh\nexit 0\n' > "$ROOTFS/usr/local/sbin/$shim"
        chmod 755 "$ROOTFS/usr/local/sbin/$shim"
    done
    install -d "$ROOTFS/etc/apt/apt.conf.d"
    printf 'DPkg::Options { "--force-not-root"; };\n' \
        > "$ROOTFS/etc/apt/apt.conf.d/99-build-chroot"

    PKGS=$(sed -e 's/#.*//' -e '/^[[:space:]]*$/d' "$DISTRO"/config/package-lists/*.list.chroot | tr '\n' ' ')
    echo "packages: $(echo "$PKGS" | wc -w)"

    as_root env LD_PRELOAD=/lib/libchownshim.so TAR_OPTIONS="--no-same-owner" chroot "$ROOTFS" /bin/bash -c "
        set -e
        export DEBIAN_FRONTEND=noninteractive LC_ALL=C
        # apt normally drops to the _apt user via setgroups, which a user
        # namespace cannot do; running the fetch as root is fine in a build.
        # ca-certificates must exist before the HTTPS Brave archive can be read.
        apt-get -o APT::Sandbox::User=root update
        apt-get -y --no-install-recommends -o APT::Sandbox::User=root \
                install ca-certificates
        apt-get -o APT::Sandbox::User=root update
        apt-get -y --no-install-recommends \
                -o APT::Sandbox::User=root \
                -o Dpkg::Options::=--force-confold \
                -o Dpkg::Options::=--force-confdef install $PKGS
    " 2>&1 | tee "$WORK/packages.log" | tail -5
}

# -------------------------------------------------------------- configure ---
stage_configure() {
    say "shipping the shell, its configuration and the hooks"
    [ -f "$DISTRO/config/packages.chroot/refurb-shell_${VERSION}_amd64.deb" ] || \
        "$ROOT/scripts/build-shell.sh"

    cp -a "$DISTRO/config/includes.chroot/." "$ROOTFS/"
    install -d "$ROOTFS/tmp/debs"
    cp "$DISTRO"/config/packages.chroot/*.deb "$ROOTFS/tmp/debs/"

    as_root env LD_PRELOAD=/lib/libchownshim.so TAR_OPTIONS="--no-same-owner" chroot "$ROOTFS" /bin/bash -c "
        set -e
        export DEBIAN_FRONTEND=noninteractive LC_ALL=C
        dpkg -i /tmp/debs/*.deb || apt-get -y -f install
        rm -rf /tmp/debs
    "

    # The hooks are the same files the live-build tree uses.
    for hook in "$DISTRO"/config/hooks/normal/*.hook.chroot; do
        echo "hook $(basename "$hook")"
        install -m 755 "$hook" "$ROOTFS/tmp/hook.sh"
        as_root chroot "$ROOTFS" /bin/bash -c 'set -e; export PATH=/usr/sbin:/usr/bin:/sbin:/bin; /tmp/hook.sh'
        rm -f "$ROOTFS/tmp/hook.sh"
    done

    # live-boot needs an initramfs that knows how to find the squashfs.
    as_root env LD_PRELOAD=/lib/libchownshim.so TAR_OPTIONS="--no-same-owner" chroot "$ROOTFS" /bin/bash -c "
        set -e
        export PATH=/usr/sbin:/usr/bin:/sbin:/bin
        mkdir -p /etc/initramfs-tools/conf.d
        echo 'MODULES=most' > /etc/initramfs-tools/conf.d/refurb.conf
        update-initramfs -u -k all
    "

    # Placeholder for a machine-specific installation tool; see docs/install.md.
    rm -f "$ROOTFS/usr/sbin/policy-rc.d" "$ROOTFS/etc/apt/apt.conf.d/99-build-chroot"
    rm -f "$ROOTFS/usr/local/sbin/chown" "$ROOTFS/usr/local/sbin/chgrp"
    rm -f "$ROOTFS/lib/libchownshim.so"
    rm -f "$ROOTFS/etc/resolv.conf"
    echo "refurbished" > "$ROOTFS/etc/hostname"
}

# --------------------------------------------------------------- squashfs ---
stage_squashfs() {
    say "squashfs"
    install -d "$ISODIR/live"
    cp "$ROOTFS/boot/vmlinuz-"* "$ISODIR/live/vmlinuz"
    cp "$ROOTFS/boot/initrd.img-"* "$ISODIR/live/initrd.img"
    rm -f "$WORK/filesystem.squashfs"
    mksquashfs "$ROOTFS" "$WORK/filesystem.squashfs" \
        -comp zstd -Xcompression-level 19 -b 1M -no-progress -wildcards -all-root \
        -e 'tmp/*' -e 'var/cache/apt/*' -e 'var/lib/apt/lists/*' \
        -e 'root/*' -e 'proc/*' -e 'sys/*' -e 'dev/*' -e 'run/*'
    cp "$WORK/filesystem.squashfs" "$ISODIR/live/filesystem.squashfs"
    du -h "$ISODIR/live/filesystem.squashfs"
}

# -------------------------------------------------------------------- iso ---
stage_iso() {
    say "boot loaders"
    install -d "$ISODIR/isolinux" "$ISODIR/boot/grub" "$ISODIR/EFI/boot"

    ISOLINUX_DIR="$TOOLBOX/usr/lib/ISOLINUX"
    SYSLINUX_DIR="$TOOLBOX/usr/lib/syslinux/modules/bios"
    cp "$ISOLINUX_DIR/isolinux.bin" "$ISODIR/isolinux/"
    for f in ldlinux.c32 libcom32.c32 libutil.c32 menu.c32 vesamenu.c32; do
        [ -f "$SYSLINUX_DIR/$f" ] && cp "$SYSLINUX_DIR/$f" "$ISODIR/isolinux/"
    done

    cat > "$ISODIR/isolinux/isolinux.cfg" <<'EOF'
UI menu.c32
PROMPT 0
TIMEOUT 30
DEFAULT live

MENU TITLE Refurbished Linux 0.2 - Focus

LABEL live
  MENU LABEL Start Refurbished Linux
  KERNEL /live/vmlinuz
  APPEND initrd=/live/initrd.img boot=live components console=ttyS0,115200 quiet splash

LABEL live-nosplash
  MENU LABEL Start Refurbished Linux (verbose boot)
  KERNEL /live/vmlinuz
  APPEND initrd=/live/initrd.img boot=live components console=ttyS0,115200

EOF

    # UEFI: a self-contained grub image, so no second filesystem is needed.
    GRUB_MOD="$TOOLBOX/usr/lib/grub/x86_64-efi"
    if [ -d "$GRUB_MOD" ]; then
        cat > "$WORK/grub-efi.cfg" <<'EOF'
set timeout=3
set default=0
menuentry "Start Refurbished Linux" {
    linux /live/vmlinuz boot=live components console=ttyS0,115200 quiet splash
    initrd /live/initrd.img
}
menuentry "Start Refurbished Linux (verbose boot)" {
    linux /live/vmlinuz boot=live components
    initrd /live/initrd.img
}
EOF
        grub-mkstandalone --directory "$GRUB_MOD" -O x86_64-efi \
            --modules="part_gpt part_msdos fat exfat ext2 normal linux search search_fs_uuid configfile echo all_video gfxterm" \
            --locales="" --fonts="" --themes="" \
            --output="$WORK/bootx64.efi" \
            "boot/grub/grub.cfg=$WORK/grub-efi.cfg" || echo "grub-mkstandalone failed: BIOS-only ISO" >&2
    fi
    if [ -f "$WORK/bootx64.efi" ]; then
        rm -f "$WORK/efi.img"
        dd if=/dev/zero of="$WORK/efi.img" bs=1M count=10 status=none
        mkfs.vfat -n REFURBEFI "$WORK/efi.img" >/dev/null
        mmd -i "$WORK/efi.img" ::EFI ::EFI/BOOT
        mcopy -i "$WORK/efi.img" "$WORK/bootx64.efi" ::EFI/BOOT/BOOTX64.EFI
        cp "$WORK/efi.img" "$ISODIR/boot/grub/efi.img"
    else
        echo "grub x86_64-efi modules missing: building a BIOS-only ISO" >&2
    fi

    say "xorriso"
    rm -f "$ROOT/$ISO_NAME"
    MBR="$TOOLBOX/usr/lib/ISOLINUX/isohdpfx.bin"
    xorriso -as mkisofs \
        -iso-level 3 \
        -full-iso9660-filenames \
        -volid "REFURBISHED_LINUX" \
        -publisher "Refurbished Linux" \
        -A "Refurbished Linux $VERSION" \
        -isohybrid-mbr "$MBR" \
        -eltorito-boot isolinux/isolinux.bin \
        -eltorito-catalog isolinux/boot.cat \
        -no-emul-boot -boot-load-size 4 -boot-info-table \
        -eltorito-alt-boot -e boot/grub/efi.img -no-emul-boot \
        -isohybrid-gpt-basdat \
        -output "$ROOT/$ISO_NAME" \
        "$ISODIR"

    say "result"
    ls -l "$ROOT/$ISO_NAME"
    sha256sum "$ROOT/$ISO_NAME" | tee "$ROOT/$ISO_NAME.sha256"
}

case "${1:-all}" in
    toolbox)   stage_toolbox ;;
    bootstrap) stage_bootstrap ;;
    bootstrap2) stage_bootstrap2 ;;
    packages)  stage_packages ;;
    configure) stage_configure ;;
    squashfs)  stage_squashfs ;;
    iso)       stage_iso ;;
    all)
        stage_toolbox
        stage_bootstrap
        stage_bootstrap2
        stage_packages
        stage_configure
        stage_squashfs
        stage_iso
        ;;
    *) echo "usage: $0 {toolbox|bootstrap|packages|configure|squashfs|iso|all}" >&2; exit 2 ;;
esac
