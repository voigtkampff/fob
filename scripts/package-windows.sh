#!/usr/bin/env bash
# Build the portable Windows folder + zip from Linux:
#   dist/fob-v<ver>-x86_64-windows.zip  ->  fob-v<ver>/{fob.exe, sensors/, PawnIO/, licenses/, README.txt}
# Needs: node/npm, cargo-xwin + llvm (see README), a .NET 8 SDK (sensors-helper/build.sh).
set -euo pipefail
cd "$(dirname "$0")/.."
# The cross toolchain comes from the script, not from whatever shell runs it:
# without llvm-rc on PATH embed-resource silently skips fob.exe's resources
# (manifest, icon, VERSIONINFO) and Windows refuses to start it.
LLVM_MINGW=${LLVM_MINGW:-/workspace/tools/llvm-mingw-20261020-ucrt-ubuntu-22.04-x86_64}
export PATH=/workspace/tools/xbin:$LLVM_MINGW/bin:$HOME/.cargo/bin:$PATH
command -v llvm-rc >/dev/null || { echo "llvm-rc not found (set LLVM_MINGW)" >&2; exit 1; }
python3 -c 'import pefile' 2>/dev/null || { echo "python3 pefile missing (pip install pefile)" >&2; exit 1; }
VER=$(node -p "require('./package.json').version")
HELPER_VER=$(sed -n 's|.*<Version>\(.*\)</Version>.*|\1|p' sensors-helper/src/winperf-sensors.csproj)
NAME=fob-v$VER
sensors-helper/build.sh              # stages sensors-helper/dist + manifest (embedded by build.rs)
npm run build
(cd src-tauri && cargo xwin build --target x86_64-pc-windows-msvc --release)
STAGE=$(mktemp -d)
OUT=$STAGE/$NAME
mkdir -p "$OUT"
# The cargo crate is still called winperf; the shipped exe is fob.exe.
cp src-tauri/target/x86_64-pc-windows-msvc/release/winperf.exe "$OUT/fob.exe"
cp -r sensors-helper/dist/sensors sensors-helper/dist/PawnIO sensors-helper/dist/licenses "$OUT/"
cp LICENSE "$OUT/licenses/Fob-LICENSE.txt"
cp LICENSE "$OUT/LICENSE.txt"
cat > "$OUT/README.txt" <<TXT
Fob $VER (portable, Windows 10/11 x64)

Run fob.exe. Keep the sensors and PawnIO folders next to it.
Settings > General > Sensor helper shows status, installed and bundled versions,
and Install, Update or Repair when needed. Refresh status never asks for UAC.
Temp & fan keeps the same setup controls. Nothing installs until you choose
an action, read the steps, press Continue, and approve Windows permission:
  - PawnIO driver only if missing, with its own visible installer,
  - the verified helper copied to C:\Program Files\Fob\sensors,
  - the existing fob-sensors task, started now and each time you sign in.
The helper provides temperatures, fans, per-app network counters and MFT scans
without elevating Fob. Programs in the same Windows user session can query
names, sizes and structure across whole fixed NTFS Disks, including otherwise
inaccessible folders. No file contents or writes. It never changes fan speeds.
Disable removes the task and helper after permission. PawnIO stays installed;
uninstall it in Windows Settings if nothing else needs it.
Scanning itself never asks for permission. Old, absent or incompatible helpers
fall back to direct MFT when allowed, otherwise the existing folder scan.

SHA256SUMS.txt lists the hash of every program file in this folder.

Licenses and sources: licenses\NOTICE.txt
TXT
# Fail before zipping if an exe lacks its manifest (Common-Controls 6), VERSIONINFO or icon.
COMPANY=$(sed -n 's/^CompanyName = "\(.*\)"$/\1/p' src-tauri/Cargo.toml)
python3 scripts/check-windows-exe.py "$VER" "$HELPER_VER" "$COMPANY" "$OUT/fob.exe" "$OUT/sensors/fob-sensors.exe" src-tauri/icons/icon.ico
python3 - "$OUT/sensors/fob-mft.exe" "$HELPER_VER" "$COMPANY" <<'PY'
import sys
sys.dont_write_bytecode = True
sys.path.insert(0, "scripts")
from importlib.machinery import SourceFileLoader
m = SourceFileLoader("check_exe", "scripts/check-windows-exe.py").load_module()
assert m.check(sys.argv[1], sys.argv[2], sys.argv[3], False)
PY
(cd "$OUT" && find . -type f \( -iname '*.exe' -o -iname '*.dll' \) | sort | sed 's|^\./||' |
  xargs -d '\n' sha256sum > SHA256SUMS.txt)
mkdir -p dist
cp "$OUT/SHA256SUMS.txt" "dist/$NAME-SHA256SUMS.txt"
python3 - "$STAGE" "$NAME" "dist/$NAME-x86_64-windows.zip" <<'PY'
import os, sys, zipfile
stage, name, out = sys.argv[1:]
with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
    for root, _, files in os.walk(os.path.join(stage, name)):
        for f in sorted(files):
            path = os.path.join(root, f)
            z.write(path, os.path.relpath(path, stage))
print(out, os.path.getsize(out))
PY
rm -rf "$STAGE"
