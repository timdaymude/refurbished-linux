#!/usr/bin/env python3
"""Render the Reawakening OS boot menu artwork.

The theme lives in one place, `distro/config/boot-theme/`, and is used twice:
the live ISO copies it into /boot/grub/themes, and the image build ships the
same files to the installed system. This script only draws the pictures, so
changing a colour means changing one constant rather than editing PNGs.

    ./scripts/make-boot-theme.py

Outputs, all written into distro/config/boot-theme/:

    background.png     1920x1080 GRUB desktop
    isolinux.png       1024x768  syslinux (BIOS) menu background
    select_*.png       the nine slices GRUB draws behind the selected entry

The colours are the shell's (shell/refurb-shell/src/theme.rs): the window
background, the clay accent and the mint accent.
"""

from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "distro" / "config" / "boot-theme"
FONTS = ROOT / "shell" / "refurb-shell" / "assets" / "fonts"

BG = (0x14, 0x16, 0x1B)
BG_DEEP = (0x08, 0x09, 0x0C)
INK = (0xEC, 0xEE, 0xF2)
SOFT = (0xA7, 0xAE, 0xBB)
CLAY = (0xE8, 0x83, 0x5C)
MINT = (0x35, 0xC2, 0xA3)


def font(name: str, size: int) -> ImageFont.FreeTypeFont:
    path = FONTS / name
    if path.is_file():
        return ImageFont.truetype(str(path), size)
    return ImageFont.load_default(size)


def gradient(width: int, height: int) -> Image.Image:
    """A vertical gradient, drawn one column-wide strip at a time."""
    img = Image.new("RGB", (width, height), BG)
    draw = ImageDraw.Draw(img)
    for y in range(height):
        t = y / max(height - 1, 1)
        # Ease the blend so the top stays flat and the bottom falls away.
        t = t * t * (3 - 2 * t)
        colour = tuple(round(a + (b - a) * t) for a, b in zip(BG, BG_DEEP))
        draw.line([(0, y), (width, y)], fill=colour)
    return img


def glow(img: Image.Image, centre: tuple[int, int], radius: int, colour, alpha: int) -> None:
    """A soft radial wash, made of concentric ellipses."""
    overlay = Image.new("RGBA", img.size, (0, 0, 0, 0))
    draw = ImageDraw.Draw(overlay)
    steps = 60
    for i in range(steps, 0, -1):
        r = int(radius * i / steps)
        a = int(alpha * (1 - i / steps) ** 1.5)
        draw.ellipse(
            [centre[0] - r, centre[1] - r, centre[0] + r, centre[1] + r],
            fill=(*colour, a),
        )
    img.alpha_composite(overlay)


def hairlines(img: Image.Image, width: int, height: int) -> None:
    """A faint grid: enough texture to read as designed, not as noise."""
    overlay = Image.new("RGBA", img.size, (0, 0, 0, 0))
    draw = ImageDraw.Draw(overlay)
    step = max(width // 24, 40)
    for x in range(0, width, step):
        draw.line([(x, 0), (x, height)], fill=(255, 255, 255, 6))
    for y in range(0, height, step):
        draw.line([(0, y), (width, y)], fill=(255, 255, 255, 4))
    img.alpha_composite(overlay)


def mark(draw: ImageDraw.ImageDraw, x: int, y: int, size: int) -> None:
    """The shell's brand mark: three stacked bars, the middle one mint."""
    bar = max(size // 7, 3)
    gap = bar
    draw.rounded_rectangle([x, y, x + size, y + bar], radius=bar // 2, fill=MINT)
    draw.rounded_rectangle(
        [x, y + bar + gap, x + size * 0.62, y + 2 * bar + gap],
        radius=bar // 2,
        fill=CLAY,
    )
    draw.rounded_rectangle(
        [x, y + 2 * (bar + gap), x + size * 0.82, y + 3 * bar + 2 * gap],
        radius=bar // 2,
        fill=SOFT,
    )


def background(width: int, height: int, wordmark: str = "left") -> Image.Image:
    img = gradient(width, height).convert("RGBA")
    glow(img, (int(width * 0.82), int(height * 0.72)), int(width * 0.42), MINT, 26)
    glow(img, (int(width * 0.12), int(height * 0.18)), int(width * 0.3), CLAY, 14)
    hairlines(img, width, height)
    draw = ImageDraw.Draw(img)

    # The BIOS menu (vesamenu) always prints its entries from the top-left
    # corner, so the isolinux backdrop puts the wordmark bottom-right where no
    # entry can overlap it. GRUB's themed menu is positioned by theme.txt and
    # keeps the wordmark at the top-left.
    if wordmark == "right":
        left = int(width * 0.56)
        top = int(height * 0.80)
    else:
        left = int(width * 0.08)
        top = int(height * 0.11)
    scale = width / 1920
    mark(draw, left, top, int(46 * scale))
    title = font("Inter-SemiBold.ttf", int(56 * scale))
    draw.text((left + int(72 * scale), top - int(6 * scale)), "REAWAKENING", font=title, fill=INK)
    w = draw.textlength("REAWAKENING", font=title)
    draw.text(
        (left + int(72 * scale) + w + int(18 * scale), top - int(6 * scale)),
        "OS",
        font=title,
        fill=MINT,
    )
    sub = font("Inter-Regular.ttf", int(24 * scale))
    draw.text(
        (left + int(72 * scale), top + int(64 * scale)),
        "LINUX  ·  0.3",
        font=sub,
        fill=SOFT,
    )
    draw.line(
        [(left, top + int(120 * scale)), (left + int(300 * scale), top + int(120 * scale))],
        fill=CLAY + (170,),
        width=max(int(2 * scale), 1),
    )
    if wordmark != "right":
        foot = font("Inter-Regular.ttf", int(18 * scale))
        draw.text(
            (left, height - int(64 * scale)),
            "Choose an entry with the arrow keys, then press Enter.",
            font=foot,
            fill=(SOFT[0], SOFT[1], SOFT[2]),
        )
    return img.convert("RGB")


def select_slices() -> dict[str, Image.Image]:
    """The nine-slice pixmap GRUB draws behind the selected boot entry.

    GRUB asks for select_nw/n/ne/w/c/e/sw/s/se.png; a rounded, translucent
    clay panel with a lighter left edge makes the selection unmistakable on a
    dark background without any animation.
    """
    edge = 10
    fill = (58, 35, 32, 255)
    border = (232, 131, 92, 255)
    corners = {"nw": (0, 0), "ne": (1, 0), "sw": (0, 1), "se": (1, 1)}
    out: dict[str, Image.Image] = {}
    for name, (right, bottom) in corners.items():
        img = Image.new("RGBA", (edge, edge), (0, 0, 0, 0))
        d = ImageDraw.Draw(img)
        d.rounded_rectangle([0, 0, edge * 2, edge * 2], radius=8, fill=fill)
        out[f"select_{name}.png"] = img
    for name in ("n", "s", "c"):
        img = Image.new("RGBA", (8, edge), (0, 0, 0, 0))
        ImageDraw.Draw(img).rectangle([0, 0, 8, edge], fill=fill)
        out[f"select_{name}.png"] = img
    for name in ("w", "e"):
        img = Image.new("RGBA", (8, 8), (0, 0, 0, 0))
        d = ImageDraw.Draw(img)
        d.rectangle([0, 0, 8, 8], fill=fill)
        # A clay stripe on the leading edge marks where the selection is.
        if name == "w":
            d.rectangle([0, 0, 2, 8], fill=border)
        out[f"select_{name}.png"] = img
    return out


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    background(1920, 1080).save(OUT / "background.png", optimize=True)
    background(1024, 768, wordmark="right").save(OUT / "isolinux.png", optimize=True)
    for name, img in select_slices().items():
        img.save(OUT / name, optimize=True)
    print(f"boot theme written to {OUT}")


if __name__ == "__main__":
    main()
