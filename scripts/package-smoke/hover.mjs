// Disk health row hover checks, shared by the browser preview check and the
// Windows package smoke. The caller moves a pointer; this module reads what
// the page painted. Points are CSS pixels in the page's viewport.

const PAGE = `(() => {
  const rgba = (c) => {
    const n = (c.match(/[\\d.]+/g) || []).map(Number);
    return { rgb: n.slice(0, 3), a: n.length > 3 ? n[3] : 1 };
  };
  const lum = (rgb) => {
    const s = rgb.map((v) => { v /= 255; return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; });
    return 0.2126 * s[0] + 0.7152 * s[1] + 0.0722 * s[2];
  };
  const contrast = (a, b) => { const x = lum(a), y = lum(b); return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05); };
  const cells = () => [...document.querySelectorAll('.panel.health dl dt, .panel.health dl dd')];
  const rows = () => [...document.querySelectorAll('.panel.health dl .metric')];
  window.__diskHover = {
    count: () => rows().length,
    row(i) {
      const row = rows()[i];
      row.scrollIntoView({ block: 'center' });
      const [dt, dd] = [row.querySelector('dt'), row.querySelector('dd')];
      const a = dt.getBoundingClientRect(), b = dd.getBoundingClientRect(), r = row.getBoundingClientRect();
      const y = (top, bottom) => (top + Math.min(bottom, top + 14)) / 2;
      return {
        label: dt.textContent.trim(),
        value: dd.textContent.trim().replace(/\\s+/g, ' '),
        warning: dd.matches('.bad, :has(.bad)'),
        wrapped: b.height > 1.6 * a.height,
        points: {
          label: [a.left + Math.min(a.width / 2, 20), y(a.top, a.bottom)],
          gap: [(a.right + b.left) / 2, y(r.top, r.bottom)],
          value: [b.right - Math.min(b.width / 2, 12), y(b.top, b.bottom)],
        },
        outside: (() => { const n = row.closest('.disk').querySelector('.name').getBoundingClientRect(); return [n.left + 4, n.top + n.height / 2]; })(),
        gapWidth: b.left - a.right,
      };
    },
    state() {
      const all = cells();
      const card = rgba(getComputedStyle(document.querySelector('.panel.health')).backgroundColor).rgb;
      return {
        rects: all.map((e) => { const r = e.getBoundingClientRect(); return [r.x, r.y, r.width, r.height].map((v) => Math.round(v * 100) / 100); }),
        painted: all.flatMap((e, i) => {
          const s = getComputedStyle(e), bg = rgba(s.backgroundColor);
          if (bg.a === 0) return [];
          const row = rows().indexOf(e.closest('.metric'));
          const inner = [e, ...e.querySelectorAll('*')].map((n) => rgba(getComputedStyle(n).color).rgb);
          return [{ index: i, row, part: e.tagName.toLowerCase(), bg: s.backgroundColor, color: s.color, shadow: s.boxShadow,
            contrast: Math.min(...inner.map((c) => contrast(c, bg.rgb))),
            chipVsCard: contrast(bg.rgb, card),
            width: Math.round(e.getBoundingClientRect().width), rowWidth: Math.round(e.closest('.metric').getBoundingClientRect().width) }];
        }),
        decorated: rows().flatMap((row, i) => {
          const r = getComputedStyle(row);
          const marks = [];
          if (rgba(r.backgroundColor).a > 0 || r.backgroundImage !== 'none') marks.push('row fill');
          for (const e of [row, ...row.querySelectorAll('*')]) {
            const c = getComputedStyle(e);
            if (c.textDecorationLine !== 'none') marks.push('underline');
            if (['Top', 'Right', 'Bottom', 'Left'].some((k) => parseFloat(c['border' + k + 'Width']) > 0)) marks.push('border');
            for (const pseudo of ['::before', '::after']) if (!['none', 'normal'].includes(getComputedStyle(e, pseudo).content)) marks.push('pseudo ' + pseudo);
          }
          return marks.length ? [{ row: i, marks: [...new Set(marks)] }] : [];
        }),
        cursor: [...new Set(all.flatMap((e) => [getComputedStyle(e).cursor, getComputedStyle(e.closest('.metric')).cursor]))],
        focusable: all.filter((e) => e.tabIndex >= 0 || e.closest('.metric').tabIndex >= 0 || e.querySelector('a, button, [tabindex]')).length,
        peach: getComputedStyle(document.documentElement).getPropertyValue('--peach').trim(),
      };
    },
  };
  return window.__diskHover.count();
})()`;

/** @param {{ evaluate: (js: string) => Promise<any>, pointTo: (x: number, y: number) => Promise<unknown>, rows?: number[], settle?: () => Promise<unknown> }} page */
export async function checkDiskHover({
  evaluate,
  pointTo,
  rows,
  settle = () => new Promise((r) => setTimeout(r, 120)),
}) {
  const count = await evaluate(PAGE);
  const results = [];
  const fail = (row, msg, detail) =>
    results.push({ row, pass: false, msg, detail });
  for (const i of rows ?? [...Array(count).keys()]) {
    if (i >= count) continue;
    const info = await evaluate(`window.__diskHover.row(${i})`);
    await pointTo(...info.outside);
    await settle();
    const before = await evaluate("window.__diskHover.state()");
    if (before.painted.length)
      fail(i, "a chip is painted before hovering", before.painted);
    const seen = {};
    for (const where of ["label", "gap", "value"]) {
      await pointTo(...info.points[where]);
      await settle();
      const s = await evaluate("window.__diskHover.state()");
      seen[where] = s.painted;
      const parts = s.painted
        .map((p) => `${p.row}:${p.part}`)
        .sort()
        .join(",");
      if (parts !== `${i}:dd,${i}:dt`)
        fail(
          i,
          `hover on the ${where} paints ${parts || "nothing"}, expected only this row's label and value`,
          s.painted,
        );
      if (JSON.stringify(s.rects) !== JSON.stringify(before.rects))
        fail(i, `hover on the ${where} moved the layout`);
      for (const p of s.painted) {
        if (p.contrast < 4.5)
          fail(
            i,
            `${p.part} text contrast ${p.contrast.toFixed(2)} on its chip`,
            p,
          );
        if (p.chipVsCard < 3)
          fail(
            i,
            `${p.part} chip stands out from the panel by only ${p.chipVsCard.toFixed(2)}`,
            p,
          );
        if (p.width >= p.rowWidth - 1)
          fail(i, `${p.part} chip fills the whole row`, p);
      }
      const dd = s.painted.find((p) => p.part === "dd");
      if (
        dd &&
        info.warning !==
          /\d+(\.\d+)?px \d+(\.\d+)?px \d+(\.\d+)?px 3\.5px/.test(dd.shadow)
      )
        fail(
          i,
          info.warning
            ? "warning value has no warning ring"
            : "plain value has a warning ring",
          dd.shadow,
        );
      if (s.decorated.length)
        fail(
          i,
          `hover on the ${where} adds a fill, leader or underline`,
          s.decorated,
        );
      if (s.cursor.some((c) => c === "pointer"))
        fail(i, "rows show a pointer cursor", s.cursor);
      if (s.focusable) fail(i, `${s.focusable} rows are keyboard focusable`);
    }
    await pointTo(...info.outside);
    await settle();
    const after = await evaluate("window.__diskHover.state()");
    if (after.painted.length)
      fail(i, "chips stay painted after the pointer leaves", after.painted);
    results.push({
      row: i,
      pass: !results.some((r) => r.row === i && !r.pass),
      label: info.label,
      value: info.value,
      warning: info.warning,
      wrapped: info.wrapped,
      gapWidth: Math.round(info.gapWidth),
      chips: seen.gap?.map(({ part, bg, color, shadow, contrast, width }) => ({
        part,
        bg,
        color,
        shadow,
        contrast: Math.round(contrast * 100) / 100,
        width,
      })),
    });
  }
  return { count, results, pass: count > 0 && results.every((r) => r.pass) };
}
