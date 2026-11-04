<script lang="ts">
  // Fob: one line per row. A long value (an OS or GPU name) ends in "…"
  // and shows in full as a tooltip; it never wraps into the rows below.
  export let label: string;
  export let value: string;
  /** Full text for the tooltip when `value` is a shortened form. */
  export let full: string = value;
</script>

<div
  class="flex min-w-0 items-baseline justify-between gap-2 text-xs leading-4"
>
  <span
    class="max-w-[50%] shrink-0 truncate text-muted-foreground"
    title={label}>{label}</span
  ><span class="min-w-0 truncate text-right tabular-nums" title={full || value}
    >{value}</span
  >
</div>
