/** @param {string} name @returns {string} */
export function shortOs(name) {
  return String(name ?? "")
    .replace(/\s+/g, " ")
    .trim()
    .replace(/^Microsoft /i, "")
    .replace(/^Windows(?= )/, "Win");
}
