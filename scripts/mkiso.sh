#!/bin/sh
# Build the Reawakening OS ISO without root on the build machine.
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
# The release round is part of the file name, as it has been since r2. It is
# deliberately separate from the shell version: 0.3.0 has had nine images.
RELEASE=${RELEASE:-r10}
ISO_NAME="reawakening-os-${VERSION}-${RELEASE}-amd64.iso"
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
    say "installing the Reawakening OS package set"
    install -d "$ROOTFS/etc/apt/apt.conf.d" "$ROOTFS/usr/share/keyrings"
    cp /etc/resolv.conf "$ROOTFS/etc/resolv.conf"

    cat > "$ROOTFS/etc/apt/sources.list" <<EOF
deb $MIRROR $SUITE main contrib non-free non-free-firmware
deb $MIRROR ${SUITE}-updates main contrib non-free non-free-firmware
deb http://security.debian.org/debian-security ${SUITE}-security main contrib non-free non-free-firmware
EOF

    # Third-party archives: every <name>.list.chroot is a source and the
    # matching <name>.key.chroot is the key it is signed with. Brave keeps its
    # upstream keyring file name because its list names that exact path.
    install -d "$ROOTFS/etc/apt/sources.list.d" "$ROOTFS/usr/share/keyrings" \
               "$ROOTFS/etc/apt/trusted.gpg.d"
    for list in "$DISTRO"/config/archives/*.list.chroot; do
        [ -e "$list" ] || continue
        name=$(basename "$list" .list.chroot)
        key="$DISTRO/config/archives/$name.key.chroot"
        dest="/usr/share/keyrings/$name.gpg"
        [ "$name" = brave ] && dest=/usr/share/keyrings/brave-browser-archive-keyring.gpg
        cp "$list" "$ROOTFS/etc/apt/sources.list.d/$name.list"
        if [ -f "$key" ]; then
            install -m 644 "$key" "$ROOTFS$dest"
            # A copy in trusted.gpg.d as well: it costs a couple of kilobytes
            # and covers an apt that does not honour signed-by for this file.
            install -m 644 "$key" "$ROOTFS/etc/apt/trusted.gpg.d/$name.gpg"
        fi
    done

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

    # The package stage removed the resolver with the rest of the live-only
    # files, but the store hook refreshes the AppStream catalogue from the
    # archive. Give the chroot a working resolver for as long as it is built,
    # and take it out again at the end, where the live system replaces it.
    cp /etc/resolv.conf "$ROOTFS/etc/resolv.conf" 2>/dev/null || true

    # An earlier build may have shipped read-only files (sudoers.d is 0440);
    # as the unprivileged build user that makes this copy fail and silently
    # skips every hook after it. Make the tree writable for its owner first.
    chmod -R u+rwX "$ROOTFS" 2>/dev/null || true

    # Git does not store directory modes: a checkout made under a restrictive
    # umask has 0700 directories, and `cp -a` copies that onto /etc, /usr and
    # the image root. A session running as a normal user then cannot traverse
    # /usr, and login fails with "no shell: Permission denied". Normalise the
    # directory modes only; file modes are meaningful (sudoers 0440, shadow
    # 0640) and are deliberately left alone.
    find "$DISTRO/config/includes.chroot" -type d -exec chmod 0755 {} +
    cp -a "$DISTRO/config/includes.chroot/." "$ROOTFS/"

    # The wallpapers are kept once, in the repository's assets directory, and
    # installed into the image here: an ISO build never depends on a copy that
    # only exists in a build tree.
    install -d "$ROOTFS/usr/share/backgrounds/reawakening"
    # Every picture in assets/wallpapers is shipped; the catalogue in
    # wallpaper.rs names them, and check-config.sh verifies the two lists match.
    install -m 644 "$ROOT"/assets/wallpapers/*.png \
                   "$ROOT"/assets/wallpapers/*.jpg \
                   "$ROOTFS/usr/share/backgrounds/reawakening/"

    # The boot menu theme. The installed system gets the same files the live
    # ISO uses, at the path /etc/default/grub points to, so one artwork and one
    # theme.txt serve both. Rendered by scripts/make-boot-theme.py.
    install -d "$ROOTFS/boot/grub/themes/reawakening"
    cp -a "$DISTRO/config/boot-theme/." "$ROOTFS/boot/grub/themes/reawakening/"
    # Replace rather than add: a package removed from packages.chroot must not
    # survive in the image because an earlier build left its .deb in /tmp.
    rm -rf "$ROOTFS/tmp/debs"
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

    # The build is rootless: tar runs as one mapped uid and silently drops the
    # setuid/setgid bits, so dpkg records them but the files stay 0755. The
    # squashfs then ships a sudo that refuses to run ("must be owned by uid 0
    # and have the setuid bit set") and a su that cannot change user. Restore
    # the bits the packages ask for, explicitly.
    as_root chroot "$ROOTFS" /bin/bash -c '
        set -e
        for f in /usr/bin/sudo /usr/bin/su /usr/bin/passwd /usr/bin/chsh \
                 /usr/bin/chfn /usr/bin/newgrp /usr/bin/gpasswd \
                 /usr/bin/mount /usr/bin/umount /usr/bin/pkexec \
                 /usr/lib/dbus-1.0/dbus-daemon-launch-helper; do
            if [ -e "$f" ]; then chmod 4755 "$f"; fi
        done
        for f in /usr/sbin/unix_chkpwd \
                 /usr/lib/openssh/ssh-keysign; do
            if [ -e "$f" ]; then chmod 2755 "$f"; fi
        done
        true
    '

    # live-boot needs an initramfs that knows how to find the squashfs.
    as_root env LD_PRELOAD=/lib/libchownshim.so TAR_OPTIONS="--no-same-owner" chroot "$ROOTFS" /bin/bash -c "
        set -e
        export PATH=/usr/sbin:/usr/bin:/sbin:/bin
        mkdir -p /etc/initramfs-tools/conf.d
        echo 'MODULES=most' > /etc/initramfs-tools/conf.d/refurb.conf
        update-initramfs -u -k all
    "

    # flatpak refuses to create its repository without
    # /proc/sys/kernel/random/boot_id, and the hook above runs inside a chroot
    # that has no /proc. Do it here, in a namespace that has both, from the
    # remote file the image already ships: no network, no DNS, no /proc in the
    # chroot.
    as_root bash -c "mount -t proc proc '$ROOTFS/proc' 2>/dev/null || true; chroot '$ROOTFS' flatpak remote-add --system --if-not-exists --from flathub /etc/flatpak/remotes.d/flathub.flatpakrepo" \
        || echo "WARNING: the flatpak repository was not initialised" >&2

    # Flathub's catalogue (appstream) is downloaded once, here, and shipped. The
    # alternative is a first-run download of 52 MB from inside the store, which
    # a 2 GB live session cannot even do (flatpak refuses to leave less than
    # 500 MB free on the overlay), and which leaves the store empty on a machine
    # with no network. Baking it means browsing works immediately and offline;
    # it costs about 30 MB in the compressed image.
    as_root bash -c "mount -t proc proc '$ROOTFS/proc' 2>/dev/null || true; chroot '$ROOTFS' flatpak update --appstream" \
        || echo "WARNING: the Flathub catalogue was not downloaded" >&2

    # Placeholder for a machine-specific installation tool; see docs/install.md.
    rm -f "$ROOTFS/usr/sbin/policy-rc.d" "$ROOTFS/etc/apt/apt.conf.d/99-build-chroot"
    rm -f "$ROOTFS/usr/local/sbin/chown" "$ROOTFS/usr/local/sbin/chgrp"
    rm -f "$ROOTFS/lib/libchownshim.so"
    rm -f "$ROOTFS/etc/resolv.conf"
    echo "reawakening-pc" > "$ROOTFS/etc/hostname"
}

# --------------------------------------------------------------- squashfs ---
stage_squashfs() {
    say "squashfs"

    # Directory modes must not depend on the machine that assembled the tree.
    # An archive restore, a restrictive umask or a copied includes/ directory
    # can leave /etc and /usr at 0700, and then the finished image cannot even
    # log the user in:
    #   -- refurb: no shell: Permission denied
    # The file modes are unaffected, so only the directories are normalised here,
    # and the few that are private on purpose are put back explicitly.
    find "$ROOTFS" -xdev -type d -exec chmod 0755 {} +
    # The session helpers are shell scripts, not data: a copy or an archive
    # restore that loses the executable bit turns them into files that can only
    # fail ("bash: /usr/lib/refurb/refurb-diag: Permission denied").
    find "$ROOTFS/usr/lib/refurb" -maxdepth 1 -type f -exec chmod 0755 {} +
    for d in root home etc/ssl/private; do
        [ -d "$ROOTFS/$d" ] && chmod 0700 "$ROOTFS/$d"
    done
    for d in "$ROOTFS"/home/*; do
        [ -d "$d" ] && chmod 0700 "$d"
    done
    [ -d "$ROOTFS/tmp" ] && chmod 1777 "$ROOTFS/tmp"
    [ -d "$ROOTFS/var/tmp" ] && chmod 1777 "$ROOTFS/var/tmp"
    [ -d "$ROOTFS/var/spool/cron/crontabs" ] && chmod 1730 "$ROOTFS/var/spool/cron/crontabs"
    [ -d "$ROOTFS/etc/polkit-1/rules.d" ] && chmod 0755 "$ROOTFS/etc/polkit-1/rules.d"

    # Restore the setuid bits the packages ask for. A filesystem assembled from
    # an extracted squashfs (or extracted without privileges) has them cleared,
    # and then sudo refuses to run at all:
    #   sudo: /usr/bin/sudo must be owned by uid 0 and have the setuid bit set
    # which leaves the machine with no way to perform an administrative task.
    for f in usr/bin/sudo usr/bin/su usr/bin/passwd usr/bin/chsh usr/bin/chfn \
             usr/bin/newgrp usr/bin/gpasswd usr/bin/mount usr/bin/umount \
             usr/bin/pkexec usr/lib/dbus-1.0/dbus-daemon-launch-helper; do
        [ -e "$ROOTFS/$f" ] && chmod 4755 "$ROOTFS/$f"
    done
    for f in usr/sbin/unix_chkpwd usr/lib/openssh/ssh-keysign; do
        [ -e "$ROOTFS/$f" ] && chmod 2755 "$ROOTFS/$f"
    done

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

    # The BIOS menu is drawn by syslinux's graphical menu when its module is
    # present, and by the plain text menu when it is not: the image must boot
    # either way, so the module choice is made here, not assumed.
    MENU_UI=menu.c32
    [ -f "$SYSLINUX_DIR/vesamenu.c32" ] && MENU_UI=vesamenu.c32

    install -d "$ISODIR/boot-theme"
    cp "$DISTRO/config/boot-theme/isolinux.png" "$ISODIR/boot-theme/"

    cat > "$ISODIR/isolinux/isolinux.cfg" <<'EOF'
UI __MENU_UI__
PROMPT 0
TIMEOUT 50
DEFAULT live

MENU RESOLUTION 1024 768
MENU BACKGROUND /boot-theme/isolinux.png
MENU TITLE
MENU MARGIN 12
MENU ROWS 6
MENU COLOR border 30;44 #00000000 #00000000 none
MENU COLOR title 1;36;44 #ff35c2a3 #00000000 none
MENU COLOR sel 7;37;40 #ffeceef2 #3a2320 none
MENU COLOR unsel 37;40 #ffa7aebb #00000000 none
MENU COLOR hotkey 1;37;44 #ffe8835c #00000000 none
MENU COLOR help 37;40 #ff737b8a #00000000 none
MENU COLOR timeout_msg 37;40 #ff737b8a #00000000 none
MENU COLOR timeout 1;37;40 #ffe8835c #00000000 none
MENU COLOR screen 37;40 #00000000 #00000000 none

LABEL live
  MENU LABEL Start Reawakening OS
  KERNEL /live/vmlinuz
  APPEND initrd=/live/initrd.img boot=live components console=tty0 console=ttyS0,115200 live-config.nottyautologin quiet splash

LABEL live-nosplash
  MENU LABEL Start Reawakening OS (verbose boot)
  KERNEL /live/vmlinuz
  APPEND initrd=/live/initrd.img boot=live components console=tty0 console=ttyS0,115200 live-config.nottyautologin

LABEL live-diag
  MENU LABEL Memory report (support / benchmark boot)
  KERNEL /live/vmlinuz
  APPEND initrd=/live/initrd.img boot=live components console=tty0 console=ttyS0,115200 live-config.nottyautologin quiet refurb.measure=1

EOF

    sed -i "s|^UI __MENU_UI__|UI $MENU_UI|" "$ISODIR/isolinux/isolinux.cfg"

    # Testing hooks, off by default so the release menu is exactly as shipped:
    #   MENU_TIMEOUT=100      longer menu (lets a test send key presses)
    #   BOOT_EXTRA="..."      extra kernel args on the default entry
    sed -i "s|^TIMEOUT 50|TIMEOUT ${MENU_TIMEOUT:-50}|" "$ISODIR/isolinux/isolinux.cfg"
    if [ -n "${BOOT_EXTRA:-}" ]; then
        # A test boot wants the messages, so quiet goes away entirely.
        sed -i "s|quiet splash|${BOOT_EXTRA}|" "$ISODIR/isolinux/isolinux.cfg"
    fi

    # UEFI: a self-contained grub image, so no second filesystem is needed.
    GRUB_MOD="$TOOLBOX/usr/lib/grub/x86_64-efi"
    if [ -d "$GRUB_MOD" ]; then
        # GRUB starts with $root on the device it was itself loaded from. On
        # UEFI that is the 10 MB FAT image carrying this binary (label
        # REFURBEFI), which never contains /live/vmlinuz, so a bare
        # "linux /live/vmlinuz" fails on real hardware with
        #   error: file '/live/vmlinuz' not found.
        #   error: you need to load the kernel first.
        # Find the medium that really holds /live instead: by kernel file
        # first, then by squashfs, then by volume label. This works for a CD,
        # a DVD and an ISO dd-written to a USB stick.
        # Note: GRUB's script language has no ${VAR:-default} expansion, so
        # nothing here may use shell parameter syntax; BOOT_EXTRA, MENU_TIMEOUT
        # and the test title are patched in with sed below, as for syslinux.
        # The graphical theme is drawn by gfxterm, which needs a font: the
        # console font GRUB ships is embedded in the image below, so no file
        # outside the ISO is ever required for the menu to appear.
        install -d "$ISODIR/boot/grub/themes/reawakening"
        cp -a "$DISTRO/config/boot-theme/." "$ISODIR/boot/grub/themes/reawakening/"

        cat > "$WORK/grub-efi.cfg" <<'EOF'
set timeout=5
set default=0

insmod all_video
insmod gfxterm
insmod gfxmenu
insmod png
set gfxmode=auto,1920x1080,1600x900,1366x768,1024x768,800x600
loadfont ($prefix)/fonts/unicode.pf2
terminal_output gfxterm

# GRUB starts with $root on the device it was loaded from. On UEFI that is the
# 10 MB FAT image carrying this binary (label REFURBEFI), which never contains
# /live/vmlinuz, so a plain "linux /live/vmlinuz" dies on real hardware with
#   error: file '/live/vmlinuz' not found.
#   error: you need to load the kernel first.
# Locate the medium that really holds /live: kernel file, then squashfs, then
# volume label. Covers CD, DVD and an ISO dd-written to a USB stick.
set live_found=0
if search --no-floppy --set=root --file /live/vmlinuz; then
    set live_found=1
fi
if [ "$live_found" = 0 ]; then
    if search --no-floppy --set=root --file /live/filesystem.squashfs; then
        set live_found=1
    fi
fi
if [ "$live_found" = 0 ]; then
    if search --no-floppy --set=root --label REAWAKENING_OS; then
        set live_found=1
    fi
fi

# No gfxmenu theme on UEFI. On this GRUB (2.12, built standalone) the themed
# menu drew its frame and countdown bar but never its entry text, leaving a
# black panel with no visible entries - unusable on a UEFI-only machine.
# GRUB's own text menu lists all three entries, so that is used here on
# purpose: the artwork theme is the BIOS side's job (isolinux/vesamenu below)
# and the plain menu is the documented text fallback for UEFI.

menuentry "Start Reawakening OS" {
    linux ($root)/live/vmlinuz boot=live components console=tty0 console=ttyS0,115200 live-config.nottyautologin quiet splash
    initrd ($root)/live/initrd.img
}
menuentry "Start Reawakening OS (verbose boot)" {
    linux ($root)/live/vmlinuz boot=live components console=tty0 console=ttyS0,115200 live-config.nottyautologin
    initrd ($root)/live/initrd.img
}
menuentry "Memory report (support / benchmark boot)" {
    linux ($root)/live/vmlinuz boot=live components console=tty0 console=ttyS0,115200 live-config.nottyautologin quiet refurb.measure=1
    initrd ($root)/live/initrd.img
}
EOF
        sed -i "s|^set timeout=5|set timeout=${MENU_TIMEOUT:-5}|" "$WORK/grub-efi.cfg"
        if [ -n "${BOOT_EXTRA:-}" ]; then
            sed -i "s|quiet splash|${BOOT_EXTRA}|" "$WORK/grub-efi.cfg"
        fi
        if [ -n "${REFURB_GRUB_EXTRA:-}" ]; then
            sed -i "s|Start Reawakening OS\"|Start Reawakening OS${REFURB_GRUB_EXTRA}\"|" "$WORK/grub-efi.cfg"
        fi
        # iso9660/udf: reading the ISO volume itself; search_fs_file and
        # search_label: the --file and --label forms used above (plain
        # "search" alone only offers --fs-uuid).
        grub-mkstandalone --directory "$GRUB_MOD" -O x86_64-efi \
            --modules="part_gpt part_msdos fat exfat ext2 ntfs iso9660 udf normal linux search search_fs_uuid search_fs_file search_label test configfile echo all_video gfxterm gfxmenu png font gfxterm_background ahci ata pata scsi usb usbms uhci ohci ehci" \
            --locales="" --fonts="unicode" --themes="" \
            --output="$WORK/bootx64.efi" \
            "boot/grub/grub.cfg=$WORK/grub-efi.cfg" || echo "grub-mkstandalone failed: BIOS-only ISO" >&2
    fi
    if [ -f "$WORK/bootx64.efi" ]; then
        rm -f "$WORK/efi.img"
        dd if=/dev/zero of="$WORK/efi.img" bs=1M count=10 status=none
        mkfs.vfat -n REAWAKENING "$WORK/efi.img" >/dev/null
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
        -volid "REAWAKENING_OS" \
        -publisher "Reawakening OS" \
        -A "Reawakening OS $VERSION" \
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
