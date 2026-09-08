#!/usr/bin/env python3
"""Build and validate a PNG-backed ICNS file without relying on iconutil."""

from __future__ import annotations

import struct
import sys
from pathlib import Path


ICNS_TYPES = {
    "icon_16x16.png": b"icp4",
    "icon_16x16@2x.png": b"ic11",
    "icon_32x32.png": b"icp5",
    "icon_32x32@2x.png": b"ic12",
    "icon_128x128.png": b"ic07",
    "icon_128x128@2x.png": b"ic13",
    "icon_256x256.png": b"ic08",
    "icon_256x256@2x.png": b"ic14",
    "icon_512x512.png": b"ic09",
    "icon_512x512@2x.png": b"ic10",
}

EXPECTED_SIZES = {
    "icon_16x16.png": (16, 16),
    "icon_16x16@2x.png": (32, 32),
    "icon_32x32.png": (32, 32),
    "icon_32x32@2x.png": (64, 64),
    "icon_128x128.png": (128, 128),
    "icon_128x128@2x.png": (256, 256),
    "icon_256x256.png": (256, 256),
    "icon_256x256@2x.png": (512, 512),
    "icon_512x512.png": (512, 512),
    "icon_512x512@2x.png": (1024, 1024),
}

PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def png_size(data: bytes) -> tuple[int, int]:
    if len(data) < 24 or data[:8] != PNG_SIGNATURE or data[12:16] != b"IHDR":
        raise ValueError("not a valid PNG with an IHDR header")
    return struct.unpack(">II", data[16:24])


def build(iconset: Path, output: Path) -> None:
    chunks: list[bytes] = []
    for name, chunk_type in ICNS_TYPES.items():
        path = iconset / name
        data = path.read_bytes()
        actual_size = png_size(data)
        if actual_size != EXPECTED_SIZES[name]:
            raise ValueError(
                f"{name} has size {actual_size}, expected {EXPECTED_SIZES[name]}"
            )
        chunks.append(chunk_type + struct.pack(">I", len(data) + 8) + data)

    body = b"".join(chunks)
    output.write_bytes(b"icns" + struct.pack(">I", len(body) + 8) + body)


def verify(path: Path) -> None:
    data = path.read_bytes()
    if len(data) < 8 or data[:4] != b"icns":
        raise ValueError("not an ICNS file")
    declared_size = struct.unpack(">I", data[4:8])[0]
    if declared_size != len(data):
        raise ValueError(
            f"ICNS declares {declared_size} bytes but contains {len(data)} bytes"
        )

    found: set[bytes] = set()
    offset = 8
    while offset < len(data):
        if offset + 8 > len(data):
            raise ValueError("truncated ICNS chunk header")
        chunk_type = data[offset : offset + 4]
        chunk_size = struct.unpack(">I", data[offset + 4 : offset + 8])[0]
        if chunk_size < 8 or offset + chunk_size > len(data):
            raise ValueError(f"invalid ICNS chunk size for {chunk_type!r}")
        found.add(chunk_type)
        offset += chunk_size

    missing = set(ICNS_TYPES.values()) - found
    if missing:
        names = ", ".join(sorted(item.decode("ascii") for item in missing))
        raise ValueError(f"ICNS is missing chunks: {names}")


def main() -> None:
    try:
        if len(sys.argv) == 3 and sys.argv[1] == "--verify":
            verify(Path(sys.argv[2]))
        elif len(sys.argv) == 3:
            build(Path(sys.argv[1]), Path(sys.argv[2]))
            verify(Path(sys.argv[2]))
        else:
            raise SystemExit(
                "usage: make_icns.py ICONSET OUTPUT | make_icns.py --verify ICNS"
            )
    except (OSError, ValueError) as error:
        raise SystemExit(f"ICNS error: {error}") from error


if __name__ == "__main__":
    main()
