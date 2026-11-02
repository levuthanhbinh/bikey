"""Writes every bikey icon: the logo (E and V sharing a tile) and the V and E
input-mode buttons, which reuse its two halves.

Run with `python scripts/gen-icons.py`; the output is committed. Standard library
only, so the icons can be rebuilt anywhere. Shapes are in a 256-unit square. Sizes
of 24 px and under get their own heavier shapes, because the large ones blur.
"""
import math
import pathlib
import struct
import zlib

NAVY = (0x16, 0x32, 0x4F)
GOLD = (0xF2, 0xA9, 0x00)
WHITE = (0xFF, 0xFF, 0xFF)
SAMPLES = 6  # per pixel, per axis

LOGO_E = [(108, 44), (52, 44), (52, 124), (108, 124)]
LOGO_E_ARM = [(52, 84), (96, 84)]
LOGO_V = [(148, 132), (176, 212), (204, 132)]
LOGO_STROKE = 22
# Small logos are drawn in pixels, so the E lands on whole rows and columns:
# {size: (e stem box, e arms as (top, left, right), arm height, v points, v stroke)}.
SMALL_LOGO = {
    16: ((2, 2, 4, 10), [(2, 2, 7), (5, 2, 6), (8, 2, 7)], 2, [(9.75, 8.5), (12, 13.5), (14.25, 8.5)], 1.7),
    20: ((3, 2, 5, 12), [(2, 3, 8), (6, 3, 7), (10, 3, 8)], 2, [(12, 11), (15, 17), (18, 11)], 2),
    24: ((3, 3, 5, 15), [(3, 3, 10), (8, 3, 9), (13, 3, 10)], 2, [(14.5, 13), (18, 20.5), (21.5, 13)], 2.5),
}
MODE_V = [(72, 60), (128, 196), (184, 60)]
MODE_E = [(176, 56), (84, 56), (84, 200), (176, 200)]
MODE_E_ARM = [(84, 128), (164, 128)]
MODE_STROKE = 36
# {size: (e stem box, e arms as (top, left, right), arm height)}, in pixels as above.
SMALL_MODE_E = {
    16: ((4, 3, 6, 13), [(3, 4, 12), (7, 4, 11), (11, 4, 12)], 2),
    20: ((5, 4, 7, 16), [(4, 5, 15), (9, 5, 14), (14, 5, 15)], 2),
    24: ((6, 5, 9, 20), [(5, 6, 18), (11, 6, 17), (17, 6, 18)], 3),
}


def segment_distance(x, y, a, b):
    dx, dy = b[0] - a[0], b[1] - a[1]
    t = max(0, min(1, ((x - a[0]) * dx + (y - a[1]) * dy) / (dx * dx + dy * dy)))
    return math.hypot(x - a[0] - t * dx, y - a[1] - t * dy)


def stroke(points, width):
    def inside(x, y):
        return any(
            segment_distance(x, y, a, b) <= width / 2 for a, b in zip(points, points[1:])
        )

    return inside


def box(left, top, right, bottom):
    return lambda x, y: left <= x < right and top <= y < bottom


def tile(x, y, radius=56):
    dx = max(radius - x, x - (256 - radius), 0)
    dy = max(radius - y, y - (256 - radius), 0)
    return 0 <= x <= 256 and 0 <= y <= 256 and dx * dx + dy * dy <= radius * radius


def lower_right(x, y):
    return tile(x, y) and x + y >= 256


def scaled(shape, factor):
    return lambda x, y: shape(x / factor, y / factor)


def pixel_e(size, stem, arms, arm_height):
    factor = 256 / size
    boxes = [box(*stem)] + [box(left, top, right, top + arm_height) for top, left, right in arms]
    return [(scaled(b, factor), WHITE) for b in boxes]


def logo_layers(size):
    layers = [(tile, NAVY), (lower_right, GOLD)]
    if size in SMALL_LOGO:
        stem, arms, arm_height, v, v_stroke = SMALL_LOGO[size]
        layers += pixel_e(size, stem, arms, arm_height)
        layers.append((scaled(stroke(v, v_stroke), 256 / size), NAVY))
        return layers
    layers.append((stroke(LOGO_E, LOGO_STROKE), WHITE))
    layers.append((stroke(LOGO_E_ARM, LOGO_STROKE), WHITE))
    layers.append((stroke(LOGO_V, LOGO_STROKE), NAVY))
    return layers


def vi_layers(size):
    return [(tile, GOLD), (stroke(MODE_V, MODE_STROKE), NAVY)]


def en_layers(size):
    if size in SMALL_MODE_E:
        return [(tile, NAVY)] + pixel_e(size, *SMALL_MODE_E[size])
    return [
        (tile, NAVY),
        (stroke(MODE_E, MODE_STROKE), WHITE),
        (stroke(MODE_E_ARM, MODE_STROKE), WHITE),
    ]


def render(layers, size):
    """Rows of (r, g, b, a) pixels, top first."""
    unit = 256 / size
    offsets = [(i + 0.5) / SAMPLES for i in range(SAMPLES)]
    rows = []
    for py in range(size):
        row = []
        for px in range(size):
            total = [0, 0, 0]
            covered = 0
            for sy in offsets:
                for sx in offsets:
                    x, y = (px + sx) * unit, (py + sy) * unit
                    colour = None
                    for shape, fill in layers:
                        if shape(x, y):
                            colour = fill
                    if colour:
                        covered += 1
                        for i in range(3):
                            total[i] += colour[i]
            if covered:
                rgb = tuple(round(t / covered) for t in total)
                row.append(rgb + (round(covered * 255 / SAMPLES**2),))
            else:
                row.append((0, 0, 0, 0))
        rows.append(row)
    return rows


def png(rows):
    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

    size = len(rows)
    raw = b"".join(b"\0" + b"".join(bytes(p) for p in row) for row in rows)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


def dib(rows):
    size = len(rows)
    header = struct.pack("<IiiHHIIiiII", 40, size, size * 2, 1, 32, 0, 0, 0, 0, 0, 0)
    colour = b"".join(bytes((b, g, r, a)) for row in reversed(rows) for r, g, b, a in row)
    mask = bytes(((size + 31) // 32) * 4 * size)  # all opaque; alpha is in the colour data
    return header + colour + mask


def icon(layers_for, sizes):
    # The 256 px image is stored as PNG, the smaller ones as bitmaps.
    images = [(s, (png if s == 256 else dib)(render(layers_for(s), s))) for s in sizes]
    out = struct.pack("<HHH", 0, 1, len(images))
    offset = 6 + 16 * len(images)
    for size, data in images:
        out += struct.pack("<BBBBHHII", size % 256, size % 256, 0, 0, 1, 32, len(data), offset)
        offset += len(data)
    return out + b"".join(data for _, data in images)


def bitmap_on_white(rows):
    """A 24-bit BMP, for the installer's header image."""
    size = len(rows)
    stride = (size * 3 + 3) // 4 * 4
    body = b""
    for row in reversed(rows):
        line = b""
        for r, g, b, a in row:
            line += bytes((c * a + 255 * (255 - a)) // 255 for c in (b, g, r))
        body += line + bytes(stride - len(line))
    info = struct.pack("<IiiHHIIiiII", 40, size, size, 1, 24, 0, len(body), 2835, 2835, 0, 0)
    return b"BM" + struct.pack("<IHHI", 54 + len(body), 0, 0, 54) + info + body


def svg():
    def path(points):
        return "M" + "L".join(f"{x} {y}" for x, y in points)

    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" role="img" aria-label="bikey logo: E and V sharing one tile">
  <clipPath id="tile"><rect width="256" height="256" rx="56"/></clipPath>
  <g clip-path="url(#tile)">
    <rect width="256" height="256" fill="#16324F"/>
    <path d="M256 0V256H0Z" fill="#F2A900"/>
  </g>
  <g fill="none" stroke-width="{LOGO_STROKE}" stroke-linecap="round" stroke-linejoin="round">
    <path d="{path(LOGO_E)}{path(LOGO_E_ARM)}" stroke="#FFFFFF"/>
    <path d="{path(LOGO_V)}" stroke="#16324F"/>
  </g>
</svg>
"""


root = pathlib.Path(__file__).resolve().parent.parent
res = root / "platforms" / "windows" / "res"
res.mkdir(exist_ok=True)
(res / "bikey.ico").write_bytes(icon(logo_layers, [16, 20, 24, 32, 40, 48, 64, 256]))
(res / "vi.ico").write_bytes(icon(vi_layers, [16, 20, 24, 32, 48]))
(res / "en.ico").write_bytes(icon(en_layers, [16, 20, 24, 32, 48]))
for size in (58, 116):
    (res / f"wizard-{size}.bmp").write_bytes(bitmap_on_white(render(logo_layers(size), size)))
assets = root / "assets"
assets.mkdir(exist_ok=True)
(assets / "bikey-logo.svg").write_text(svg(), encoding="utf-8", newline="\n")
print("wrote", res, "and", assets)
