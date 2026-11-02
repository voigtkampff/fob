// winperf: the UI locale. `language` setting ("auto" | "en" | "zh"); "auto"
// follows the Windows display language as reported by the OS plugin.
import { derived, writable } from "svelte/store";
import { isTauri } from "@tauri-apps/api/core";
import { locale as osLocale } from "@tauri-apps/plugin-os";
import { committedSettings } from "$lib/stores/settings";
import { resolveLocale, translate } from "./strings.js";

export type Locale = "en" | "zh";
export type LanguagePref = "auto" | Locale;
export type Translate = (
  key: string,
  vars?: Record<string, string | number>,
) => string;

const systemLanguage = writable<string>(
  typeof navigator === "undefined" ? "en" : navigator.language,
);
if (isTauri())
  void osLocale()
    .then((tag) => tag && systemLanguage.set(tag))
    .catch(() => {});

export const locale = derived(
  [committedSettings, systemLanguage],
  ([settings, system]) =>
    resolveLocale(settings.behavior.language, system) as Locale,
);

export const t = derived(
  locale,
  (current): Translate =>
    (key, vars) =>
      translate(current, key, vars),
);

// <html lang> for fonts and screen readers.
if (typeof document !== "undefined")
  locale.subscribe((current) => {
    document.documentElement.lang = current === "zh" ? "zh-CN" : "en";
  });
