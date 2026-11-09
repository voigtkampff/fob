#!/usr/bin/env python3
"""Check the Windows executables Fob ships before they are zipped.

usage: check-windows-exe.py <version> <helper-version> <company> <fob.exe> <fob-sensors.exe> [icon.ico]

The helper keeps its own version (sensors-helper/src/winperf-sensors.csproj)
so an app-only release ships the same helper bundle and installed helpers stay
current without another UAC update.

fob.exe must embed exactly one application manifest that depends on
Microsoft.Windows.Common-Controls 6.0 (otherwise Windows loads comctl32 v5,
which has no TaskDialogIndirect, and refuses to start the exe), runs asInvoker,
carries VERSIONINFO (CompanyName, version) and the Fob icon. Any exe that
imports a comctl32 v6-only function must have that manifest dependency.
Needs python3-pefile.
"""
import struct
import sys
import xml.etree.ElementTree as ET

import pefile

ASM = "{urn:schemas-microsoft-com:asm.v1}"
# comctl32 exports that only exist in v6 (side-by-side); v5 lacks them.
V6_ONLY = {b"TaskDialog", b"TaskDialogIndirect", b"LoadIconMetric", b"LoadIconWithScaleDown"}


def resources(pe):
    out = {}
    directory = getattr(pe, "DIRECTORY_ENTRY_RESOURCE", None)
    for entry in directory.entries if directory else []:
        kind = pefile.RESOURCE_TYPE.get(entry.struct.Id, str(entry.name))
        for res in entry.directory.entries:
            for lang in res.directory.entries:
                data = pe.get_data(lang.data.struct.OffsetToData, lang.data.struct.Size)
                out.setdefault(kind, []).append((res.struct.Id, data))
    return out


def comctl32_imports(pe):
    names = set()
    for attr in ("DIRECTORY_ENTRY_IMPORT", "DIRECTORY_ENTRY_DELAY_IMPORT"):
        for imp in getattr(pe, attr, []):
            if imp.dll.lower().startswith(b"comctl32"):
                names.update(i.name for i in imp.imports if i.name)
    return names


def manifest_facts(xml_bytes):
    root = ET.fromstring(xml_bytes.decode("utf-8-sig").strip())
    cc6 = False
    for ident in root.iter(ASM + "assemblyIdentity"):
        if ident.get("name") == "Microsoft.Windows.Common-Controls" and ident.get("version", "").startswith("6."):
            cc6 = True
    levels = [e.get("level") for e in root.iter() if e.tag.endswith("requestedExecutionLevel")]
    return cc6, levels


def version_strings(pe):
    strings = {}
    for info in getattr(pe, "FileInfo", None) or []:
        for item in info:
            for table in getattr(item, "StringTable", []):
                strings.update({k.decode(): v.decode() for k, v in table.entries.items()})
    return strings


# Every size Windows 10/11 asks for at 100-250 % scaling; src-tauri/icons/make-ico.py
# renders each from SVG.
ICON_SIZES = [16, 20, 24, 30, 32, 36, 40, 48, 60, 64, 72, 80, 96, 128, 256]


def group_icon_frames(data):
    """(size, bit depth) of each frame in an RT_GROUP_ICON (GRPICONDIR)."""
    _, kind, count = struct.unpack_from("<HHH", data)
    frames = []
    for i in range(count):
        w, _h, _c, _r, _planes, bpp = struct.unpack_from("<BBBBHH", data, 6 + 14 * i)
        frames.append((w or 256, bpp))
    return sorted(frames)


def ico_images(path):
    data = open(path, "rb").read()
    _, kind, count = struct.unpack_from("<HHH", data)
    assert kind == 1, f"{path} is not an .ico"
    images = set()
    for i in range(count):
        size, offset = struct.unpack_from("<II", data, 6 + 16 * i + 8)
        images.add(data[offset : offset + size])
    return images


def check(path, version, company, need_cc6, icon=None):
    errors = []
    pe = pefile.PE(path)
    res = resources(pe)
    manifests = res.get("RT_MANIFEST", [])
    imports = comctl32_imports(pe)
    if len(manifests) != 1:
        errors.append(f"{len(manifests)} RT_MANIFEST resources (want exactly 1)")
    else:
        cc6, levels = manifest_facts(manifests[0][1])
        if (need_cc6 or imports & V6_ONLY) and not cc6:
            errors.append("manifest lacks the Microsoft.Windows.Common-Controls 6.0 dependency")
        if levels != ["asInvoker"]:
            errors.append(f"requestedExecutionLevel {levels} (want ['asInvoker'])")
    if imports & V6_ONLY and not manifests:
        errors.append(f"imports {sorted(n.decode() for n in imports & V6_ONLY)} from comctl32 without a manifest")
    strings = version_strings(pe)
    if strings.get("CompanyName") != company:
        errors.append(f"VERSIONINFO CompanyName {strings.get('CompanyName')!r} (want {company!r})")
    if not strings.get("ProductVersion", "").startswith(version):
        errors.append(f"VERSIONINFO ProductVersion {strings.get('ProductVersion')!r} (want {version})")
    if not res.get("RT_GROUP_ICON"):
        errors.append("no icon (RT_GROUP_ICON)")
    else:
        frames = group_icon_frames(res["RT_GROUP_ICON"][0][1])
        if [w for w, _ in frames] != ICON_SIZES:
            errors.append(f"icon sizes {[w for w, _ in frames]} (want {ICON_SIZES})")
        if any(bpp != 32 for _, bpp in frames):
            errors.append(f"icon frames not all 32-bit: {frames}")
    if icon and res.get("RT_GROUP_ICON"):
        embedded = {data for _, data in res.get("RT_ICON", [])}
        if not embedded or not embedded <= ico_images(icon):
            errors.append(f"embedded icon images differ from {icon}")
    for e in errors:
        print(f"FAIL {path}: {e}")
    if not errors:
        print(f"ok   {path}: 1 manifest (asInvoker{', Common-Controls 6.0' if need_cc6 else ''}), "
              f"CompanyName {company}, version {strings.get('ProductVersion')}, icon")
    return not errors


def main(argv):
    if len(argv) not in (6, 7):
        sys.exit(__doc__)
    version, helper_version, company, fob, sensors = argv[1:6]
    icon = argv[6] if len(argv) == 7 else None
    ok = check(fob, version, company, need_cc6=True, icon=icon)
    ok &= check(sensors, helper_version, company, need_cc6=False)
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main(sys.argv)
