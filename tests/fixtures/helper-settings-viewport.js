const sizes = {
  min: { width: 580, height: 560 },
  normal: { width: 640, height: 640 },
};

/** @param {URLSearchParams} params */
export function previewViewport(params) {
  return params.get("size") === "normal" ? sizes.normal : sizes.min;
}

/** @param {URLSearchParams} params @param {Document} document */
export function createSettingsFrame(params, document) {
  const viewport = previewViewport(params);
  const frame = document.createElement("iframe");
  frame.id = "fixture-settings";
  frame.title = "Actual Settings component with mocked Tauri transport";
  frame.src = `./helper-settings-panel.html?${params}`;
  frame.width = String(viewport.width);
  frame.height = String(viewport.height);
  frame.style.cssText = "display:block;border:0";
  return frame;
}
