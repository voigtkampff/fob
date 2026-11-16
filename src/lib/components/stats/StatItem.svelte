<script lang="ts">
  // Fob: one line per row. A long value (an OS or GPU name) ends in "…"
  // and shows in full as a tooltip; it never wraps into the rows below.
  import Tip from "$lib/components/Tip.svelte";
  export let label: string;
  export let value: string;
  /** Full text for the tooltip when `value` is a shortened form. */
  export let full: string = value;
</script>

<div
  class="flex min-w-0 items-baseline justify-between gap-2 text-xs leading-4"
>
  <Tip text={label}>
    {#snippet children(props)}<span
        class="max-w-[50%] shrink-0 truncate text-muted-foreground"
        {...props}>{label}</span
      >{/snippet}
  </Tip><Tip text={full || value}>
    {#snippet children(props)}<span
        class="min-w-0 truncate text-right tabular-nums"
        {...props}>{value}</span
      >{/snippet}
  </Tip>
</div>
