#!/usr/bin/env python3
"""Audit the actual portable zip against compiled programs and embedded helper pins."""
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import zipfile

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]


def digest(data):
    return hashlib.sha256(data).hexdigest()


def main():
    version = json.loads((ROOT / "package.json").read_text())["version"]
    archive = ROOT / "dist" / f"fob-v{version}-x86_64-windows.zip"
    prefix = f"fob-v{version}/"
    spec = importlib.util.spec_from_file_location("exe_check", ROOT / "scripts/check-windows-exe.py")
    exe_check = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(exe_check)
    with zipfile.ZipFile(archive) as zipped, tempfile.TemporaryDirectory(prefix="helper-audit-", dir=ROOT / "dist") as temporary:
        assert zipped.testzip() is None, "zip CRC failure"
        contents = {name[len(prefix):]: zipped.read(name) for name in zipped.namelist() if name.startswith(prefix)}
        assert "fob.exe" in contents, "missing expected package root"
        programs = {
            "fob.exe": ROOT / "src-tauri/target/x86_64-pc-windows-msvc/release/winperf.exe",
            "sensors/fob-sensors.exe": ROOT / "sensors-helper/dist/sensors/fob-sensors.exe",
            "sensors/fob-mft.exe": ROOT / "sensors-helper/mft-worker/target/x86_64-pc-windows-msvc/release/fob-mft.exe",
        }
        for name, compiled in programs.items():
            assert contents[name] == compiled.read_bytes(), f"packaged {name} differs from compiled output"
            path = Path(temporary) / Path(name).name
            path.write_bytes(contents[name])
            assert exe_check.check(str(path), version, "HCB", name == "fob.exe")
        manifest = (ROOT / "sensors-helper/dist/manifest.sha256").read_bytes()
        assert manifest in contents["fob.exe"], "main executable embeds a different helper manifest"
        for line in manifest.decode().splitlines():
            expected, name = line.split("  ", 1)
            assert digest(contents[name]) == expected, f"helper manifest mismatch for {name}"
        for line in contents["SHA256SUMS.txt"].decode().splitlines():
            expected, name = line.split("  ", 1)
            assert digest(contents[name]) == expected, f"program checksum mismatch for {name}"
        worker_hash = digest(contents["sensors/fob-mft.exe"])
        assert worker_hash.encode("utf-16le") in contents["sensors/fob-sensors.exe"], "helper's worker pin differs from packaged worker"
        assert b"Settings > General > Sensor helper" in contents["README.txt"]
        assert b"No file contents or writes" in contents["README.txt"]
        print("PASS: zip CRC, compiled program equality, HCB versions/asInvoker, embedded manifest, all program hashes and worker pin")
    checksum = digest(archive.read_bytes())
    archive.with_suffix(archive.suffix + ".sha256").write_text(f"{checksum}  {archive.name}\n")
    print(f"{checksum}  {archive}")
    print(f"{archive.stat().st_size} bytes")


if __name__ == "__main__":
    main()
