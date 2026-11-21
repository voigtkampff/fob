<script lang="ts">
  import type { Snippet } from "svelte";
  import * as Card from "$lib/components/ui/card/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import { getContext } from "svelte";
  let {
    title,
    color = "",
    icon,
    badge = "",
    action,
    value,
    middle,
    band,
  }: {
    title: string;
    color?: string;
    icon: Snippet;
    badge?: string;
    action?: Snippet;
    value: Snippet;
    middle?: Snippet;
    band?: Snippet;
  } = $props();
  const movable = getContext<boolean | undefined>("movable-cards") ?? false;
</script>

<Card.Root size="sm" class="stat-card">
  <div class="sc-head">
    <span class="sc-title"
      >{#if movable}<span class="drag-grip" aria-hidden="true"
        ></span>{/if}<Badge
        variant="secondary"
        class={`sc-icon size-5 shrink-0 rounded-md border-0 p-0 [&>svg]:size-3! ${color ? "bg-primary/10 text-primary" : "bg-muted text-muted-foreground"}`}
        style={color ? `--primary:${color}` : ""}
        aria-hidden="true">{@render icon()}</Badge
      ><span class="truncate" {title}>{title}</span></span
    >
    {#if badge}<Badge
        variant="secondary"
        class="sc-badge shrink-0 text-[10px] font-normal tabular-nums"
        >{badge}</Badge
      >{/if}
    {#if action}{@render action()}{/if}
  </div>
  <div class="sc-value">{@render value()}</div>
  <div class="sc-middle">
    {#if middle}{@render middle()}{/if}
  </div>
  <div class="sc-band">
    {#if band}{@render band()}{/if}
  </div>
</Card.Root>

<style>
  :global(.stat-card[data-slot="card"]) {
    display: grid;
    grid-template-rows: 24px 48px minmax(0, 1fr) 64px;
    row-gap: 8px;
    height: 244px;
    padding: 12px var(--sc-pad, 16px);
    gap: 8px;
    min-width: 0;
    overflow: hidden;
    container-type: inline-size;
  }
  @container (max-width: 150px) {
    .sc-head :global(.sc-icon),
    .sc-head :global(.sc-badge) {
      display: none;
    }
  }
  .sc-head {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }
  .sc-title {
    display: flex;
    flex: 1;
    min-width: 0;
    align-items: center;
    gap: 8px;
    font-size: 14px;
    font-weight: 500;
    color: var(--muted-foreground);
  }
  .sc-value,
  .sc-middle,
  .sc-band {
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }
  .sc-value {
    display: grid;
    grid-template-rows: 32px 16px;
  }
  .sc-value > :global(:first-child) {
    line-height: 32px;
  }
  .sc-value > :global(:first-child *) {
    line-height: 1;
  }
  .sc-value > :global(:nth-child(2)) {
    line-height: 16px;
  }
  .sc-middle {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .sc-band {
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
  }
</style>
