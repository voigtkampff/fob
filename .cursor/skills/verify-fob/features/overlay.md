# Status bar (overlay, v0.7.0)

## Sub-features
Thin always-on-top row of readings (`overlay.rs`, `OverlayBar.svelte`,
`utils/overlay.js`). Click-through; Ctrl (Windows poll) or `overlayUnlocked`
makes it draggable. Drag end snaps to top/bottom within 24 px, saves
`overlay-position.json` and `behavior.overlayDock`.

## Driving it with fob-verify.sh
Put every key you need in SETTINGS: the app only rewrites settings.json on
a change, so `sed` on a key that isn't in the file does nothing.
`"overlayEnabled":true,"overlayUnlocked":true` then drag with xdotool
(press, small move, 0.5 s pause, steps). Window title `Fob status bar`.

## Gotchas
- xfwm's compositor on the box only repaints the changed parts of the
  transparent bar, so box screenshots show just the changing numbers. For a
  picture render `/?view=overlay&label=hud&overlay=cpu,...&hudOpacity=60` with
  `/workspace/scratch/chrome/serve.py 8765` and `shot.mjs`.
- Ctrl click-through and WS_EX_NOACTIVATE are Windows-only paths.
