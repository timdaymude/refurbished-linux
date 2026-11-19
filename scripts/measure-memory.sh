#!/bin/sh
# Measure the idle memory cost of a Reawakening OS desktop.
#
#   ./scripts/measure-memory.sh            # inside the running session
#   ./scripts/measure-memory.sh --quiet    # only the summary
#
# Method: wait for the machine to settle, then add up the resident set size of
# every process. RSS double counts shared pages, which is why the session is
# also reported process by process. The number that matters for the project
# target is the sum of RSS - it is deliberately conservative (it over-reports,
# never under-reports, because shared libraries appear once per process).
set -eu

SETTLE=${SETTLE:-30}
QUIET=no
[ "${1:-}" = "--quiet" ] && QUIET=yes

say() { [ "$QUIET" = yes ] || printf '%s\n' "$*"; }

say "Waiting ${SETTLE}s for the desktop to settle (no applications open)..."
sleep "$SETTLE"

mem_kb() { awk -v k="$1" '$1 == k":" { print $2 }' /proc/meminfo; }
total_mb=$(( $(mem_kb MemTotal) / 1024 ))
avail_mb=$(( $(mem_kb MemAvailable) / 1024 ))

sum_rss() { ps -eo rss=,comm= | awk -v names="$1" '
    BEGIN { n = split(names, a, " ") }
    { for (i = 1; i <= n; i++) if ($2 == a[i]) { s += $1; break } }
    END { printf "%d", s / 1024 }'; }

all_rss=$(ps -eo rss= | awk '{ s += $1 } END { printf "%d", s / 1024 }')
procs=$(ps -e --no-headers | wc -l)

compositor=$(sum_rss "labwc")
shell=$(sum_rss "refurb-shell")
systemd=$(sum_rss "systemd systemd-journald systemd-logind systemd-udevd dbus-daemon")

say ""
say "Top processes by resident memory"
ps -eo rss=,comm=,args= --sort=-rss | head -12 | \
    awk '{ printf "  %6.1f MB  %s\n", $1 / 1024, $2 }'

say ""
say "Summary"
printf '  %-34s %s MB\n' "RAM installed" "$total_mb"
printf '  %-34s %s MB\n' "RAM available right now" "$avail_mb"
printf '  %-34s %s MB\n' "compositor (labwc)" "$compositor"
printf '  %-34s %s MB\n' "desktop shell (refurb-shell)" "$shell"
printf '  %-34s %s MB\n' "core system services" "$systemd"
printf '  %-34s %s MB\n' "sum of all process RSS" "$all_rss"
printf '  %-34s %s\n' "running processes" "$procs"

if [ "$all_rss" -lt 500 ]; then
    printf '\n  RESULT: under the 500 MB target (%s MB)\n' "$all_rss"
else
    printf '\n  RESULT: over the 500 MB target (%s MB) - record what is running before changing anything\n' "$all_rss"
fi

if [ "$all_rss" -ge 300 ] && [ "$all_rss" -le 400 ]; then
    printf '  RESULT: inside the 300-400 MB ideal band\n'
fi

say ""
say "For the record, also run:  free -m   and   cat /proc/swaps   (zram should be listed)"
