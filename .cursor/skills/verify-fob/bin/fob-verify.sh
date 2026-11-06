#!/usr/bin/env bash
# fob-verify.sh <command> <name> [args]  (see ../SKILL.md)
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../../../.." && pwd)
RUNS=${FOB_VERIFY_RUNS:-/workspace/scratch/verify}
EVIDENCE=${FOB_VERIFY_EVIDENCE:-$RUNS/evidence}
export DISPLAY=${DISPLAY_FOB:-:55}
cmd=${1:?command}; name=${2:?name}; shift 2
H=$RUNS/$name; CFG=$H/.config/com.hcb.fob; LOGS=$H/.local/share/com.hcb.fob/logs
mkdir -p "$EVIDENCE"
win() { xdotool search --onlyvisible --pid "$(cat "$H/pid")" --name "${1:-.}" 2>/dev/null | head -1; }
case $cmd in
build)  # build <name>: frontend + debug app with the opaque test config
  cd "$ROOT" && npm run -s build >/dev/null
  cd src-tauri && touch src/main.rs build.rs
  TAURI_CONFIG="$(cat "${CONFIG:-/workspace/scratch/opaque.json}")" cargo build -q
  rm -f "$RUNS/winperf-$name"; mkdir -p "$RUNS"; cp target/debug/winperf "$RUNS/winperf-$name"
  echo "built $RUNS/winperf-$name" ;;
launch) # launch <name> [ENV=value ...]; SETTINGS='{"behavior":{...}}' (unset: first launch)
  [ -f "$H/pid" ] && kill "$(cat "$H/pid")" 2>/dev/null || true
  rm -rf "$H"; mkdir -p "$CFG"
  [ -n "${SETTINGS:-}" ] && printf '%s' "$SETTINGS" > "$CFG/settings.json"
  env HOME="$H" WEBKIT_DISABLE_DMABUF_RENDERER=1 WEBKIT_DISABLE_COMPOSITING_MODE=1 \
    LANG=${LANG_FOB:-en_US.UTF-8} WINPERF_FAKE_GPU=1 "$@" \
    "${BIN:-$RUNS/winperf-app}" > "$H/app.log" 2>&1 &
  echo $! > "$H/pid"
  for _ in $(seq 60); do [ -n "$(win Fob)" ] && break; sleep 0.5; done
  [ -n "$(win Fob)" ] && echo "ready pid $(cat "$H/pid")" || { echo "no window; see $H/app.log"; exit 1; } ;;
relaunch) # relaunch <name> [ENV=value ...]: same HOME, keeps saved state
  [ -f "$H/pid" ] && kill "$(cat "$H/pid")" 2>/dev/null || true; sleep 1
  env HOME="$H" WEBKIT_DISABLE_DMABUF_RENDERER=1 WEBKIT_DISABLE_COMPOSITING_MODE=1 \
    LANG=${LANG_FOB:-en_US.UTF-8} WINPERF_FAKE_GPU=1 "$@" \
    "${BIN:-$RUNS/winperf-app}" >> "$H/app.log" 2>&1 &
  echo $! > "$H/pid"
  for _ in $(seq 60); do [ -n "$(win Fob)" ] && break; sleep 0.5; done
  echo "ready pid $(cat "$H/pid")" ;;
doctor) # doctor <name>: process alive, its windows, saved files
  [ -r "$H/pid" ] || { echo "missing application pid"; exit 1; }
  pid=$(< "$H/pid")
  [[ "$pid" =~ ^[1-9][0-9]*$ ]] || { echo "invalid application pid"; exit 1; }
  kill -0 "$pid" 2>/dev/null || { echo "application is not running"; exit 1; }
  visible=$(xdotool search --onlyvisible --pid "$pid" --name . 2>/dev/null) || { echo "no visible application window"; exit 1; }
  [ -n "$visible" ] || { echo "no visible application window"; exit 1; }
  echo "alive pid $pid"
  for w in $visible; do
    echo "window $w '$(xdotool getwindowname "$w")' $(xdotool getwindowgeometry "$w" | tr '\n' ' ')"; done
  ls "$CFG" 2>/dev/null | sed 's/^/config: /'
  if [ -d "$LOGS" ]; then
    ls "$LOGS" | sed 's/^/log: /'
  else
    echo "no application log directory (optional)"
  fi ;;
windows) # windows <name>: visible window ids and titles
  for w in $(xdotool search --onlyvisible --pid "$(cat "$H/pid")" --name . 2>/dev/null); do
    echo "$w $(xdotool getwindowname "$w")"; done ;;
shot)   # shot <name> <file.png>: whole display
  ffmpeg -loglevel error -y -f x11grab -video_size "${SIZE:-1600x1000}" -i "$DISPLAY" -frames:v 1 "$EVIDENCE/$1"
  echo "saved $EVIDENCE/$1" ;;
file)   # file <name> <relative path under the config dir or logs:NAME>
  case $1 in logs:*) cat "$LOGS/${1#logs:}";; *) cat "$CFG/$1";; esac ;;
stop)   # stop <name>: kills only the process this script started
  kill "$(cat "$H/pid")" 2>/dev/null && echo "stopped" || echo "not running"; rm -f "$H/pid" ;;
*) echo "unknown command $cmd"; exit 2 ;;
esac
