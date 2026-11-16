<script lang="ts">
  // Fob: one hover hint for the whole app. It draws the same dark bubble the
  // CPU voltage rows use (bits-ui tooltip), instead of the WebView's native
  // yellow `title` box, so one window never shows two kinds of hint.
  //
  // The trigger is the caller's own element, handed back through `children`
  // with the props it needs. Nothing extra is added to the document, so grids,
  // tables and scoped styles keep working. A blank `text` renders the element
  // bare, with no tooltip machinery at all. Use it on text, not on buttons or
  // inputs, which keep their native hint.
  import type { Snippet } from "svelte";
  import * as Tooltip from "$lib/components/ui/tooltip/index.js";

  let {
    text = "",
    tabindex = 0,
    children,
  }: {
    /** The hint. Empty, null or blank renders the element bare. */
    text?: string | null;
    /**
     * `-1` keeps a hint on a value cell out of the tab order: a grid of 32
     * cells or a table of rows would otherwise bury real controls. Hover still
     * opens it.
     */
    tabindex?: number;
    /** The element to attach the hint to, written as it would appear alone. */
    children: Snippet<[Record<string, unknown>]>;
  } = $props();

  const hint = $derived(typeof text === "string" ? text.trim() : "");

  // bits-ui hands every trigger `type="button"` and `disabled: false`. A span,
  // a <p> or a <div> has no business carrying either, so they go. Everything
  // else it adds (the id, aria-describedby, the pointer and focus handlers)
  // is what makes the hint open.
  const triggerProps = (props: Record<string, unknown>) => {
    const { type: _type, disabled: _disabled, ...rest } = props;
    return rest;
  };
</script>

{#if hint}
  <Tooltip.Provider delayDuration={300}>
    <Tooltip.Root>
      <Tooltip.Trigger {tabindex}>
        {#snippet child({ props })}
          {@render children(triggerProps(props))}
        {/snippet}
      </Tooltip.Trigger>
      <Tooltip.Content
        side="top"
        align="start"
        sideOffset={6}
        collisionPadding={8}>{hint}</Tooltip.Content
      >
    </Tooltip.Root>
  </Tooltip.Provider>
{:else}
  {@render children({})}
{/if}
