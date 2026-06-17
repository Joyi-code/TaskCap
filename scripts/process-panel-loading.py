"""将 panel-loading.gif 转为透明青绿 PNG，供主面板加载态使用。"""
from __future__ import annotations

from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "public" / "panel-loading.gif"
OUT = ROOT / "public" / "panel-loading.png"
ACCENT = (20, 184, 166)
BG_THRESHOLD = 40


def main() -> None:
    im = Image.open(SRC).convert("RGBA")
    px = im.load()
    w, h = im.size
    for y in range(h):
        for x in range(w):
            r, g, b, _a = px[x, y]
            if r + g + b < BG_THRESHOLD:
                px[x, y] = (0, 0, 0, 0)
                continue
            strength = max(r, g, b) / 255.0
            alpha = int(min(255, max(72, strength * 255)))
            px[x, y] = (*ACCENT, alpha)

    bbox = im.getbbox()
    if bbox:
        pad = 8
        l, t, r, b = bbox
        im = im.crop(
            (
                max(0, l - pad),
                max(0, t - pad),
                min(w, r + pad),
                min(h, b + pad),
            )
        )

    im.save(OUT, "PNG")
    print(f"wrote {OUT} ({im.size[0]}x{im.size[1]})")


if __name__ == "__main__":
    main()