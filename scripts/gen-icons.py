"""Writes the input-mode icons: V for Vietnamese, E for English (bikey off).

Run with `python scripts/gen-icons.py`; the output is committed. Standard library
only, so the icons can be rebuilt anywhere.
"""
import pathlib
import struct

GLYPHS = {
    "V": ["#...#", "#...#", "#...#", "#...#", ".#.#.", ".#.#.", "..#.."],
    "E": ["#####", "#....", "#....", "####.", "#....", "#....", "#####"],
}
GOLD = (0x1E, 0x8E, 0xC4, 0xFF)  # BGRA
GREY = (0x70, 0x70, 0x70, 0xFF)
WHITE = (0xFF, 0xFF, 0xFF, 0xFF)


def pixels(glyph, size, background):
    scale = max(1, (size - 2) // 8)
    width, height = 5 * scale, 7 * scale
    left, top = (size - width) // 2, (size - height) // 2
    rows = []
    for y in range(size):
        row = []
        for x in range(size):
            gx, gy = (x - left) // scale, (y - top) // scale
            on = 0 <= x - left < width and 0 <= y - top < height and glyph[gy][gx] == "#"
            row.append(WHITE if on else background)
        rows.append(row)
    return rows


def image(glyph, size, background):
    rows = pixels(glyph, size, background)
    header = struct.pack("<IiiHHIIiiII", 40, size, size * 2, 1, 32, 0, 0, 0, 0, 0, 0)
    colour = b"".join(bytes(p) for row in reversed(rows) for p in row)
    mask = bytes(((size + 31) // 32) * 4 * size)  # all opaque; alpha is in the colour data
    return header + colour + mask


def icon(glyph, background):
    sizes = [16, 20, 24, 32, 48]
    images = [image(glyph, s, background) for s in sizes]
    out = struct.pack("<HHH", 0, 1, len(sizes))
    offset = 6 + 16 * len(sizes)
    for size, data in zip(sizes, images):
        out += struct.pack("<BBBBHHII", size, size, 0, 0, 1, 32, len(data), offset)
        offset += len(data)
    return out + b"".join(images)


res = pathlib.Path(__file__).resolve().parent.parent / "platforms" / "windows" / "res"
res.mkdir(exist_ok=True)
(res / "vi.ico").write_bytes(icon(GLYPHS["V"], GOLD))
(res / "en.ico").write_bytes(icon(GLYPHS["E"], GREY))
print("wrote", res)
