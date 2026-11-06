import assert from "node:assert/strict";
import test from "node:test";
import {
  createSettingsFrame,
  previewViewport,
} from "../tests/fixtures/helper-settings-viewport.js";

for (const size of ["", "min", "invalid", "__proto__"]) {
  test(`preview keeps the minimum component viewport for size=${size}`, () => {
    assert.deepEqual(previewViewport(new URLSearchParams({ size })), {
      width: 580,
      height: 560,
    });
  });
}

test("normal preview uses the normal Settings viewport", () => {
  assert.deepEqual(previewViewport(new URLSearchParams("size=normal")), {
    width: 640,
    height: 640,
  });
});

for (const size of ["min", "normal"]) {
  test(`frame attributes isolate size=${size} from the outer banner`, () => {
    const params = new URLSearchParams({
      size,
      lang: "zh",
      state: "old",
      outcome: "error",
    });
    const document = {
      createElement(tag) {
        assert.equal(tag, "iframe");
        return { style: {} };
      },
    };
    const frame = createSettingsFrame(params, document);
    const viewport = previewViewport(params);
    assert.equal(frame.width, String(viewport.width));
    assert.equal(frame.height, String(viewport.height));
    assert.equal(frame.style.cssText, "display:block;border:0");
    const url = new URL(frame.src, "http://localhost/");
    assert.equal(url.pathname, "/helper-settings-panel.html");
    assert.equal(url.searchParams.toString(), params.toString());
    assert.equal(frame.id, "fixture-settings");
  });
}
