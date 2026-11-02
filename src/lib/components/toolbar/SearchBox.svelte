<script lang="ts">
  import { onMount } from "svelte";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Popover from "$lib/components/ui/popover/index.js";
  import { Search, X, CircleHelp } from "@lucide/svelte";
  import { t } from "$lib/i18n";
  export let searchTerm: string;
  let input: HTMLInputElement | null = null;
  let open = false;
  // Example queries are literal search text, the same in every language.
  $: examples = [
    ["chrome, msedge", $t("search.exMulti")], // i18n-ignore
    ["host$", $t("search.exEnd")],
    ["^svc", $t("search.exStart")],
    ["nvidia.*container", $t("search.exRegex")],
    ["1234", $t("search.exPid")],
    [":3000", $t("search.exPort")],
    ["port:443", $t("search.exPortExplicit")],
    ["python, node, java", $t("search.exRuntimes")], // i18n-ignore
    ["explorer, dwm", $t("search.exShell")], // i18n-ignore
  ];
  function useExample(query: string) {
    searchTerm = query;
    open = false;
    requestAnimationFrame(() => input?.focus());
  }
  onMount(() => {
    const key = (event: KeyboardEvent) => {
      if (
        (event.metaKey || event.ctrlKey) &&
        event.key.toLowerCase() === "f" &&
        !document.querySelector('[role="dialog"],[role="alertdialog"]')
      ) {
        event.preventDefault();
        input?.focus();
      }
    };
    document.addEventListener("keydown", key);
    return () => document.removeEventListener("keydown", key);
  });
</script>

<div class="flex min-w-56 max-w-md flex-1 items-center gap-1">
  <div class="relative min-w-0 flex-1">
    <Search
      class="pointer-events-none absolute left-3 top-2.5 size-4 text-muted-foreground"
    /><Input
      bind:ref={input}
      bind:value={searchTerm}
      aria-label={$t("search.aria")}
      placeholder={$t("search.placeholder")}
      class="h-9 pl-9 pr-9"
      onkeydown={(event) => {
        if (event.key === "Escape") {
          searchTerm = "";
          event.preventDefault();
        }
      }}
    />{#if searchTerm}<Button
        variant="ghost"
        size="icon-xs"
        class="absolute right-1.5 top-1.5 text-muted-foreground"
        aria-label={$t("search.clear")}
        onclick={() => {
          searchTerm = "";
          input?.focus();
        }}><X /></Button
      >{/if}
  </div>
  <Popover.Root bind:open
    ><Popover.Trigger
      >{#snippet child({ props })}<Button
          {...props}
          variant="ghost"
          size="icon-sm"
          aria-label={$t("search.help")}
          ><CircleHelp class="size-4 text-muted-foreground" /></Button
        >{/snippet}</Popover.Trigger
    ><Popover.Content
      align="start"
      class="max-h-[70vh] w-80 overflow-y-auto space-y-4"
      ><div>
        <h2 class="text-sm font-medium">{$t("search.title")}</h2>
        <p class="mt-1 text-xs text-muted-foreground">
          {$t("search.note")}
        </p>
      </div>
      <div class="space-y-1">
        {#each examples as [query, label]}<Button
            variant="ghost"
            class="h-auto w-full items-start justify-start gap-3 whitespace-normal py-2 text-left"
            onclick={() => useExample(query)}
            ><code class="shrink-0 text-xs text-primary">{query}</code><span
              class="text-xs font-normal text-muted-foreground">{label}</span
            ></Button
          >{/each}
      </div>
      <p class="border-t border-border pt-3 text-xs text-muted-foreground">
        {$t("search.keys")}
      </p></Popover.Content
    ></Popover.Root
  >
</div>
