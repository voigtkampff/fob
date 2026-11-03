<script lang="ts">
  // Fob: the one body layout every side panel uses, in this order:
  // (picker) → identity → key stats → history chart → detail sections →
  // note. Panels fill the parts; spacing, card style and type come from here.
  import type { Snippet } from "svelte";
  import * as Card from "$lib/components/ui/card/index.js";
  let {
    picker,
    name,
    sub = "",
    facts = [],
    chartTitle = "",
    chart,
    detail,
    noteTitle = "",
    note = "",
  }: {
    /** Above the identity, e.g. the GPU adapter choice. */
    picker?: Snippet;
    name: string;
    sub?: string;
    facts?: [string, string][];
    chartTitle?: string;
    chart?: Snippet;
    /** One or more DetailSection cards. */
    detail?: Snippet;
    noteTitle?: string;
    note?: string;
  } = $props();
</script>

<div class="detail-layout flex flex-col gap-5">
  {#if picker}{@render picker()}{/if}
  <div class="min-w-0">
    <h3 class="break-words text-lg font-semibold">{name}</h3>
    {#if sub}<p class="mt-1 break-words text-sm text-muted-foreground">
        {sub}
      </p>{/if}
  </div>
  {#if facts.length}<Card.Root size="sm"
      ><Card.Content class="gap-0 divide-y divide-border">
        {#each facts as [label, value]}<div
            class="flex justify-between gap-4 py-3 first:pt-0 last:pb-0"
          >
            <span class="text-sm text-muted-foreground">{label}</span><span
              class="min-w-0 max-w-[65%] break-words text-right text-sm font-medium tabular-nums"
              data-selectable>{value}</span
            >
          </div>{/each}
      </Card.Content></Card.Root
    >{/if}
  {#if chart}<section class="space-y-2" aria-label={chartTitle}>
      <h3 class="text-sm font-medium">{chartTitle}</h3>
      {@render chart()}
    </section>{/if}
  {#if detail}{@render detail()}{/if}
  {#if note}<div
      class="space-y-1 rounded-lg border border-border bg-muted/40 p-4 text-sm leading-relaxed"
    >
      {#if noteTitle}<p class="font-medium">{noteTitle}</p>{/if}
      <p class="text-muted-foreground">{note}</p>
    </div>{/if}
</div>
