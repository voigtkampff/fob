#!/usr/bin/env bash
# Build the read-only sensor helper (.NET Framework 4.7.2) from Linux and stage
# everything winperf bundles next to winperf.exe:
#   dist/sensors/            winperf-sensors.exe + LibreHardwareMonitorLib and deps
#   dist/PawnIO/             PawnIO_setup.exe 2.2.0 (signed by namazso, pinned sha256)
#   dist/licenses/           MPL-2.0, LHM third-party notices, PawnIO GPL-2.0, ...
#   dist/manifest.sha256     hashes winperf.exe embeds and checks before the
#                            elevated setup copies or runs any of these files
# Needs a .NET 8 SDK: $DOTNET, /workspace/tools/dotnet/dotnet or dotnet on PATH.
set -euo pipefail
cd "$(dirname "$0")"
DOTNET=${DOTNET:-$(command -v dotnet || echo /workspace/tools/dotnet/dotnet)}
export DOTNET_CLI_TELEMETRY_OPTOUT=1 DOTNET_NOLOGO=1

PAWNIO_URL=https://github.com/namazso/PawnIO.Setup/releases/download/2.2.0/PawnIO_setup.exe
PAWNIO_SHA256=1f519a22e47187f70a1379a48ca604981c4fcf694f4e65b734aaa74a9fba3032
PAWNIO_CACHE=${PAWNIO_CACHE:-$HOME/.cache/winperf/PawnIO_setup-2.2.0.exe}

"$DOTNET" run --project tests -c Release
rm -rf dist
"$DOTNET" publish src/winperf-sensors.csproj -c Release -o dist/sensors -p:ContinuousIntegrationBuild=true
rm -f dist/sensors/*.pdb

if [ ! -f "$PAWNIO_CACHE" ]; then
  mkdir -p "$(dirname "$PAWNIO_CACHE")"
  curl -fsSL -o "$PAWNIO_CACHE.part" "$PAWNIO_URL"
  mv "$PAWNIO_CACHE.part" "$PAWNIO_CACHE"
fi
echo "$PAWNIO_SHA256  $PAWNIO_CACHE" | sha256sum -c -
mkdir -p dist/PawnIO dist/licenses
cp "$PAWNIO_CACHE" dist/PawnIO/PawnIO_setup.exe
cp licenses/* dist/licenses/

( cd dist && find sensors PawnIO -type f | LC_ALL=C sort | xargs sha256sum ) > dist/manifest.sha256
echo "helper staged in $(pwd)/dist ($(du -sh dist | cut -f1))"
