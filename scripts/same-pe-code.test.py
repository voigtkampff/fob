#!/usr/bin/env python3
"""Checks same-pe-code.py on a small PE32+ built here, so it runs anywhere.

usage: python3 scripts/same-pe-code.test.py   (needs python3-pefile)
"""
import importlib.util
import pathlib
import struct
import sys
import unittest

spec = importlib.util.spec_from_file_location(
    "same_pe_code", pathlib.Path(__file__).with_name("same-pe-code.py")
)
same = importlib.util.module_from_spec(spec)
spec.loader.exec_module(same)

TEXT, RDATA = 0x200, 0x400  # file offsets; RVAs are 0x1000 and 0x2000
DESCRIPTOR, ILT, IAT, HINT_NAME, DLL_NAME, DEBUG, RSDS = (
    0x00, 0x40, 0x60, 0x80, 0xA0, 0xC0, 0xE0
)  # offsets inside .rdata
ORDINAL = 1 << 63


def pe(
    link_time=1,
    debug_time=1,
    pdb_id=b"\x11" * 16,
    hint=20,
    code=b"\x48\x31\xc0\xc3",
    name=b"GetTickCount",
    dll=b"KERNEL32.dll",
    ordinal=7,
    dll_characteristics=0x8160,
    hint_name_in_text=False,
):
    out = bytearray(0x600)
    # Filler keeps pefile from flagging a mostly-zero file as truncated.
    out[TEXT:] = bytes((i * 7 + 3) % 256 for i in range(len(out) - TEXT))
    out[RDATA : RDATA + 0x100] = bytes(0x100)
    out[0:2] = b"MZ"
    struct.pack_into("<I", out, 0x3C, 0x40)
    out[0x40:0x44] = b"PE\0\0"
    struct.pack_into("<HHIIIHH", out, 0x44, 0x8664, 2, link_time, 0, 0, 240, 0x22)
    opt = 0x58
    struct.pack_into("<HBBIIIII", out, opt, 0x20B, 14, 0, 0x200, 0x200, 0, 0x1000, 0x1000)
    struct.pack_into("<QII", out, opt + 24, 0x140000000, 0x1000, 0x200)
    struct.pack_into("<HHHHHH", out, opt + 40, 6, 0, 0, 0, 6, 0)
    struct.pack_into("<IIII", out, opt + 52, 0, 0x3000, 0x200, 0)
    struct.pack_into("<HH", out, opt + 68, 3, dll_characteristics)
    struct.pack_into("<QQQQII", out, opt + 72, 0x100000, 0x1000, 0x100000, 0x1000, 0, 16)
    dirs = opt + 112
    struct.pack_into("<II", out, dirs + 8, 0x2000 + DESCRIPTOR, 40)
    struct.pack_into("<II", out, dirs + 48, 0x2000 + DEBUG, 28)
    struct.pack_into("<II", out, dirs + 96, 0x2000 + IAT, 24)
    sections = opt + 240
    struct.pack_into("<8sIIII12xI", out, sections, b".text", 0x100, 0x1000, 0x200, TEXT, 0x60000020)
    struct.pack_into("<8sIIII12xI", out, sections + 40, b".rdata", 0x200, 0x2000, 0x200, RDATA, 0x40000040)
    out[TEXT : TEXT + len(code)] = code
    r = RDATA
    hint_name_rva, hint_name_at = 0x2000 + HINT_NAME, r + HINT_NAME
    if hint_name_in_text:
        hint_name_rva, hint_name_at = 0x1000 + 0x80, TEXT + 0x80
    struct.pack_into("<IIIII", out, r + DESCRIPTOR, 0x2000 + ILT, 0, 0, 0x2000 + DLL_NAME, 0x2000 + IAT)
    for table in (ILT, IAT):
        struct.pack_into("<QQQ", out, r + table, hint_name_rva, ORDINAL | ordinal, 0)
    struct.pack_into("<H", out, hint_name_at, hint)
    out[hint_name_at + 2 : hint_name_at + 2 + len(name)] = name
    out[r + DLL_NAME : r + DLL_NAME + len(dll)] = dll
    struct.pack_into("<IIHHIIII", out, r + DEBUG, 0, debug_time, 0, 0, 2, 30, 0x2000 + RSDS, r + RSDS)
    out[r + RSDS : r + RSDS + 4] = b"RSDS"
    out[r + RSDS + 4 : r + RSDS + 20] = pdb_id
    out[r + RSDS + 24 : r + RSDS + 30] = b"a.pdb\0"
    return bytes(out)


class SamePeCode(unittest.TestCase):
    def same(self, a, b):
        same.compare(a, b)

    def differs(self, a, b, why="differ beyond"):
        with self.assertRaisesRegex(same.Mismatch, why):
            same.compare(a, b)

    def test_the_fixture_parses_with_one_name_and_one_ordinal_import(self):
        _, regions = same.masked(pe())
        self.assertEqual([k for _, _, k in regions], ["link time", "debug time", "PDB id", "import hint"])

    def test_link_stamps_pdb_id_and_name_import_hints_may_differ(self):
        self.same(pe(), pe(link_time=2, debug_time=3, pdb_id=b"\x22" * 16, hint=245))

    def test_code_differs(self):
        self.differs(pe(), pe(code=b"\x48\x31\xc0\xcc"))

    def test_an_imported_name_differs(self):
        self.differs(pe(), pe(name=b"GetTickCounu"))

    def test_an_imported_dll_differs(self):
        self.differs(pe(), pe(dll=b"KERNEL33.dll"))

    def test_an_import_by_ordinal_is_never_masked(self):
        self.differs(pe(), pe(ordinal=8))

    def test_header_metadata_differs(self):
        self.differs(pe(), pe(dll_characteristics=0x8120))

    def test_data_differs(self):
        a = bytearray(pe())
        a[RDATA + 0x1F0] = 1
        self.differs(pe(), bytes(a))

    def test_a_hint_inside_code_is_never_masked(self):
        self.differs(pe(hint_name_in_text=True), pe(hint_name_in_text=True, hint=245), "")

    def test_a_truncated_file_fails_closed(self):
        self.differs(pe(), pe()[:0x300], "")

    def test_not_a_pe_file_fails_closed(self):
        self.differs(b"MZ" + bytes(100), b"MZ" + bytes(100), "not a PE")


if __name__ == "__main__":
    result = unittest.main(argv=[sys.argv[0], "-v"], exit=False).result
    sys.exit(0 if result.wasSuccessful() else 1)
