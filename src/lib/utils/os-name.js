// Fob: a shorter OS name for the narrow System card (the full name is its
// tooltip): "Microsoft Windows 11 Pro for Workstations" -> "Win 11 Pro for
// Workstations". Other systems are only trimmed.
/** @param {string} name @returns {string} */
export function shortOs(name) {
  return String(name ?? "")
    .replace(/\s+/g, " ")
    .trim()
    .replace(/^Microsoft /i, "")
    .replace(/^Windows(?= )/, "Win");
}
