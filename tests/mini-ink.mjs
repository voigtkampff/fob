import assert from "node:assert/strict";
import { themes } from "../src/lib/definitions/themes.ts";
import { themeTokens, themeIsDark, contrast } from "../src/lib/utils/theme-tokens.js";
import { miniInk, miniSurfaces, PRIMARY_CONTRAST, SECONDARY_CONTRAST } from "../src/lib/utils/mini-ink.js";

let checks = 0;
for (const theme of Object.values(themes)) {
  const tokens = themeTokens(theme.colors, { name: theme.name });
  const dark = themeIsDark(theme.colors);
  for (const opacity of [0, 25, 50, 75, 100]) {
    const ink = miniInk(tokens, dark, opacity);
    for (const s of miniSurfaces(tokens, dark, opacity)) {
      assert.ok(contrast(ink.foreground, s) >= PRIMARY_CONTRAST, `${theme.name} ${opacity}% text on ${s}: ${contrast(ink.foreground, s).toFixed(2)}`);
      assert.ok(contrast(ink["muted-foreground"], s) >= SECONDARY_CONTRAST, `${theme.name} ${opacity}% secondary on ${s}`);
      checks++;
    }
  }
  const opaque = miniInk(tokens, dark, 100);
  assert.equal(opaque.foreground, tokens.foreground, `${theme.name}: opaque window keeps theme text`);
  assert.equal(opaque["muted-foreground"], tokens["muted-foreground"], `${theme.name}: opaque keeps secondary`);
}
console.log(`PASS: mini text contrast for ${Object.keys(themes).length} themes at 0/25/50/75/100 % (${checks} surface checks); opaque = theme colors`);
