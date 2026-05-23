#!/usr/bin/env python3
"""
Generate a minimal valid Matroska (.mkv) file with a given title
and a single video track.  The file is valid enough for the `matroska`
Rust crate to open and extract { info.title, video_tracks[].pixel_* }.
"""

import struct
import sys


# ── EBML VINT helpers ──────────────────────────────────────────────

def vint_width_from_encoded(encoded_val: int) -> int:
    """Determine the byte width of a *pre-encoded* VINT by inspecting
    the leading bits of its most significant byte."""
    msb = encoded_val.to_bytes((encoded_val.bit_length() + 7) // 8, "big")[0]
    if msb >= 0x80:
        return 1
    if msb >= 0x40:
        return 2
    if msb >= 0x20:
        return 3
    if msb >= 0x10:
        return 4
    raise ValueError(
        f"Cannot determine VINT width for {encoded_val:#x} (MSB={msb:#x})"
    )


def encode_vint(value: int, width: int = None) -> bytes:
    """Encode *value* as a VINT of *width* bytes (auto if None)."""
    if width is None:
        for w, mask in [(1, 0x80), (2, 0x4000), (3, 0x200000), (4, 0x10000000)]:
            if value < mask:
                width = w
                break
        else:
            raise ValueError(f"Value {value} too large for 4-byte VINT")
    masks = {1: 0x80, 2: 0x4000, 3: 0x200000, 4: 0x10000000}
    return (value | masks[width]).to_bytes(width, "big")


def encode_element(elem_id_encoded: int, data: bytes) -> bytes:
    """EBML element: raw pre-encoded VINT id + VINT(data_len) + data."""
    id_bytes = elem_id_encoded.to_bytes(
        vint_width_from_encoded(elem_id_encoded), "big"
    )
    return id_bytes + encode_vint(len(data)) + data


def encode_uint(value: int, width: int = None) -> bytes:
    """Encode an unsigned integer (big-endian, minimal width unless given)."""
    if width is None:
        width = max(1, (value.bit_length() + 7) // 8)
    return value.to_bytes(width, "big")


def encode_float(value: float) -> bytes:
    return struct.pack(">d", value)


def encode_string(value: str) -> bytes:
    return value.encode("utf-8")


# ── Known EBML/Matroska element IDs ──────────────────────────────
# These are the *pre-encoded* VINT forms as listed in the spec.

EBML             = 0x1A45DFA3
EBML_VERSION     = 0x4286
EBML_READVER     = 0x42F7
EBML_MAXIDLEN    = 0x42F2
EBML_MAXSIZELEN  = 0x42F3
DOCTYPE          = 0x4282
DOCTYPE_VER      = 0x4287
DOCTYPE_READVER  = 0x4285

SEGMENT          = 0x18538067

INFO             = 0x1549A966
TIMECODE_SCALE   = 0x2AD7B1
DURATION         = 0x4489
TITLE            = 0x7BA9
MUXING_APP       = 0x4D80
WRITING_APP      = 0x5741

TRACKS           = 0x1654AE6B
TRACK_ENTRY      = 0xAE
TRACK_NUMBER     = 0xD7
TRACK_UID        = 0x73C5
TRACK_TYPE       = 0x83
FLAG_DEFAULT     = 0x88
CODEC_ID         = 0x86
VIDEO            = 0xE0
PIXEL_WIDTH      = 0xB0
PIXEL_HEIGHT     = 0xBA
DEFAULT_DURATION = 0x23E383


# ── Build a minimal but valid MKV ─────────────────────────────────

def build_mkv(title: str, width: int = 1920, height: int = 1080) -> bytes:
    ebml_header = encode_element(EBML, b"".join([
        encode_element(EBML_VERSION, encode_uint(1)),
        encode_element(EBML_READVER, encode_uint(1)),
        encode_element(EBML_MAXIDLEN, encode_uint(4)),
        encode_element(EBML_MAXSIZELEN, encode_uint(8)),
        encode_element(DOCTYPE, encode_string("matroska")),
        encode_element(DOCTYPE_VER, encode_uint(4)),
        encode_element(DOCTYPE_READVER, encode_uint(2)),
    ]))

    info = encode_element(INFO, b"".join([
        encode_element(TIMECODE_SCALE, encode_uint(1_000_000)),
        encode_element(DURATION, encode_float(60_000.0)),
        encode_element(TITLE, encode_string(title)),
        encode_element(MUXING_APP, encode_string("mkvgen.py")),
        encode_element(WRITING_APP, encode_string("mkvgen.py")),
    ]))

    video_track = encode_element(TRACK_ENTRY, b"".join([
        encode_element(TRACK_NUMBER, encode_uint(1)),
        encode_element(TRACK_UID, encode_uint(1)),
        encode_element(TRACK_TYPE, encode_uint(1)),
        encode_element(FLAG_DEFAULT, encode_uint(1)),
        encode_element(CODEC_ID, encode_string("V_MPEG4/ISO/AVC")),
        encode_element(DEFAULT_DURATION, encode_uint(41_666_666, 4)),
        encode_element(VIDEO, b"".join([
            encode_element(PIXEL_WIDTH, encode_uint(width, 2)),
            encode_element(PIXEL_HEIGHT, encode_uint(height, 2)),
        ])),
    ]))

    tracks = encode_element(TRACKS, video_track)
    segment = encode_element(SEGMENT, info + tracks)

    return ebml_header + segment


# ── CLI ──────────────────────────────────────────────────────────

def main():
    if len(sys.argv) < 3:
        print(f"Usage: {sys.argv[0]} <output.mkv> <title> [width] [height]",
              file=sys.stderr)
        sys.exit(1)

    out_path = sys.argv[1]
    title = sys.argv[2]
    width = int(sys.argv[3]) if len(sys.argv) > 3 else 1920
    height = int(sys.argv[4]) if len(sys.argv) > 4 else 1080

    data = build_mkv(title, width, height)
    with open(out_path, "wb") as f:
        f.write(data)
    print(f"Created {out_path}: title={title!r} {width}x{height} ({len(data)} bytes)")


if __name__ == "__main__":
    main()
