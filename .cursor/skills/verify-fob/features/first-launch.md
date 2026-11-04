# First launch

## Sub-features
With no settings file the main window opens even though tray-only mode is
the default. Later launches in tray-only mode show only the tray icon.

## How to get to it (user POV)
Start Fob for the first time.

## Driving it with fob-verify.sh
`$V launch first` (no SETTINGS) then `$V windows first` lists `Fob`.
`$V relaunch first` then `$V windows first` lists no `Fob` main window.

## Gotchas
The settings file appears right after the first launch; delete the run
(`launch` again) to repeat it.
