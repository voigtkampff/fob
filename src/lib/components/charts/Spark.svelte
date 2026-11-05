<script lang="ts">
  import { sparklinePath } from "$lib/utils/dashboard.js";
  import {
    SERIES_COLOR,
    DASHED,
    STROKE_WIDTH,
    FILL_TOP,
    FILL_BOTTOM,
    sparkAreaPath,
  } from "$lib/utils/chart-style.js";
  import type { ResourceSample } from "$lib/types";

  type Key = keyof typeof SERIES_COLOR;
  let {
    history,
    lines,
    max,
    width,
    height,
    class: klass = "",
  }: {
    history: ResourceSample[];
    lines: {
      key: Key;
      pick: (s: ResourceSample) => number | null | undefined;
    }[];
    max: number;
    width: number;
    height: number;
    class?: string;
  } = $props();
  const uid = Math.random().toString(36).slice(2, 8);
  const paths = $derived(
    lines.map((l) => {
      const d = sparklinePath(history, l.pick, { width, height, max });
      return { key: l.key, d, area: d ? sparkAreaPath(d, height) : "" };
    }),
  );
</script>

<svg
  viewBox={`0 0 ${width} ${height}`}
  preserveAspectRatio="none"
  class={klass}
>
  <defs>
    {#each paths as p}<linearGradient
        id={`spark-${uid}-${p.key}`}
        x1="0"
        y1="0"
        x2="0"
        y2="1"
        ><stop
          offset="0%"
          stop-color={SERIES_COLOR[p.key]}
          stop-opacity={FILL_TOP}
        /><stop
          offset="100%"
          stop-color={SERIES_COLOR[p.key]}
          stop-opacity={FILL_BOTTOM}
        /></linearGradient
      >{/each}
  </defs>
  {#each paths as p}<path
      d={p.area}
      fill={`url(#spark-${uid}-${p.key})`}
      stroke="none"
    />{/each}
  {#each paths as p}<path
      d={p.d}
      fill="none"
      stroke={SERIES_COLOR[p.key]}
      stroke-width={STROKE_WIDTH}
      stroke-dasharray={DASHED.has(p.key) ? "3 2" : undefined}
      stroke-linecap="round"
      stroke-linejoin="round"
      vector-effect="non-scaling-stroke"
    />{/each}
</svg>
