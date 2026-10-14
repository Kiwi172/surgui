"""Merge staged Tauri ICOs into the final Lunarz Surge icon.

Takes small-source ICO (entries <=48) and large-source ICO (entries >=64)
and writes a 6-entry PNG-backed ICO with recomputed offsets.

Usage:
    python3 build-ico.py <small.ico> <large.ico> <output.ico>

Pure stdlib, no network. Fails loudly instead of shipping a mismatched ICO.
"""
import struct
import sys
from pathlib import Path

PNG_SIG = b"\x89PNG\r\n\x1a\n"
EXPECTED_SIZES = (16, 24, 32, 48, 64, 256)
HEADER_FMT = "<HHH"
ENTRY_FMT = "<BBBBHHII"
HEADER_SIZE = struct.calcsize(HEADER_FMT)
ENTRY_SIZE = struct.calcsize(ENTRY_FMT)


def parse_ico(path: Path):
    data = path.read_bytes()
    if len(data) < HEADER_SIZE:
        raise ValueError(f"{path}: too short for ICO header")
    reserved, ico_type, count = struct.unpack_from(HEADER_FMT, data, 0)
    if reserved != 0:
        raise ValueError(f"{path}: bad reserved={reserved}, want 0")
    if ico_type != 1:
        raise ValueError(f"{path}: bad type={ico_type}, want 1 (ICO)")
    if count != 6:
        raise ValueError(f"{path}: bad count={count}, want exactly 6")
    entries = []
    for i in range(count):
        off = HEADER_SIZE + i * ENTRY_SIZE
        w, h, colors, res, planes, bpp, size, img_off = struct.unpack_from(
            ENTRY_FMT, data, off
        )
        width = w if w != 0 else 256
        height = h if h != 0 else 256
        if width != height:
            raise ValueError(f"{path} entry {i}: non-square {width}x{height}")
        if width not in EXPECTED_SIZES:
            raise ValueError(f"{path} entry {i}: unexpected size {width}")
        if bpp != 32:
            raise ValueError(f"{path} entry {i}: bad bpp={bpp}, want 32")
        if img_off + size > len(data):
            raise ValueError(f"{path} entry {i}: corrupt offset/size")
        payload = data[img_off : img_off + size]
        if payload[:8] != PNG_SIG:
            raise ValueError(f"{path} entry {i}: payload is not PNG-backed")
        if len(payload) < 24:
            raise ValueError(f"{path} entry {i}: PNG payload truncated")
        iw, ih = struct.unpack(">II", payload[16:24])
        if (iw, ih) != (width, height):
            raise ValueError(
                f"{path} entry {i}: IHDR {iw}x{ih} != ICO {width}x{height}"
            )
        entries.append(
            {
                "size": width,
                "header8": data[off : off + 8],
                "payload": payload,
            }
        )
    sizes = tuple(sorted(e["size"] for e in entries))
    if sizes != EXPECTED_SIZES:
        raise ValueError(f"{path}: sizes {sizes} != {EXPECTED_SIZES}")
    return entries


def main(argv):
    if len(argv) != 4:
        print(__doc__, file=sys.stderr)
        return 2
    small_p = Path(argv[1])
    large_p = Path(argv[2])
    out_p = Path(argv[3])
    for p in (small_p, large_p):
        if not p.is_file():
            print(f"error: input not a file: {p}", file=sys.stderr)
            return 2
        if p.suffix.lower() != ".ico":
            print(f"error: input must be .ico: {p}", file=sys.stderr)
            return 2
    if out_p.suffix.lower() != ".ico":
        print(f"error: output must be .ico: {out_p}", file=sys.stderr)
        return 2
    if small_p.resolve() == large_p.resolve():
        print("error: small and large inputs are the same file", file=sys.stderr)
        return 2
    if out_p.resolve() in (small_p.resolve(), large_p.resolve()):
        print("error: output must not overwrite an input", file=sys.stderr)
        return 2

    small_entries = parse_ico(small_p)
    large_entries = parse_ico(large_p)
    by_size_small = {e["size"]: e for e in small_entries}
    by_size_large = {e["size"]: e for e in large_entries}

    picked = []
    for s in EXPECTED_SIZES:
        src = by_size_small if s <= 48 else by_size_large
        picked.append((s, src[s]))

    out = bytearray()
    out += struct.pack(HEADER_FMT, 0, 1, len(picked))
    data_off = HEADER_SIZE + ENTRY_SIZE * len(picked)
    for size, entry in picked:
        payload = entry["payload"]
        out += entry["header8"] + struct.pack("<II", len(payload), data_off)
        data_off += len(payload)
    for _, entry in picked:
        out += entry["payload"]

    out_p.parent.mkdir(parents=True, exist_ok=True)
    out_p.write_bytes(bytes(out))
    origin = "small" if picked[0][0] <= 48 else "?"
    detail = ", ".join(
        f"{s}={'small' if s <= 48 else 'large'}:{len(e['payload'])}B"
        for s, e in picked
    )
    print(f"wrote {out_p} ({len(out)}B) [{detail}]")
    _ = origin
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
