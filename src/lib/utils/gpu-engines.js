// GPU engine labels, hover text, and “about these readings” source notes.
// Official Chinese engine names come from Microsoft zh-cn DXGK docs and
// NVIDIA cn docs; anything else keeps the raw driver name.

/** @type {Record<string, string>} */
const ZH_NAMES = {
  "3D": "三维引擎",
  Copy: "复制引擎",
  OFA: "光流加速器",
  VideoDecode: "视频解码",
  VideoEncode: "视频编码",
};

/** @type {Record<string, string>} */
const ENGINE_DESC_IDS = {
  "3D": "3d",
  Copy: "copy",
  LegacyOverlay: "legacyOverlay",
  OFA: "ofa",
  Security: "security",
  VR: "vr",
  VideoDecode: "videoDecode",
  VideoEncode: "videoEncode",
};

/** @type {Record<string, string>} */
const SOURCE_IDS = {
  nvml: "nvml",
  "nvml-pdh": "nvmlPdh",
  "nvml-no-pdh": "nvmlNoPdh",
  "nvml-shared-name": "nvmlSharedName",
  "nvml-unmatched": "nvmlUnmatched",
  "dxgi-pdh": "dxgiPdh",
  dxgi: "dxgi",
  "linux-amdgpu": "linuxAmdgpu",
  "linux-drm": "linuxDrm",
  "macos-ioreg": "macosIoreg",
  fake: "fake",
};

/**
 * @param {string} raw
 * @returns {{ base: string, index: string | null }}
 */
export function splitEngineName(raw) {
  const text = String(raw ?? "");
  const match = text.match(/^(.*)_(\d+)$/);
  return match
    ? { base: match[1], index: match[2] }
    : { base: text, index: null };
}

/**
 * @param {string} raw
 * @param {string} locale
 */
export function engineLabel(raw, locale) {
  const text = String(raw ?? "");
  if (locale !== "zh") return text;
  const { base, index } = splitEngineName(text);
  const name = ZH_NAMES[base];
  if (!name) return text;
  return index == null ? name : `${name} ${index}`;
}

/**
 * @param {string} raw
 * @param {(key: string) => string} t
 */
export function engineDescription(raw, t) {
  const { base } = splitEngineName(raw ?? "");
  const id = ENGINE_DESC_IDS[base] ?? "unknown";
  return t(`gpu.engine.${id}`);
}

/**
 * @param {string} raw
 * @param {string} locale
 * @param {(key: string) => string} t
 */
export function engineTitle(raw, locale, t) {
  return `${engineLabel(raw, locale)} · ${engineDescription(raw, t)}`;
}

/**
 * @param {string} source
 * @param {(key: string) => string} t
 */
export function gpuSourceText(source, t) {
  const id = SOURCE_IDS[source];
  return id ? t(`gpu.source.${id}`) : (source ?? "");
}

/**
 * @param {string} source
 * @param {string} memoryKind
 * @param {(key: string) => string} t
 */
export function gpuAboutNote(source, memoryKind, t) {
  return [
    gpuSourceText(source, t),
    memoryKind === "shared" ? t("gpu.sharedNote") : "",
    t("gpu.counterNote"),
  ]
    .filter(Boolean)
    .join(" · ");
}
