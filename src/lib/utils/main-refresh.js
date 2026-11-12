/** The main window's process list reads. A tick while the window is hidden
 * to the tray or minimized reads nothing; the first focus after such a tick
 * reads at once, so a window brought back does not show the old list until
 * the next tick (up to one refresh interval, 30 s at the longest setting).
 * @param {() => unknown} read
 * @returns {{ tick: (onScreen: boolean) => void, shown: () => void }} */
export function mainRefresh(read) {
  let missed = false;
  return {
    tick(onScreen) {
      missed = !onScreen;
      if (onScreen) read();
    },
    shown() {
      if (!missed) return;
      missed = false;
      read();
    },
  };
}
