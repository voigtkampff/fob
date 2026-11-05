# Mini window position

## Sub-features
Drag the window by its title bar, pinned or not (0.7.12); saved to `mini-position.json`;
restored on the next show, pulled into the work area of the monitor holding
most of it; tray position when no monitor overlaps; double-click the title
bar (or Settings, Back to the tray) resets.

## How to get to it (user POV)
Pin the mini window (pin button), drag the bar, close, reopen.

## Driving it with fob-verify.sh
```bash
SETTINGS='{"behavior":{"language":"zh","startInTray":true,"menuBarEnabled":true,"miniPinned":true}}' $V launch drag WINPERF_SHOW_MINI=1
W=$($V windows drag | awk '/Fob mini/{print $1}')
read X Y < <(xdotool getwindowgeometry $W | awk -F'[ ,]+' '/Position/{print $3, $4}')
xdotool mousemove $((X+80)) $((Y+18)) mousedown 1 sleep 0.2 mousemove $((X+90)) $((Y+18)) sleep 0.6
for i in 1 2 3 4 5 6; do xdotool mousemove $((X+80-i*50)) $((Y+18-i*25)); sleep 0.1; done; xdotool mouseup 1
sleep 1.5; $V file drag mini-position.json; $V file drag logs:mini-placement.txt   # "source dragged"
$V relaunch drag WINPERF_SHOW_MINI=1; sleep 6; $V file drag logs:mini-placement.txt | head -1  # "source dragged position"
```
Clamp: write `{"left":1400,"top":700,"right":1760,"bottom":1352}` to the
file, relaunch, expect the window inside 1600x1000. Reset: double-click the
bar, expect `mini-position.json` gone and a `reset` line in the log.

## Gotchas
Unpinned: the same recipe works with `"miniPinned":false` (0.7.12; the
move loop's focus loss is ignored via `mini_drag_begin`). Clicking the Xvfb
desktop never takes focus from the window, so hide-on-focus-loss can't be
shown here. The DWM frame and the Windows
modal move loop are not on Linux.
