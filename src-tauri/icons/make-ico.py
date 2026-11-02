"""Build icon.ico (the exe icon and every runtime window icon) from the
pixel-hinted SVGs in svg/ (copied from the Fob v1 design; the design folder
itself is never written). Every frame is rendered from SVG at its own size,
never downscaled from 256: the 16/24/32 hinted drawings for small frames, the
full drawing above 32 (same rule as the design's export.py).

Sizes: every size Windows 10/11 asks for at 100-250 % scaling (taskbar,
title bar, Alt-Tab, Start, Explorer). scripts/check-windows-exe.py requires
exactly this list in fob.exe.  Run: python3 src-tauri/icons/make-ico.py
"""
import io, os
import cairosvg
from PIL import Image

SIZES = (16, 20, 24, 30, 32, 36, 40, 48, 60, 64, 72, 80, 96, 128, 256)
HERE = os.path.dirname(os.path.abspath(__file__))


def render(size):
    name = "fob-16.svg" if size <= 16 else "fob-24.svg" if size <= 24 else "fob-32.svg" if size <= 32 else "fob.svg"
    png = cairosvg.svg2png(url=os.path.join(HERE, "svg", name), output_width=size, output_height=size)
    return Image.open(io.BytesIO(png)).convert("RGBA")


frames = [render(s) for s in SIZES]
# Largest first: Pillow builds the file from the first image plus the exact
# append_images for each size.
frames[-1].save(
    os.path.join(HERE, "icon.ico"),
    format="ICO",
    sizes=[(s, s) for s in SIZES],
    append_images=frames[:-1],
)
