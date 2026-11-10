#!/usr/bin/env bash
# Build the read-only sensor helper (.NET Framework 4.7.2) from Linux and stage
# everything Fob bundles next to fob.exe:
#   dist/sensors/            fob-sensors.exe + LibreHardwareMonitorLib and deps
#   dist/PawnIO/             PawnIO_setup.exe 2.2.0 (signed by namazso, pinned sha256)
#   dist/licenses/           MPL-2.0, LHM third-party notices, PawnIO GPL-2.0, ...
#   dist/manifest.sha256     hashes fob.exe embeds and checks before the
#                            elevated setup copies or runs any of these files
# Needs cargo-xwin, llvm-rc on PATH, python3-pefile, and a .NET 8 SDK: $DOTNET, /workspace/tools/dotnet/dotnet or dotnet on PATH.
set -euo pipefail
cd "$(dirname "$0")"
DOTNET=${DOTNET:-$(command -v dotnet || echo /workspace/tools/dotnet/dotnet)}
export DOTNET_CLI_TELEMETRY_OPTOUT=1 DOTNET_NOLOGO=1

PAWNIO_URL=https://github.com/namazso/PawnIO.Setup/releases/download/2.2.0/PawnIO_setup.exe
PAWNIO_SHA256=1f519a22e47187f70a1379a48ca604981c4fcf694f4e65b734aaa74a9fba3032
PAWNIO_CACHE=${PAWNIO_CACHE:-$HOME/.cache/winperf/PawnIO_setup-2.2.0.exe}

# Installed helpers are checked against fob.exe's embedded manifest, so an
# unchanged helper version must ship the released bytes or every user gets a
# UAC update. released-helper-<version>.sha256 is that manifest.
HELPER_VER=$(sed -n 's|.*<Version>\(.*\)</Version>.*|\1|p' src/winperf-sensors.csproj)
RELEASED=released-helper-$HELPER_VER.sha256
# The 0.8.2 worker as released, inside the v0.8.4 zip.
RELEASED_ZIP_URL=https://github.com/voigtkampff/fob/releases/download/v0.8.4/fob-v0.8.4-x86_64-windows.zip
RELEASED_ZIP_SHA256=5e2a40cdefbd006eaf102af927cc338c6709b9a650c91ccce32b7a79aba8456c
RELEASED_ZIP=${RELEASED_ZIP:-$HOME/.cache/winperf/fob-v0.8.4-x86_64-windows.zip}

fetch() { # url sha256 path
  if [ ! -f "$3" ]; then
    mkdir -p "$(dirname "$3")"
    curl -fsSL -o "$3.part" "$1"
    mv "$3.part" "$3"
  fi
  echo "$2  $3" | sha256sum -c -
}

"$DOTNET" run --project tests -c Release
rm -rf dist
# mft-worker/rust-toolchain.toml pins the compiler. Panic messages embed
# CARGO_HOME, which was this path for the released worker.
(cd mft-worker && RUSTFLAGS="-C link-arg=/IGNORE:4099 --remap-path-prefix=${CARGO_HOME:-$HOME/.cargo}=/workspace/winperf/dist/toolchain/cargo" \
  cargo xwin build --locked --target x86_64-pc-windows-msvc --release)
mkdir -p src/obj dist/sensors
cp mft-worker/target/x86_64-pc-windows-msvc/release/fob-mft.exe dist/sensors/
if [ -f "$RELEASED" ]; then
  # A rebuild differs only in link stamps and import hints; ship the released file.
  fetch "$RELEASED_ZIP_URL" "$RELEASED_ZIP_SHA256" "$RELEASED_ZIP"
  python3 -c 'import sys, zipfile; open(sys.argv[3], "wb").write(zipfile.ZipFile(sys.argv[1]).read(sys.argv[2]))' \
    "$RELEASED_ZIP" fob-v0.8.4/sensors/fob-mft.exe dist/fob-mft.released.exe
  python3 ../scripts/same-pe-code.test.py
  python3 ../scripts/same-pe-code.py dist/sensors/fob-mft.exe dist/fob-mft.released.exe
  mv dist/fob-mft.released.exe dist/sensors/fob-mft.exe
fi
WORKER_SHA=$(sha256sum dist/sensors/fob-mft.exe | cut -d ' ' -f 1)
printf 'namespace WinperfSensors { static class MftWorkerHash { internal const string Value = "%s"; } }\n' "$WORKER_SHA" > src/obj/MftWorkerHash.g.cs
"$DOTNET" publish src/winperf-sensors.csproj -c Release -o dist/sensors -p:ContinuousIntegrationBuild=true
rm -f dist/sensors/*.pdb

fetch "$PAWNIO_URL" "$PAWNIO_SHA256" "$PAWNIO_CACHE"
mkdir -p dist/PawnIO dist/licenses
cp "$PAWNIO_CACHE" dist/PawnIO/PawnIO_setup.exe
cp licenses/* dist/licenses/

( cd dist && find sensors PawnIO -type f | LC_ALL=C sort | xargs sha256sum ) > dist/manifest.sha256
if [ -f "$RELEASED" ] && ! diff -u "$RELEASED" dist/manifest.sha256; then
  echo "The helper differs from the released $HELPER_VER helper. Raise its version in src/winperf-sensors.csproj." >&2
  exit 1
fi
echo "helper staged in $(pwd)/dist ($(du -sh dist | cut -f1))"
