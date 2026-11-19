# Release archive

Every image this project has built, its recorded hash, and where a copy lives.
Hashes are the ones published with the release; `releases/*.sha256` holds the
full per-release report.

| Release | Size (bytes) | SHA256 | Published | Local copy |
|---|---|---|---|---|
| 0.2.0 "Focus" | 810,549,248 | `777b498360dc218e02a88a4622fbc10ce1094d579d9c51c14dd48f77814db003` | `v0.2.0` — `refurbished-linux-0.2.0-amd64.iso` | none |
| 0.3.0 "First Desktop" | 810,549,248 | `b933349293d72034c86288f4c4a40b3ac086b1ee18fc95307befb4878fe00bff` | `v0.3.0` — `refurbished-linux-0.3.0-amd64.iso` | none |
| 0.3.0-r2 UEFI boot fix | 810,549,248 | `676c08c9532e75881a4119b0e5868c2b2fa0a15772030b7f5b575cff5eb06d27` | `v0.3.0-r2` — `refurbished-linux-0.3.0-r2-amd64.iso` | none |
| 0.3.0-r3 hardware | 824,180,736 | `7262e2dc5470fc4ee41e67ac22f58d9a6c1e73ff203edd061e7d950f3d9417e1` | `v0.3.0-r3` — asset is named `r3.iso` | none |
| 0.3.0-r4 displays | 824,180,736 | `b1707dd575729786022768df37dc636ded16262b3c955ec59a5136cd9cbcd821` | `v0.3.0-r4` — `refurbished-linux-0.3.0-r4-amd64.iso` | none |
| 0.3.0-r5 controls | 824,180,736 | `a3342d3ac2ae90185afa8ec79ed8cbbbb25dada56c351b243ba91f3d6d60bdd9` | `v0.3.0-r5` — `refurbished-linux-0.3.0-r5-amd64.iso` | none |
| 0.3.0-r6 hardware | 824,180,736 | `5cdce8ee0e9dfba8d4f1ec6324887504c3d50822fb9318605bdf481c9bbe2e7e` | **never published** (no tag, no release) | `~/refurbished-linux-releases/` |
| 0.3.0-r7 components | 930,086,912 | `320a42e55ed129fb6741994e8bbeeea47444fa0065bf3c9b915d22728c5fd019` | `v0.3.0-r7` — `refurbished-linux-0.3.0-amd64.iso` | none |
| 0.3.0-r8 session | 929,038,336 | `38f428fa5497126e8f436a53c069dbf2cfb3e87e125b5cfe92d2af381bf04a10` | `v0.3.0-r8` — `refurbished-linux-0.3.0-r8-amd64.iso` | none |
| 0.3.0-r9 chromebook | 929,038,336 | `4c7548668ee635d0e60f8722ff879507a3b7445a0f7468f92e7f470344b7387e` | `v0.3.0-r9` — `refurbished-linux-0.3.0-r9-amd64.iso` | workspace root |

## How each one was verified

- **r9 and r8**: the published asset was downloaded again and hashed. Both match
  the local file byte for byte, so the release is a complete backup, not just a
  recorded number.
- **r3, r4, r5, r7, 0.2.0, 0.3.0**: the local ISO hashed to the value recorded
  with the release, and the published asset has the same byte size. The bytes
  themselves were not re-downloaded.
- **r6** is the exception: it was superseded before it was published, so the
  local ISO is the only copy and is kept for that reason.

Local copies of published images were removed once the checks above passed.
A published asset plus its hash in this file is the durable copy; `gh release
download v0.3.0-r9 --pattern '*.iso'` brings any of them back.

## Rebuilding

Every image above is reproducible from its tag:

    git checkout v0.3.0-r9
    ./scripts/mkiso.sh all

The build needs ~6 GB of free disk. `scripts/clean.sh` reclaims it afterwards;
see `docs/iso-build.md`.
