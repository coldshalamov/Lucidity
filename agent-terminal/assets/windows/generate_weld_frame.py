"""Generate the checked-in multi-plane Weld Frame ICO using only Python stdlib."""

from pathlib import Path
import struct


SIZES = (16, 24, 32, 48, 64, 256)
BACKGROUND = (0x0C, 0x0E, 0x11, 0xFF)
FOREGROUND = (0xE6, 0xEA, 0xF0, 0xFF)
RECTANGLES = (
    (2, 2, 5, 14),
    (5, 2, 12, 5),
    (5, 11, 12, 14),
    (7, 6, 9, 10),
    (11, 6, 14, 10),
)


def snap(coordinate: int, size: int) -> int:
    return (coordinate * size + 8) // 16


def dib_plane(size: int) -> bytes:
    pixels = [[BACKGROUND for _ in range(size)] for _ in range(size)]
    for left, top, right, bottom in RECTANGLES:
        for y in range(snap(top, size), snap(bottom, size)):
            for x in range(snap(left, size), snap(right, size)):
                pixels[y][x] = FOREGROUND

    bgra = bytearray()
    for row in reversed(pixels):
        for red, green, blue, alpha in row:
            bgra.extend((blue, green, red, alpha))

    mask_stride = ((size + 31) // 32) * 4
    mask = bytes(mask_stride * size)
    bitmap_header = struct.pack(
        "<IIIHHIIIIII",
        40,
        size,
        size * 2,
        1,
        32,
        0,
        len(bgra),
        0,
        0,
        0,
        0,
    )
    return bitmap_header + bgra + mask


def build_ico() -> bytes:
    planes = [(size, dib_plane(size)) for size in SIZES]
    header_size = 6 + 16 * len(planes)
    offset = header_size
    directory = bytearray(struct.pack("<HHH", 0, 1, len(planes)))
    payload = bytearray()
    for size, plane in planes:
        encoded_size = 0 if size == 256 else size
        directory.extend(
            struct.pack(
                "<BBBBHHII",
                encoded_size,
                encoded_size,
                0,
                0,
                1,
                32,
                len(plane),
                offset,
            )
        )
        payload.extend(plane)
        offset += len(plane)
    return bytes(directory + payload)


if __name__ == "__main__":
    destination = Path(__file__).with_name("weld-frame.ico")
    destination.write_bytes(build_ico())
    print(destination)
