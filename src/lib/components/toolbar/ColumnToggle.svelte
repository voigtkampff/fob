<script lang="ts">
  import { settingsStore } from "$lib/stores";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as DropdownMenu from "$lib/components/ui/dropdown-menu/index.js";
  import { Columns3, ChevronDown } from "@lucide/svelte";
  import { t } from "$lib/i18n";
  export let columns: {
    id: string;
    label: string;
    visible: boolean;
    required?: boolean;
  }[];
  function change(id: string, visible: boolean) {
    settingsStore.updateConfig({
      appearance: {
        ...$settingsStore.appearance,
        columnVisibility: {
          ...$settingsStore.appearance.columnVisibility,
          [id]: visible,
        },
      },
    });
  }
</script>

<DropdownMenu.Root
  ><DropdownMenu.Trigger
    >{#snippet child({ props })}<Button
        {...props}
        variant="ghost"
        size="sm"
        aria-label={$t("col.viewOptions")}
        ><Columns3 />{$t("col.view")}<ChevronDown class="size-3" /></Button
      >{/snippet}</DropdownMenu.Trigger
  ><DropdownMenu.Content align="end" class="max-h-[70vh] w-60 overflow-y-auto"
    ><DropdownMenu.Label>{$t("col.visible")}</DropdownMenu.Label
    ><DropdownMenu.Separator
    />{#each columns as column}<DropdownMenu.CheckboxItem
        checked={column.visible}
        disabled={column.required}
        onCheckedChange={(value) => change(column.id, value)}
        >{$t(`col.${column.id}`)}{#if column.required}<span
            class="ml-auto text-[10px] text-muted-foreground"
            >{$t("col.required")}</span
          >{/if}</DropdownMenu.CheckboxItem
      >{/each}</DropdownMenu.Content
  ></DropdownMenu.Root
>
