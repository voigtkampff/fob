# Status bar layout standard (Fob 0.7.2)

The tokens live in one block, `.bar` in `src/lib/components/OverlayBar.svelte`
(`--ob-*`). Change them there only.

| Token | Value | Rule |
|---|---|---|
| `--ob-height` | 26px | Bar height (`HEIGHT` in overlay.rs) |
| `--ob-box` | 14px | Every label, value and chart sits in a box this tall, centred on the bar's one centre line (13 px) |
| `--ob-label-size` / `--ob-label-weight` | 11px / 400 | Labels, CJK and Latin alike, `line-height: 1` |
| `--ob-value-size` / `--ob-value-weight` | 13px / 700 | Values, `tabular-nums`; the unit is part of the same text, so it shares the baseline |
| `--ob-value-shift` | -0.5px | Optical fix: bold figures have no descenders and ink 0.5 px low in a centred box |
| `--ob-chart-w` × `--ob-chart-h` | 32 × 9px | Under the values' cap height (~9.3 px); the line is drawn inside, never past the text |
| `--ob-gap-label` | 5px | Label to value |
| `--ob-gap-pair` | 8px | Between the two values of a pair (R/W, ↓/↑) |
| `--ob-gap-chart` | 6px | Value to chart |
| `--ob-gap-metric` | 14px | Between metrics |
| `--ob-pad-x` | 10px | Bar ends |

Measured in headless Chrome at 2x (`v072-*-guides.png`): ink centres are
13.0 (values), 12.75 (CJK labels), 13.25 (Latin labels). The chart box spans
8.5–17.5 and the value ink spans 8–18. The ↓/↑ arrows are taller glyphs and
centre on 12.5.
