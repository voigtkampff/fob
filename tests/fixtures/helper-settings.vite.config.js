import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = fileURLToPath(new URL("../..", import.meta.url));
const manifest = JSON.parse(
  readFileSync(
    resolve(projectRoot, ".svelte-kit/output/client/.vite/manifest.json"),
    "utf8",
  ),
);
const assets = new Set();
const visited = new Set();
/** @param {string} key */
function collectStyles(key) {
  if (visited.has(key)) return;
  visited.add(key);
  const entry = manifest[key];
  if (!entry) throw new Error(`Missing production manifest entry ${key}`);
  for (const dependency of entry.imports ?? []) collectStyles(dependency);
  for (const asset of entry.css ?? []) assets.add(asset);
}
for (const name of ["nodes/0", "nodes/2"]) {
  const key = Object.keys(manifest).find((key) => manifest[key].name === name);
  if (!key) throw new Error(`Build production frontend first, missing ${name}`);
  collectStyles(key);
}
if (!assets.size) throw new Error("Production frontend has no CSS assets");
const styles = new Map(
  [...assets].map((asset) => [
    `/fixture-production/${asset}`,
    readFileSync(resolve(projectRoot, "build", asset)),
  ]),
);

// Fixture pages that receive the production stylesheets.
const panels = new Set([
  "/helper-settings-panel.html",
  "/process-table-panel.html",
  "/cpu-views-panel.html",
  "/disk-health-panel.html",
  "/main-page-panel.html",
]);

export default defineConfig({
  root: fileURLToPath(new URL(".", import.meta.url)),
  plugins: [
    svelte({ configFile: false }),
    {
      name: "fixture-production-css",
      transformIndexHtml(html, context) {
        if (!panels.has(context.path)) return html;
        return [...styles.keys()].map((href) => ({
          tag: "link",
          attrs: { rel: "stylesheet", href },
          injectTo: "head",
        }));
      },
      configureServer(server) {
        server.middlewares.use((request, response, next) => {
          const css = styles.get((request.url ?? "").split("?")[0]);
          if (!css) return next();
          response.setHeader("Content-Type", "text/css; charset=utf-8");
          response.setHeader("Cache-Control", "no-store");
          response.end(css);
        });
      },
    },
  ],
  resolve: {
    alias: { $lib: fileURLToPath(new URL("../../src/lib", import.meta.url)) },
  },
  server: { fs: { allow: [projectRoot] } },
});
