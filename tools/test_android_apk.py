"""Offline packaging regressions; no Android SDK or signing secrets required."""

import struct
import tempfile
import unittest
from pathlib import Path
from zipfile import ZipFile

from android_apk import ABIS, elf_sections, inspect_apk


def elf(wide=True, symbols=False):
    names = b"\0.shstrtab\0.text\0.symtab\0"
    data = bytearray(512)
    data[:6] = b"\x7fELF" + bytes([2 if wide else 1, 1])
    struct.pack_into("<Q" if wide else "<I", data, 40 if wide else 32, 128)
    size = 64 if wide else 40
    struct.pack_into("<HHH", data, 58 if wide else 46, size, 4 if symbols else 3, 1)
    fmt = "<IIQQQQIIQQ" if wide else "<IIIIIIIIII"
    struct.pack_into(fmt, data, 128 + size, 1, 3, 0, 0, 400, len(names), 0, 0, 1, 0)
    struct.pack_into(fmt, data, 128 + size * 2, 11, 1, 0, 0, 450, 10, 0, 0, 1, 0)
    if symbols:
        struct.pack_into(fmt, data, 128 + size * 3, 17, 2, 0, 0, 470, 24, 0, 0, 1, 0)
    data[400:400 + len(names)] = names
    return data


class ApkTests(unittest.TestCase):
    def test_elf32_and_elf64(self):
        for wide in (False, True):
            self.assertEqual(elf_sections(elf(wide))[".text"], 10)
            self.assertEqual(elf_sections(elf(wide, True))[".symtab"], 24)
        with self.assertRaises(ValueError):
            elf_sections(b"not an ELF file")

    def test_abi_and_stripping_contracts(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "app.apk"
            for abis in [ABIS, {"arm64-v8a"}]:
                for symbols in (False, True):
                    with ZipFile(path, "w") as archive:
                        for abi in abis:
                            archive.writestr(f"lib/{abi}/libskellyspeak_core.so", elf(symbols=symbols))
                        archive.writestr("classes.dex", b"fixture")
                    self.assertIn("classes.dex", inspect_apk(path, abis))
                    if symbols:
                        with self.assertRaisesRegex(ValueError, "static symbols"):
                            inspect_apk(path, abis, True)
                    else:
                        self.assertIn("APK:", inspect_apk(path, abis, True))
                    with self.assertRaisesRegex(ValueError, "expected ABIs"):
                        inspect_apk(path, {"x86"})

    def test_missing_app_library(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "app.apk"
            with ZipFile(path, "w") as archive:
                archive.writestr("lib/x86/other.so", elf())
            with self.assertRaisesRegex(ValueError, "missing app library"):
                inspect_apk(path, {"x86"})


if __name__ == "__main__":
    unittest.main()
