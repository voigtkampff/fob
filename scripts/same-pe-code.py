#!/usr/bin/env python3
"""Exit 0 when two PE files differ only in link stamps and import hints.

usage: same-pe-code.py <a.exe> <b.exe>

lld writes the wall-clock link time and a PDB signature into every exe, and
the PDB signature hashes temporary paths, so a rebuild never reproduces them.
Each imported name also carries a hint, its index in the DLL's export table as
the Windows SDK import library recorded it. A newer SDK servicing release
shifts these indexes; the loader treats a hint only as a starting guess and
binds by name. Everything else (code, data, resources, imported DLLs and
names) must be byte-identical.
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
    for dll in getattr(pe, "DIRECTORY_ENTRY_IMPORT", []):
        for symbol in dll.imports:
            if symbol.name is not None:
                hint = symbol.name_offset - 2
                assert int.from_bytes(data[hint : hint + 2], "little") == symbol.hint
                stamps.append((hint, 2))
    for offset, size in stamps:
        data[offset : offset + size] = bytes(size)
    return bytes(data)


a, b = sys.argv[1:]
if without_link_stamps(a) != without_link_stamps(b):
    sys.exit(f"{a} and {b} differ beyond link stamps and import hints")
print(f"{a} matches {b} apart from link stamps and import hints")
