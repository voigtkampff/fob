#!/usr/bin/env bash
# Build the portable Windows folder + zip from Linux:
#   dist/winperf-v<ver>-x86_64-windows.zip  ->  winperf-v<ver>/{winperf.exe, sensors/, PawnIO/, licenses/, README.txt}
# Needs: node/npm, cargo-xwin + llvm (see README), a .NET 8 SDK (sensors-helper/build.sh).
set -euo pipefail
cd "$(dirname "$0")/.."
VER=$(node -p "require('./package.json').version")
NAME=winperf-v$VER
sensors-helper/build.sh              # stages sensors-helper/dist + manifest (embedded by build.rs)
npm run build
(cd src-tauri && cargo xwin build --target x86_64-pc-windows-msvc --release)
STAGE=$(mktemp -d)
OUT=$STAGE/$NAME
mkdir -p "$OUT"
cp src-tauri/target/x86_64-pc-windows-msvc/release/winperf.exe "$OUT/"
cp -r sensors-helper/dist/sensors sensors-helper/dist/PawnIO sensors-helper/dist/licenses "$OUT/"
cp LICENSE "$OUT/licenses/winperf-LICENSE.txt"
cp LICENSE "$OUT/LICENSE.txt"
cat > "$OUT/README.txt" <<TXT
winperf $VER (portable, Windows 10/11 x64)

Run winperf.exe. Keep this folder together: the "sensors" and "PawnIO"
folders are only used if you choose "Enable fan/temperature" in the
Temp & fan card. winperf explains each step and Windows asks for permission
(UAC) once before anything is installed:
  - PawnIO driver (only if it is not installed yet),
  - the read-only helper copied to C:\Program Files\winperf\sensors,
  - the scheduled task "winperf-sensors" that starts it when you sign in
    (it also counts per-app network traffic for the Network detail).
PawnIO's own installer window opens for the driver step.
"Disable..." in the same place removes the task and the helper again.

SHA256SUMS.txt lists the hash of every program file in this folder.

Licenses and sources: licenses\NOTICE.txt
TXT
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
