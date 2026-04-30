"""
NaxOne logo processor.

Input:  logo.png (root) — blue/cyan N on a black noisy background.
Output: crates/<tauri-crate>/icons/{32x32.png,128x128.png,128x128@2x.png,
        icon.ico (multi-size embedded), icon.icns (best-effort PNG-pack)}
        plus a clean transparent PNG at logo_transparent.png in repo root.

Strategy for background removal:
  - Pixels are classified by HSV saturation + blue dominance, not by brightness alone,
    because the black background has bright-white noise speckles that a simple
    luminance threshold would keep.
  - Logo pixels: blue/cyan, fairly saturated.
  - Background pixels: dark (black) OR low-saturation bright (white speckles).
"""

from __future__ import annotations

import io
import os
import struct
import sys
from pathlib import Path

from PIL import Image, ImageFilter

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "logo.png"
ICONS_DIR = ROOT / "crates" / "naxone-tauri" / "icons"
TRANSPARENT_OUT = ROOT / "logo_transparent.png"


def remove_background(im: Image.Image) -> Image.Image:
    """Replace black-noisy background with transparency. Returns RGBA."""
    rgba = im.convert("RGBA")
    px = rgba.load()
    w, h = rgba.size
    for y in range(h):
        for x in range(w):
            r, g, b, _ = px[x, y]
            mx = max(r, g, b)
            mn = min(r, g, b)
            sat = (mx - mn) / mx if mx else 0
            # Background rule:
            #   dark pixel             OR  bright but desaturated (white noise)
            if mx < 50 or (mx > 200 and sat < 0.15):
                px[x, y] = (0, 0, 0, 0)
                continue
            # Mild edge bleed cleanup: low-blue + low-sat → background too
            if sat < 0.25 and b < 80:
                px[x, y] = (0, 0, 0, 0)
                continue
    return rgba


def trim(im: Image.Image) -> Image.Image:
    bbox = im.getbbox()
    return im.crop(bbox) if bbox else im


def pad_to_square(im: Image.Image, ratio: float = 0.85) -> Image.Image:
    """Center the (already-trimmed) image on a transparent square canvas.
    `ratio` controls inner padding — 0.85 means logo occupies 85% of the side.
    """
    w, h = im.size
    side = max(w, h)
    canvas_side = int(side / ratio)
    canvas = Image.new("RGBA", (canvas_side, canvas_side), (0, 0, 0, 0))
    ox = (canvas_side - w) // 2
    oy = (canvas_side - h) // 2
    canvas.paste(im, (ox, oy), im)
    return canvas


def smooth_alpha(im: Image.Image) -> Image.Image:
    """Light blur on the alpha channel to soften jagged edges from threshold."""
    r, g, b, a = im.split()
    a = a.filter(ImageFilter.GaussianBlur(radius=0.6))
    return Image.merge("RGBA", (r, g, b, a))


def resize(im: Image.Image, size: int) -> Image.Image:
    return im.resize((size, size), Image.LANCZOS)


def write_pngs(master: Image.Image) -> None:
    ICONS_DIR.mkdir(parents=True, exist_ok=True)
    targets = {
        "32x32.png": 32,
        "128x128.png": 128,
        "128x128@2x.png": 256,
    }
    for name, size in targets.items():
        resize(master, size).save(ICONS_DIR / name, "PNG", optimize=True)
        print(f"  wrote {name}")


def write_ico(master: Image.Image) -> None:
    sizes = [16, 32, 48, 64, 128, 256]
    imgs = [resize(master, s) for s in sizes]
    out = ICONS_DIR / "icon.ico"
    imgs[0].save(out, format="ICO", sizes=[(s, s) for s in sizes], append_images=imgs[1:])
    print(f"  wrote {out.name} (sizes={sizes})")


def write_icns(master: Image.Image) -> None:
    """Write a basic .icns with PNG-encoded entries.
    PIL's ICNS writer covers what we need for Tauri bundle on macOS."""
    out = ICONS_DIR / "icon.icns"
    # PIL needs a list of square sizes; it will pick supported ones.
    sizes = [(16, 16), (32, 32), (64, 64), (128, 128), (256, 256), (512, 512), (1024, 1024)]
    try:
        master.save(out, format="ICNS", sizes=sizes)
        print(f"  wrote {out.name}")
    except Exception as e:
        # Fallback: write a 512 PNG as .icns is non-critical on Windows-only build
        print(f"  ICNS write failed ({e}), writing 512 PNG fallback")
        resize(master, 512).save(out, "PNG")


def main() -> int:
    if not SRC.exists():
        print(f"ERROR: {SRC} not found", file=sys.stderr)
        return 1
    print(f"[1/5] load {SRC.name} ({SRC.stat().st_size} bytes)")
    raw = Image.open(SRC)
    print(f"      input size = {raw.size}")

    print("[2/5] remove background (HSV-based)")
    rgba = remove_background(raw)

    print("[3/5] trim + pad to square + smooth")
    trimmed = trim(rgba)
    print(f"      content bbox size = {trimmed.size}")
    squared = pad_to_square(trimmed, ratio=0.82)
    smoothed = smooth_alpha(squared)
    smoothed.save(TRANSPARENT_OUT, "PNG", optimize=True)
    print(f"      saved {TRANSPARENT_OUT}")

    print("[4/5] resample to PNG icon set")
    # Use 1024 as the master so all downsamples are clean
    master = resize(smoothed, 1024)
    write_pngs(master)

    print("[5/5] generate ICO + ICNS")
    write_ico(master)
    write_icns(master)

    print("\nDone.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
