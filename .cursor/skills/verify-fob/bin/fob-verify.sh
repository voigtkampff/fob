#!/usr/bin/env bash
# fob-verify.sh <command> <name> [args]  (see ../SKILL.md)
#
# Exit codes: 0 = the check passed, 1 = contract failure, 2 = bad invocation,
#             3 = unsupported platform (this machine has no X11 the harness can drive)
set -euo pipefail
[ $# -ge 2 ] || { echo "usage: fob-verify.sh <command> <name> [args]  (see ../SKILL.md)"; exit 2; }
cmd=${1}; name=${2}; shift 2
RUNS=${FOB_VERIFY_RUNS:-/workspace/scratch/verify}
EVIDENCE=${FOB_VERIFY_EVIDENCE:-$RUNS/evidence}
export DISPLAY=${DISPLAY_FOB:-:55}
H=$RUNS/$name; CFG=$H/.config/com.hcb.fob; LOGS=$H/.local/share/com.hcb.fob/logs

# Capability probe, before anything looks at a process or a window. Every
# command below drives a real desktop: windows are found with `xdotool search`
# and screenshots come from ffmpeg's x11grab. On a platform without an X server
# or without xdotool (Windows, a headless box) nothing here can tell the truth,
# so say that and exit 3 instead of reporting a fake "application is not
# running". Only bash builtins run before this point, so the probe also works
# with an empty PATH.
x11_problem() {
  command -v xdotool >/dev/null 2>&1 || { echo "xdotool is not installed"; return 1; }
  [ -n "${DISPLAY:-}" ] || { echo "DISPLAY is empty"; return 1; }
  xdotool getdisplaygeometry >/dev/null 2>&1 || { echo "no X server answers on DISPLAY=$DISPLAY"; return 1; }
}
case $cmd in
launch|relaunch|doctor|windows|shot)
  if ! problem=$(x11_problem); then
    echo "unsupported on this platform: the verification harness needs X11 and xdotool ($problem)"
    exit 3
  fi ;;
esac
mkdir -p "$EVIDENCE"
ROOT=$(cd "$(dirname "$0")/../../../.." && pwd)
# Process liveness. `kill -0` is the real check whenever the shell knows the
# pid. Git Bash runs the app through MSYS and MSYS cannot signal a native
# Windows pid it never started (its `ps` cannot see it either), so `launch` and
# `relaunch` record the pid they saw alive in "$H/alive" and doctor falls back
# to that, printing which evidence it used. The window check still has to find
# a window owned by that pid, so a marker left behind by a crashed app does not
# pass the doctor.
liveness() { # liveness <pid>: prints the evidence it accepted, else fails
  local recorded=
  kill -0 "$1" 2>/dev/null && { echo "kill"; return 0; }
  if [ -r "$H/alive" ]; then
    { recorded=$(< "$H/alive"); } 2>/dev/null || recorded=
    [ "$recorded" = "$1" ] && { echo "launcher-confirmed"; return 0; }
  fi
  return 1
}
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
  [ -n "$(win Fob)" ] && echo "ready pid $(cat "$H/pid")" || { echo "no window; see $H/app.log"; exit 1; }
  cat "$H/pid" > "$H/alive" ;; # what this launcher saw alive, for doctor
relaunch) # relaunch <name> [ENV=value ...]: same HOME, keeps saved state
  [ -f "$H/pid" ] && kill "$(cat "$H/pid")" 2>/dev/null || true; sleep 1
  env HOME="$H" WEBKIT_DISABLE_DMABUF_RENDERER=1 WEBKIT_DISABLE_COMPOSITING_MODE=1 \
    LANG=${LANG_FOB:-en_US.UTF-8} WINPERF_FAKE_GPU=1 "$@" \
    "${BIN:-$RUNS/winperf-app}" >> "$H/app.log" 2>&1 &
  echo $! > "$H/pid"
  for _ in $(seq 60); do [ -n "$(win Fob)" ] && break; sleep 0.5; done
  echo "ready pid $(cat "$H/pid")"
  cat "$H/pid" > "$H/alive" ;; # what this launcher saw alive, for doctor
doctor) # doctor <name>: process alive, its windows, saved files
  [ -r "$H/pid" ] || { echo "missing application pid"; exit 1; }
  pid=$(< "$H/pid")
  [[ "$pid" =~ ^[1-9][0-9]*$ ]] || { echo "invalid application pid"; exit 1; }
  evidence=$(liveness "$pid") || { echo "application is not running"; exit 1; }
  visible=$(xdotool search --onlyvisible --pid "$pid" --name . 2>/dev/null) || { echo "no visible application window"; exit 1; }
  [ -n "$visible" ] || { echo "no visible application window"; exit 1; }
  echo "alive pid $pid ($evidence)"
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
  kill "$(cat "$H/pid")" 2>/dev/null && echo "stopped" || echo "not running"; rm -f "$H/pid" "$H/alive" ;;
*) echo "unknown command $cmd"; exit 2 ;;
esac
