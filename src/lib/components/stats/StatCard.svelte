<script lang="ts">
  // Fob: the one layout every card in the process tab's stats row uses, so
  // the rows line up across cards: header / value / middle (fixed height) /
  // bottom band (fixed height, its caption on the card's last line).
  import type { Snippet } from "svelte";
  import * as Card from "$lib/components/ui/card/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
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
</script>

<Card.Root size="sm" class="stat-card">
  <div class="sc-head">
    <span class="sc-title"
      ><Badge
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
  /* Narrow cards (seven across a 1024 px window): the title keeps the room;
     the icon and the count badge step aside. */
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
  /* The value block has one set of metrics in every card: a 32 px value
     line and a 16 px caption line. Units inside the value (%, °C, GB) take
     line-height 1 so their own line box can never make the line taller and
     push the caption out of the row (it is clipped by overflow there). */
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
  /* The band's content sits on the card's bottom edge, so every caption
     shares one baseline. */
  .sc-band {
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
  }
</style>
