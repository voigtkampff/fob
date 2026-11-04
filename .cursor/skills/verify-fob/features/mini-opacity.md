# Mini window opacity

## Sub-features
Slider in Settings (tray card) and in the mini title bar popover (blend
icon), 40 to 100 %, saved as `behavior.miniOpacity`.

## How to get to it (user POV)
Settings > General > Mini window opacity, or the mini window's blend button.

## Driving it with fob-verify.sh
Launch with `"miniOpacity":70` in SETTINGS and check `$V file <name> settings.json`
after moving the slider. Xvfb has no compositor, so transparency is not
visible on the box. For a picture, render the page in headless Chrome with
`/workspace/scratch/chrome/serve.py 8765` and `cdp.mjs` (mocked IPC), then
composite it over a screenshot.

## Gotchas
Only backgrounds fade; text stays opaque. The real look needs Windows.
