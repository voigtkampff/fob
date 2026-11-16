#!/usr/bin/env python3
"""Check a packaged Fob folder before it is zipped.

usage: check-sensors-bundle.py <fob.exe> <package-root> <manifest>

The sensor helper's manifest is compiled into fob.exe by src-tauri/build.rs.
Text matching is deliberate: if the exe was built before sensors-helper/build.sh
staged the current helper, its embedded manifest is the old one and this check
fails before a mismatch can reach a user.

Also verifies every manifest entry against the bytes in the package, so an
exe/sensors mix from two builds, a stale DLL or a missing config file is
reported with the exact path. Both sha256sum separators are accepted
(`hash  path` and `hash *path`).
"""
import hashlib
import os
import sys


def entries(text):
    out = []
    for line in text.splitlines():
        line = line.strip()
        if not line:
            continue
        sep = next((i for i, c in enumerate(line) if c.isspace()), None)
        if sep is None:
            continue
        digest = line[:sep].lower()
        path = line[sep:].lstrip().lstrip("*")
        if path.startswith("./"):
            path = path[2:]
        if len(digest) != 64 or any(c not in "0123456789abcdef" for c in digest):
            continue
        if not path or any(p in ("", "..") for p in path.split("/")):
            continue
        out.append((digest, path))
    return out


def main(argv):
    if len(argv) != 4:
        sys.exit(__doc__)
    exe, root, manifest_path = argv[1:]
    manifest_bytes = open(manifest_path, "rb").read()
    exe_bytes = open(exe, "rb").read()
    text = manifest_bytes.decode("utf-8")
    parsed = entries(text)
    if not parsed:
        sys.exit(f"FAIL {manifest_path}: no manifest entries")
    # The exact staged manifest must be the one inside the exe. This is the
    # build-order guard: cargo build after a helper rebuild, then package.
    if manifest_bytes not in exe_bytes:
        sys.exit(
            f"FAIL {exe}: does not embed {manifest_path}. "
            "The exe was built before the current sensor helper was staged; "
            "run the helper build, then cargo build, then package again."
        )
    bad = []
    for digest, rel in parsed:
        path = os.path.join(root, rel)
        try:
            with open(path, "rb") as f:
                actual = hashlib.sha256(f.read()).hexdigest()
        except OSError as e:
            bad.append(f"{rel}: {e}")
            continue
        if actual != digest:
            bad.append(f"{rel}: {actual} != {digest}")
    if bad:
        print(f"FAIL {root}: exe manifest and packaged files differ:", file=sys.stderr)
        for line in bad:
            print(f"  {line}", file=sys.stderr)
        return 1
    print(f"ok   {root}: {len(parsed)} sensor files match the manifest in {exe}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
