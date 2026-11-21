// Fob: text colors for the mini window at low opacity. The window is
// transparent over the Windows 11 Acrylic backdrop, whose tint follows the
// window's dark/light mode, which the theme store sets from the theme. So the
// surface behind text is known without sampling the desktop: the acrylic
// tint, then the theme panel and card mixed in at the opacity. Text moves
// toward white or black only as far as it must to stay readable.
import { composite, contrast } from "./theme-tokens.js";

export const ACRYLIC = { dark: "#2c2c2c", light: "#f3f3f3" };
export const POPUP_FLOOR = 0.8;
export const PRIMARY_CONTRAST = 4.5;
export const SECONDARY_CONTRAST = 3;

/**
 * @param {Record<string,string>} tokens themeTokens() output
 * @param {boolean} dark @param {number} opacity */
export function miniSurfaces(tokens, dark, opacity) {
  const a = Math.min(1, Math.max(0, opacity / 100));
  const panel = composite(
    tokens.background,
    dark ? ACRYLIC.dark : ACRYLIC.light,
    a,
  );
  return [
    panel,
    composite(tokens.card, panel, a),
    composite(tokens.popover, panel, Math.max(a, POPUP_FLOOR)),
  ];
}

/**
 * @param {string} color @param {string[]} surfaces @param {number} min */
function reach(color, surfaces, min) {
  for (let step = 0; step <= 100; step++)
    for (const target of ["#ffffff", "#000000"]) {
      const c = composite(target, color, step / 100);
      if (surfaces.every((s) => contrast(c, s) >= min)) return c;
    }
  return surfaces.reduce(
    (sum, s) =>
      sum + (contrast("#ffffff", s) >= contrast("#000000", s) ? 1 : -1),
    0,
  ) >= 0
    ? "#ffffff"
    : "#000000";
}

/**
 * @param {Record<string,string>} tokens @param {boolean} dark @param {number} opacity */
export function miniInk(tokens, dark, opacity) {
  const surfaces = miniSurfaces(tokens, dark, opacity);
  const foreground = reach(tokens.foreground, surfaces, PRIMARY_CONTRAST);
  const muted = reach(tokens["muted-foreground"], surfaces, SECONDARY_CONTRAST);
  return {
    foreground,
    "card-foreground": foreground,
    "popover-foreground": foreground,
    "muted-foreground": muted,
  };
}
