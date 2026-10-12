"""Inspect APK payload sizes and enforce the release's native ABI contract (stdlib only)."""

import argparse
import struct
from pathlib import Path
from zipfile import ZipFile

ABIS = {"arm64-v8a", "armeabi-v7a", "x86", "x86_64"}


def elf_sections(data):
    """Read section sizes from the little-endian ELF32/ELF64 Android libraries."""
    if data[:4] != b"\x7fELF" or data[5] != 1 or data[4] not in (1, 2):
        raise ValueError("Expected a little-endian Android ELF library")
    wide = data[4] == 2
    offset = struct.unpack_from("<Q" if wide else "<I", data, 40 if wide else 32)[0]
    size, count, names_index = struct.unpack_from("<HHH", data, 58 if wide else 46)
    headers = [struct.unpack_from("<IIQQQQIIQQ" if wide else "<IIIIIIIIII", data, offset + i * size)
               for i in range(count)]
    names_header = headers[names_index]
    names = data[names_header[4]:names_header[4] + names_header[5]]
    return {names[h[0]:].split(b"\0", 1)[0].decode(): h[5] for h in headers if h[1] != 8}


def inspect_apk(path, expected, require_stripped=False):
    with ZipFile(path) as archive:
        entries = archive.infolist()
        libraries = [entry for entry in entries if entry.filename.startswith("lib/") and entry.filename.endswith(".so")]
        actual = {entry.filename.split("/")[1] for entry in libraries}
        if actual != expected:
            raise ValueError(f"{path}: expected ABIs {sorted(expected)}, found {sorted(actual)}")
        for abi in expected:
            if f"lib/{abi}/libskellyspeak_core.so" not in archive.namelist():
                raise ValueError(f"{path}: missing app library for {abi}")
        rows = []
        for entry in libraries:
            sections = elf_sections(archive.read(entry))
            symbols = sum(n for name, n in sections.items() if name in (".symtab", ".strtab") or name.startswith((".debug", ".zdebug")))
            if require_stripped and symbols:
                raise ValueError(f"{path}: {entry.filename} contains {symbols} bytes of static symbols/debug data")
            rows.append((entry.filename, entry.compress_size, entry.file_size, symbols))
        native_bytes = sum(entry.compress_size for entry in libraries)
        total = Path(path).stat().st_size
        report = [f"### {Path(path).name}", "", f"APK: {total:,} bytes ({total / 1048576:.2f} MiB).",
                  "", "| Native library | Stored bytes | Expanded bytes | Static symbols/debug bytes |",
                  "| --- | ---: | ---: | ---: |"]
        report.extend(f"| {name} | {stored:,} | {expanded:,} | {symbols:,} |" for name, stored, expanded, symbols in rows)
        report.extend(["", f"Other entries, ZIP alignment and signing overhead: {total - native_bytes:,} bytes.", "",
                       "Largest non-native entries (stored bytes):", ""])
        report.extend(f"- `{entry.filename}`: {entry.compress_size:,}" for entry in sorted(
            (entry for entry in entries if entry not in libraries), key=lambda entry: entry.compress_size, reverse=True)[:10])
        return "\n".join(report) + "\n"


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("apk", type=Path)
    parser.add_argument("abi", choices=["universal", *sorted(ABIS)])
    parser.add_argument("--require-stripped", action="store_true")
    args = parser.parse_args()
    print(inspect_apk(args.apk, ABIS if args.abi == "universal" else {args.abi}, args.require_stripped))
