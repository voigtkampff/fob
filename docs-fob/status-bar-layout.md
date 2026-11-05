# Status bar layout standard (Fob 0.7.2)

The tokens live in one block, `.bar` in `src/lib/components/OverlayBar.svelte`
(`--ob-*`). Change them there only.

| Token | Value | Rule |
|---|---|---|
| `--ob-height` | 26px | Bar height (`HEIGHT` in overlay.rs) |
| `--ob-box` | 14px | Every label, value and chart sits in a box this tall, centred on the bar's one centre line (13 px) |
| `--ob-label-size` / `--ob-label-weight` | 11px / 400 | Latin labels, `line-height: 1` |
| `--ob-label-size-cjk` | 11px | CJK labels (0.7.6: 10px looked small and light on Windows) |
| `--ob-font-latin` | "Segoe UI", Selawik | Latin labels and all values (tabular figures, `tnum`) |
| `--ob-font-cjk` | "Microsoft YaHei UI", "Microsoft YaHei", "Noto Sans CJK SC" | CJK labels |
| `--ob-value-size` / `--ob-value-weight` | 13px / 700 | Values, `tabular-nums`; the unit is part of the same text, so it shares the baseline |
| `--ob-shift-value` / `-latin` / `-cjk` | measured | Set at run time by `inkShift` from the real font's canvas metrics, so each script's ink is centred whatever font the WebView picks |
| `--ob-chart-w` × `--ob-chart-h` | 32 × 9px | Under the values' cap height (~9.3 px); the line is drawn inside, never past the text |
| `--ob-gap-label` | 5px | Label to value |
| `--ob-gap-pair` | 8px | Between the two values of a pair (R/W, ↓/↑); the fixed width (13ch net, 15ch disk) is on the pair plus its chart, so the chart sits --ob-gap-chart after the second value and the slack falls after the chart |
| `--ob-gap-chart` | 6px | Value to chart |
| `--ob-gap-metric` | 14px | Between metrics |
| `--ob-pad-x` | 10px | Bar ends |

Measured in headless Chrome at 2x (`v072-*-guides.png`): ink centres are
13.0 (values), 12.75 (CJK labels), 13.25 (Latin labels). The chart box spans
8.5–17.5 and the value ink spans 8–18. The ↓/↑ arrows are taller glyphs and
centre on 12.5.

0.7.5: the font stack names Windows fonts first and the Linux stand-ins
(Selawik, Noto Sans CJK SC) after them; ↓/↑ are drawn as 7×9 px paths, not
font glyphs. Values are left-aligned in their fixed-width boxes: the
label-to-value gap is exactly `--ob-gap-label`, and the slack from short
numbers (6% in a 4ch box) falls after the value, before the next metric.

0.7.6: a `line-height: 1` box centres the font's ascent + descent, not its
glyphs. Microsoft YaHei UI's metrics differ from Segoe UI's, so on Windows
the CJK labels sat off the values' centre line. The bar now measures each
script's real font with canvas `measureText` and shifts it so the ink centre
is on the line.
