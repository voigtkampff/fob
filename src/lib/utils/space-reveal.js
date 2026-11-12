// Words for a failed Open in File Explorer on the space map. The backend
// sends `{ kind, code? }` (space.rs RevealError); anything else is shown as is.

/**
 * @param {unknown} error what `invoke("space_reveal")` rejected with
 * @param {(key: string, vars?: Record<string, string | number>) => string} t
 */
export function revealMessage(error, t) {
  const e = /** @type {{ kind?: unknown, code?: unknown } | null} */ (
    typeof error === "object" ? error : null
  );
  switch (e?.kind) {
    case "gone":
      return t("space.revealGone");
    case "denied":
      return t("space.revealDenied");
    case "explorer":
      return t("space.revealExplorer");
    case "failed":
      return typeof e.code === "number"
        ? t("space.revealFailedCode", { code: e.code })
        : t("space.revealFailed");
    default:
      return String(error);
  }
}
