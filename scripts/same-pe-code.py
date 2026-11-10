#!/usr/bin/env python3
"""Exit 0 when two PE files differ only in their link time and PDB id.

usage: same-pe-code.py <a.exe> <b.exe>

lld writes the wall-clock link time and a PDB signature into every exe, and
the PDB signature hashes temporary paths, so a rebuild never reproduces them.
Everything else (code, data, resources, imports) must be byte-identical.
Needs python3-pefile.
"""
import sys

import pefile


def without_link_stamps(path):
    data = bytearray(open(path, "rb").read())
    pe = pefile.PE(data=bytes(data))
    stamps = [(pe.FILE_HEADER.get_field_absolute_offset("TimeDateStamp"), 4)]
    for entry in getattr(pe, "DIRECTORY_ENTRY_DEBUG", []):
        stamps.append((entry.struct.get_field_absolute_offset("TimeDateStamp"), 4))
        raw = entry.struct.PointerToRawData
        if data[raw : raw + 4] == b"RSDS":
            stamps.append((raw + 4, 16))
    for offset, size in stamps:
        data[offset : offset + size] = bytes(size)
    return bytes(data)


a, b = sys.argv[1:]
if without_link_stamps(a) != without_link_stamps(b):
    sys.exit(f"{a} and {b} differ beyond the link time and PDB id")
print(f"{a} matches {b} apart from the link time and PDB id")
