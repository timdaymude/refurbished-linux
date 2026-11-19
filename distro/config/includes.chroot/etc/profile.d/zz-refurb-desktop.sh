# Start the Reawakening OS desktop from the account's own login session.
#
# 0.3.0-r7 started labwc from a *system* unit through runuser. That produced a
# graphical session with no logind session behind it, and every architectural
# problem followed from there: polkit saw an inactive subject (so NetworkManager
# writes, Blueman's radio switch and udisks2 mounts were refused), there was no
# systemd user instance (so PipeWire had to be spawned by hand and could not be
# restarted), and there was nowhere for XDG_SESSION_ID or a seat to come from.
#
# Logging in normally on tty1 (agetty --autologin) makes pam_systemd register a
# real user session. This hook is the only thing that turns that login into the
# desktop, and only on the first VT: every other login shell (tty2-tty6, SSH, a
# terminal in the desktop) is left alone.

# Interactive login shells only.
case "$-" in
*i*) ;;
*) return 0 ;;
esac

# Never start a second compositor from inside the running one, and never
# hijack a shell that already belongs to a graphical session.
[ -z "${WAYLAND_DISPLAY:-}" ] || return 0
[ -z "${DISPLAY:-}" ] || return 0
[ -z "${REFURB_SESSION_ACTIVE:-}" ] || return 0
# Never run the desktop as root, whatever account the console was logged in as.
[ "$(id -u 2>/dev/null || echo 0)" -ge 1000 ] || return 0
[ "$(tty 2>/dev/null)" = "/dev/tty1" ] || return 0
[ -x /usr/bin/refurb-session ] || return 0

export REFURB_SESSION_ACTIVE=1
exec /usr/bin/refurb-session
