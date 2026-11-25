# dde-store, rebuilt with one upstream fix

Reawakening OS ships **dde-store 1.2.5-2** from Debian trixie as its graphical
application store. This directory produces the package the image installs, from
Debian's own source, with a single additional patch.

## Why a local build

`pages/homepage.cpp` in 1.2.5 builds nine category rows with:

```c++
for (int i = 0; i < 10; i++) {
    list->addItem(items[i]->name, items[i]->icon, i);
```

`items` is whatever AppStream returned for that category. On a Debian system a
category usually has fewer than ten desktop applications, so `items[i]` reads
past the end of the list. That is undefined behaviour in the main window's
constructor: the store crashes before it can draw anything. It is exactly the
segfault seen on the Chromebook after `qtwayland5` was installed.

Debian fixed it in 1.2.5-2.1 (`qMin(10, items.size())`, upstream PR #34,
[Debian #1117833](https://bugs.debian.org/1117833)), but that upload is also a
Qt 6 / DTK 6 port. Trixie has Qt 5 DTK and no Qt 6 DTK, so pulling 1.2.5-2.1
from unstable would drag a second Qt stack into a 2 GB machine - or fail to
install at all. `002-homepage-short-category.patch` therefore backports the
bound only; no other code, dependency or Qt version changes.

The other Debian patch (001, the updates-page null check) is already in
1.2.5-2 and stays.

`003-ratings-must-not-block-catalogue.patch` fixes the second failure found on
the r15 test ISO: the store only shows a category or search result after its
star-ratings request to `odrs.gnome.org` has finished, and on a machine that
cannot reach that server (offline, captive portal, IPv6-only DNS answer) the
catalogue stayed empty forever. The request now has a four-second timeout and
a four-second fallback, after which the local catalogue is shown without
ratings.

## Build

```sh
./packaging/dde-store/build.sh
```

The script fetches the pinned source tarballs (SHA-256 checked), builds them in
a throw-away trixie chroot, and writes:

```
distro/config/packages.chroot/dde-store_1.2.5-2.2~refurb2_amd64.deb
```

`mkiso.sh` installs every `.deb` in `packages.chroot`, so the next ISO contains
this build. The version sorts above Debian's `1.2.5-2`, which keeps a later
`apt upgrade` from silently replacing the fix with the crashing build.

## Upstream obligations

* Package licence: GPL-3.0-or-later (unchanged; `debian/copyright` ships in the
  package).
* Patch origin and bug reference are recorded in the patch header.
* When Debian's trixie package gains the fix, delete this directory and the
  `packages.chroot` package, and use the archive version again.
