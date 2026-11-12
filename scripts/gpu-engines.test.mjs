import assert from "node:assert/strict";
import test from "node:test";
import { translate } from "../src/lib/i18n/strings.js";
import {
  engineDescription,
  engineLabel,
  engineTitle,
  gpuAboutNote,
  gpuSourceText,
  splitEngineName,
} from "../src/lib/utils/gpu-engines.js";

const t = (locale) => (key, vars) => translate(locale, key, vars);

test("splits a trailing numeric suffix and leaves other names alone", () => {
  assert.deepEqual(splitEngineName("OFA_0"), { base: "OFA", index: "0" });
  assert.deepEqual(splitEngineName("Security_1"), {
    base: "Security",
    index: "1",
  });
  assert.deepEqual(splitEngineName("VideoDecode_1"), {
    base: "VideoDecode",
    index: "1",
  });
  assert.deepEqual(splitEngineName("Compute_0"), {
    base: "Compute",
    index: "0",
  });
  assert.deepEqual(splitEngineName("OFA_00"), { base: "OFA", index: "00" });
  assert.deepEqual(splitEngineName("3D"), { base: "3D", index: null });
  assert.deepEqual(splitEngineName("Copy"), { base: "Copy", index: null });
  assert.deepEqual(splitEngineName("LegacyOverlay"), {
    base: "LegacyOverlay",
    index: null,
  });
  assert.deepEqual(splitEngineName("Foo_Bar_2"), {
    base: "Foo_Bar",
    index: "2",
  });
});

test("Chinese UI uses official names and keeps the index; English keeps the driver name", () => {
  assert.equal(engineLabel("3D", "zh"), "三维引擎");
  assert.equal(engineLabel("Copy", "zh"), "复制引擎");
  assert.equal(engineLabel("OFA_0", "zh"), "光流加速器 0");
  assert.equal(engineLabel("VideoDecode_1", "zh"), "视频解码 1");
  assert.equal(engineLabel("VideoEncode", "zh"), "视频编码");
  assert.equal(engineLabel("LegacyOverlay", "zh"), "LegacyOverlay");
  assert.equal(engineLabel("Security_1", "zh"), "Security_1");
  assert.equal(engineLabel("VR", "zh"), "VR");
  assert.equal(engineLabel("Compute_0", "zh"), "Compute_0");
  assert.equal(engineLabel("MysteryEngine_2", "zh"), "MysteryEngine_2");
  for (const raw of [
    "3D",
    "Copy",
    "OFA_0",
    "VideoDecode_1",
    "LegacyOverlay",
    "Security_1",
    "VR",
    "Compute_0",
  ]) {
    assert.equal(engineLabel(raw, "en"), raw);
  }
});

test("hover text is only the description, never the engine name", () => {
  assert.equal(
    engineDescription("3D", t("zh")),
    "三维图形渲染，包括游戏和桌面合成",
  );
  assert.equal(
    engineDescription("OFA_0", t("en")),
    "NVIDIA optical flow hardware for motion estimation, such as frame generation and video processing",
  );
  assert.equal(engineTitle("VideoDecode_1", t("zh")), "硬件视频解码，例如播放");
  assert.equal(
    engineTitle("LegacyOverlay", t("en")),
    "Older video overlay display path",
  );
  assert.equal(engineTitle("Compute_0", t("zh")), "显卡驱动报告的引擎类型");
  assert.equal(
    engineTitle("MysteryEngine_2", t("en")),
    "Engine type reported by the graphics driver",
  );
  assert.doesNotMatch(engineTitle("OFA_0", t("zh")), /光流加速器/);
  assert.doesNotMatch(engineTitle("OFA_0", t("en")), /OFA/);
});

test("source notes are localized from codes and never drop an unknown code", () => {
  assert.equal(gpuSourceText("nvml-pdh", t("en")).includes("PDH"), true);
  assert.equal(gpuSourceText("nvml-pdh", t("zh")).includes("同一取样"), true);
  assert.match(gpuSourceText("nvml-pdh", t("zh")), /NVIDIA NVML 驱动/);
  assert.doesNotMatch(gpuSourceText("nvml-pdh", t("zh")), /utilization from/);
  assert.equal(
    gpuSourceText("unexpected-source", t("zh")),
    "unexpected-source",
  );
  const zh = gpuAboutNote("fake", "dedicated", t("zh"));
  assert.match(zh, /调试用模拟显卡，不是真实传感器/);
  assert.match(zh, /显卡驱动不提供的读数显示为 —，不会显示为 0/);
  assert.doesNotMatch(zh, /。/);
  const shared = gpuAboutNote("nvml-pdh", "shared", t("zh"));
  assert.match(shared, /共享显存是系统内存的一部分/);
  assert.match(shared, / · /);
  // The previous template jammed sharedNote against counterNote with no break.
  assert.doesNotMatch(shared, /显存显卡驱动/);
});
