import {
  createSettingsFrame,
  previewViewport,
} from "./helper-settings-viewport.js";

const params = new URLSearchParams(location.search);
const viewport = previewViewport(params);
const frame = createSettingsFrame(params, document);
const banner = document.createElement("div");
banner.id = "fixture-banner";
banner.style.width = `${viewport.width}px`;
banner.textContent = `Browser fixture. Mocked Windows helper, no installation or UAC. State ${params.get("state") ?? "absent"}, outcome ${params.get("outcome") ?? "success"}. Setup calls `;
const count = document.createElement("span");
count.id = "fixture-count";
count.textContent = "0";
const reads = document.createElement("span");
reads.id = "fixture-reads";
reads.textContent = "0";
const dimensions = document.createElement("span");
dimensions.id = "fixture-viewport";
dimensions.textContent = `${viewport.width} × ${viewport.height} component viewport`;
banner.append(count, ". Status reads ", reads, ". ", dimensions, ". ");
for (const [field, values] of Object.entries({
  state: [
    "absent",
    "old",
    "mixed",
    "current",
    "stopped",
    "busy",
    "missing",
    "query-error",
  ],
  lang: ["zh", "en"],
  outcome: ["success", "cancel", "error"],
  size: ["min", "normal"],
})) {
  const group = document.createElement("span");
  group.append(`${field}: `);
  for (const value of values) {
    const next = new URLSearchParams(params);
    next.set(field, value);
    const link = document.createElement("a");
    link.href = `?${next}`;
    link.textContent = value;
    group.append(link, " ");
  }
  banner.append(group, " ");
}
window.addEventListener("message", (event) => {
  if (event.origin !== location.origin || event.source !== frame.contentWindow)
    return;
  const data = event.data;
  if (data?.fixture !== "fob-helper-settings") return;
  if (!Number.isSafeInteger(data.operations) || data.operations < 0) return;
  if (!Number.isSafeInteger(data.reads) || data.reads < 0) return;
  count.textContent = String(data.operations);
  reads.textContent = String(data.reads);
});
frame.addEventListener("load", () => {
  dimensions.textContent = `${frame.contentWindow?.innerWidth} × ${frame.contentWindow?.innerHeight} actual component viewport`;
});
document.body.append(banner, frame);
