"""Export Fob v1 icon: PNGs, ICO, preview.png, taskbar.png. Run from this dir."""
import io, cairosvg
from PIL import Image, ImageDraw, ImageFont

def svg(path, size):
    return Image.open(io.BytesIO(cairosvg.svg2png(url=path, output_width=size, output_height=size))).convert("RGBA")

def icon(size):
    if size <= 16: return svg("fob-16.svg", size)
    if size <= 24: return svg("fob-24.svg", size)
    if size <= 32: return svg("fob-32.svg", size)
    return svg("fob.svg", size)

for s in (1024, 512, 256, 128, 64, 32, 16):
    icon(s).save(f"fob-{s}.png")

ico_sizes = (16, 24, 32, 48, 64, 256)
imgs = [icon(s) for s in ico_sizes]
imgs[-1].save("fob.ico", format="ICO", sizes=[(s, s) for s in ico_sizes], append_images=imgs[:-1])

FONT = "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"
font = ImageFont.truetype(FONT, 14)
LIGHT, DARK = "#F3F3F3", "#202020"

# preview.png
row_sizes = (256, 128, 64, 48, 32, 24, 16)
pad, rowh = 24, 256 + 48
W = pad + sum(s + pad for s in row_sizes) + 2 * (128 + pad)
P = Image.new("RGB", (W, rowh * 2), LIGHT)
d = ImageDraw.Draw(P)
for r, bg in enumerate((LIGHT, DARK)):
    y0 = r * rowh
    d.rectangle([0, y0, W, y0 + rowh], fill=bg)
    fg = "#333" if bg == LIGHT else "#ccc"
    x = pad
    for s in row_sizes:
        im = icon(s); y = y0 + 16 + (256 - s) // 2
        P.paste(im, (x, y), im); d.text((x, y0 + 280), f"{s}", fill=fg, font=font); x += s + pad
    for s in (32, 16):
        im = icon(s).resize((128, 128), Image.NEAREST); y = y0 + 16 + 64
        P.paste(im, (x, y), im); d.text((x, y0 + 280), f"{s} x{128//s}", fill=fg, font=font); x += 128 + pad
P.save("preview.png")

# taskbar.png: native tray at 100% (16px) and 150% (24px), then 4x zoom of the same strip
def bar(bg, fg, s):
    h = 48 if s == 16 else 72
    b = Image.new("RGB", (360 if s == 16 else 540, h), bg)
    dd = ImageDraw.Draw(b)
    x = 12
    for i in range(3):  # placeholder neighbour tray glyphs
        dd.rounded_rectangle([x, (h - s) // 2, x + s - 1, (h + s) // 2 - 1], radius=s // 5, outline=fg, width=1 if s == 16 else 2)
        x += s + (12 if s == 16 else 18)
    im = icon(s); b.paste(im, (x, (h - s) // 2), im)
    dd.text((x + s + 16, h // 2 - 8), "12:34", fill=fg, font=ImageFont.truetype(FONT, 12 if s == 16 else 18))
    return b
bars = []
for s in (16, 24):
    for bg, fg in (("#202020", "#E6E6E6"), ("#EEEEEE", "#1A1A1A")):
        bars.append(bar(bg, fg, s))
zoom = [b.resize((b.width * 3, b.height * 3), Image.NEAREST) for b in bars[:2]]
Wt = max(z.width for z in zoom) + 16
Ht = sum(b.height for b in bars) + 16 * len(bars) + sum(z.height + 16 for z in zoom) + 40
T = Image.new("RGB", (Wt, Ht), "#808080")
y = 8
for b in bars: T.paste(b, (8, y)); y += b.height + 16
ImageDraw.Draw(T).text((8, y), "16px tray, 3x nearest-neighbour zoom (dark / light):", fill="#fff", font=font); y += 24
for z in zoom: T.paste(z, (8, y)); y += z.height + 16
T.save("taskbar.png")

ico = Image.open("fob.ico")
print("ICO sizes:", sorted(ico.info.get("sizes", ico.ico.sizes())))
for s in ico_sizes:
    ico.size = (s, s); ico.load(); print(s, ico.size, ico.mode)

ico.size = (16, 16); ico.load()
print("ico16 == fob-16.png:", ico.convert("RGBA").tobytes() == Image.open("fob-16.png").convert("RGBA").tobytes())
