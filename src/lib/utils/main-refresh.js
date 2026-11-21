/**
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
