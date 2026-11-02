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

Run fob.exe. Keep this folder together: the "sensors" and "PawnIO"
folders are only used if you choose "Enable fan/temperature" in the
Temp & fan card. Fob explains each step and Windows asks for permission
(UAC) once before anything is installed:
  - PawnIO driver (only if it is not installed yet),
  - the read-only helper copied to C:\Program Files\Fob\sensors,
  - the scheduled task "fob-sensors" that starts it when you sign in
    (it also counts per-app network traffic for the Network detail).
PawnIO's own installer window opens for the driver step.
"Disable..." in the same place removes the task and the helper again.

SHA256SUMS.txt lists the hash of every program file in this folder.

Licenses and sources: licenses\NOTICE.txt
TXT
# Fail before zipping if an exe lacks its manifest (Common-Controls 6), VERSIONINFO or icon.
COMPANY=$(sed -n 's/^CompanyName = "\(.*\)"$/\1/p' src-tauri/Cargo.toml)
python3 scripts/check-windows-exe.py "$VER" "$COMPANY" "$OUT/fob.exe" "$OUT/sensors/fob-sensors.exe" src-tauri/icons/icon.ico
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
