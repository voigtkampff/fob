import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { createServer } from "vite";
import postcss from "postcss";

const root = new URL("../", import.meta.url);
const manifest = JSON.parse(
  readFileSync(
    new URL(".svelte-kit/output/client/.vite/manifest.json", root),
    "utf8",
  ),
);
const expected = new Set();
const pending = Object.entries(manifest)
  .filter(([, value]) => ["nodes/0", "nodes/2"].includes(value.name))
  .map(([key]) => key);
assert.equal(pending.length, 2, "production layout and page entries exist");
const visited = new Set();
while (pending.length) {
  const key = pending.pop();
  if (visited.has(key)) continue;
  visited.add(key);
  const entry = manifest[key];
  for (const asset of entry.css ?? []) expected.add(asset);
  pending.push(...(entry.imports ?? []));
}
assert.ok(expected.size >= 1, "production has compiled styles");
const server = await createServer({
  configFile: fileURLToPath(
    new URL("fixtures/helper-settings.vite.config.js", import.meta.url),
  ),
  server: { host: "127.0.0.1", port: 0, strictPort: true },
  logLevel: "error",
});
try {
  await server.listen();
  const address = server.httpServer.address();
  assert.ok(address && typeof address !== "string");
  const base = `http://127.0.0.1:${address.port}`;
  async function get(path) {
    const response = await fetch(new URL(path, base));
    assert.equal(response.status, 200, path);
    return response;
  }
  const panel = await (
    await get("/helper-settings-panel.html?lang=zh&state=old&outcome=error")
  ).text();
  const links = [
    ...panel.matchAll(
      /<link rel="stylesheet" href="(\/fixture-production\/[^"]+)">/g,
    ),
  ].map((match) => match[1]);
  assert.deepEqual(
    new Set(links.map((link) => link.replace("/fixture-production/", ""))),
    expected,
  );
  let css = "";
  for (const link of links) {
    const response = await get(link);
    assert.match(response.headers.get("content-type"), /^text\/css/);
    assert.equal(response.headers.get("cache-control"), "no-store");
    const bytes = Buffer.from(await response.arrayBuffer());
    const asset = link.replace("/fixture-production/", "");
    assert.deepEqual(
      bytes,
      readFileSync(new URL(`build/${asset}`, root)),
      asset,
    );
    css += bytes.toString();
  }
  const parsed = postcss.parse(css);
  const utilities = [
    [".flex", "display", "flex"],
    [".h-screen", "height", "100vh"],
    [".flex-col", "flex-direction", "column"],
    [".justify-between", "justify-content", "space-between"],
    [".gap-6", "gap", "calc(var(--spacing) * 6)"],
    [".p-6", "padding", "calc(var(--spacing) * 6)"],
    [".text-lg", "font-size", "var(--text-lg)"],
    [".text-sm", "font-size", "var(--text-sm)"],
    [".appearance-none", "appearance", "none"],
    [
      ":where(.space-y-5>:not(:last-child))",
      "margin-block-end",
      "calc(calc(var(--spacing) * 5) * calc(1 - var(--tw-space-y-reverse)))",
    ],
  ];
  for (const [selector, property, value] of utilities) {
    let present = false;
    parsed.walkRules(selector, (rule) =>
      rule.walkDecls(property, (declaration) => {
        if (declaration.value === value) present = true;
      }),
    );
    assert.ok(present, `${selector} supplies ${property}: ${value}`);
  }
  const fixture = await (await get("/helper-settings-preview.js")).text();
  assert.match(
    fixture,
    /import SettingsWindow from [^;]*src\/lib\/components\/SettingsWindow.svelte/,
  );
  assert.match(fixture, /mockIPC/);
  assert.doesNotMatch(fixture, /import [^;]*app\.css/);
  const component = await (
    await get(
      `/@fs${fileURLToPath(new URL("src/lib/components/SettingsWindow.svelte", root))}`,
    )
  ).text();
  assert.match(component, /flex h-screen flex-col bg-background/);
  const host = await (await get("/helper-settings.html?size=min")).text();
  assert.match(host, /helper-settings-frame\.js/);
  assert.doesNotMatch(host, /fixture-production/);
  assert.doesNotMatch(panel, /fixture-banner|fixture-settings/);
  const frameModule = await (await get("/helper-settings-frame.js")).text();
  assert.match(frameModule, /createSettingsFrame/);
  const viewportModule = await (
    await get("/helper-settings-viewport.js")
  ).text();
  assert.match(viewportModule, /frame.width = String\(viewport.width\)/);
  assert.match(viewportModule, /frame.height = String\(viewport.height\)/);
  console.log(
    `helper Settings preview passes. ${links.length} production CSS files are byte-identical, ${utilities.length} utility declaration checks pass, real component and isolated panel return HTTP 200`,
  );
} finally {
  await server.close();
}
