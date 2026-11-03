<script lang="ts">
  // Fob: a detail card in a side panel (per-core, interfaces, drives, …):
  // title + optional description, a count or button on the right, body.
  import type { Snippet } from "svelte";
  import * as Card from "$lib/components/ui/card/index.js";
  let {
    title,
    description = "",
    aside = "",
    action,
    children,
  }: {
    title: string;
    description?: string;
    aside?: string;
    action?: Snippet;
    children: Snippet;
  } = $props();
</script>

<Card.Root size="sm"
  ><Card.Header class="flex items-start justify-between gap-3"
    ><div class="min-w-0 space-y-1">
      <Card.Title class="text-sm">{title}</Card.Title>
      {#if description}<Card.Description>{description}</Card.Description>{/if}
    </div>
    {#if action}{@render action()}{:else if aside}<span
        class="shrink-0 text-xs text-muted-foreground tabular-nums"
        >{aside}</span
      >{/if}</Card.Header
  ><Card.Content class="flex flex-col gap-3">{@render children()}</Card.Content
  ></Card.Root
>
