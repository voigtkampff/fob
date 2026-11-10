#!/usr/bin/env python3
"""Exit 0 when two PE files differ only in link stamps and import hints.

usage: same-pe-code.py <a.exe> <b.exe>

lld writes the wall-clock link time and a PDB signature into every exe, and
the PDB signature hashes temporary paths, so a rebuild never reproduces them.
Each import by name also starts with a 2-byte hint, the export index the
Windows SDK import library recorded. A newer SDK servicing release renumbers
exports, and the PE format says the loader tries the hint first and falls back
to a search by name, so the hint never changes what gets bound.

Only those bytes are masked. Every masked hint must sit in a section that is
not executable and must hold the hint pefile decoded, and both files must mask
the same offsets. Everything else (code, data, resources, headers, imported
DLLs, names and ordinals) must be byte-identical. A file pefile cannot parse
cleanly fails. Needs python3-pefile.
"""
import sys

import pefile

EXECUTABLE = 0x20000000  # IMAGE_SCN_MEM_EXECUTE


class Mismatch(Exception):
    pass


def masked(data):
    """Return (bytes with link stamps and hints zeroed, masked regions)."""
    try:
        pe = pefile.PE(data=data)
    except pefile.PEFormatError as error:
        raise Mismatch(f"not a PE file: {error}")
    if pe.get_warnings():
        raise Mismatch(f"malformed PE: {pe.get_warnings()[0]}")
    regions = []

    def mask(offset, size, kind):
        if offset is None or offset < 0 or offset + size > len(data):
            raise Mismatch(f"{kind} at {offset} is outside the file")
        regions.append((offset, size, kind))

    mask(pe.FILE_HEADER.get_field_absolute_offset("TimeDateStamp"), 4, "link time")
    for entry in getattr(pe, "DIRECTORY_ENTRY_DEBUG", []):
        mask(entry.struct.get_field_absolute_offset("TimeDateStamp"), 4, "debug time")
        raw = entry.struct.PointerToRawData
        if data[raw : raw + 4] == b"RSDS":
            mask(raw + 4, 16, "PDB id")
    for dll in getattr(pe, "DIRECTORY_ENTRY_IMPORT", []):
        for symbol in dll.imports:
            if symbol.import_by_ordinal:
                continue
            if symbol.name is None or symbol.name_offset is None:
                raise Mismatch(f"{dll.dll!r}: import without a name or an ordinal")
            hint = symbol.name_offset - 2
            mask(hint, 2, "import hint")
            if int.from_bytes(data[hint : hint + 2], "little") != symbol.hint:
                raise Mismatch(f"{symbol.name!r}: hint is not where pefile read it")
            section = pe.get_section_by_offset(hint)
            if section is None or not (
                section.PointerToRawData <= hint
                and hint + 2 <= section.PointerToRawData + section.SizeOfRawData
            ):
                raise Mismatch(f"{symbol.name!r}: hint is outside every section")
            if section.Characteristics & EXECUTABLE:
                raise Mismatch(f"{symbol.name!r}: hint is in an executable section")
    out = bytearray(data)
    for offset, size, _ in regions:
        out[offset : offset + size] = bytes(size)
    return bytes(out), regions


def compare(a, b):
    """Raise Mismatch unless a and b differ only in masked bytes."""
    a_data, a_regions = masked(a)
    b_data, b_regions = masked(b)
    if a_regions != b_regions:
        raise Mismatch("the files mask different offsets")
    if a_data != b_data:
        raise Mismatch("they differ beyond link stamps and import hints")


def main(a, b):
    try:
        compare(open(a, "rb").read(), open(b, "rb").read())
    except Mismatch as error:
        sys.exit(f"{a} and {b}: {error}")
    print(f"{a} matches {b} apart from link stamps and import hints")


if __name__ == "__main__":
    main(*sys.argv[1:])
