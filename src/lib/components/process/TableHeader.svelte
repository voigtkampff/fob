<script lang="ts">
  import { dragReorder } from "$lib/utils/drag-reorder.js";
  import type { Process, Column } from "$lib/types";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Table from "$lib/components/ui/table/index.js";
  import { ArrowDown, ArrowUp, ArrowDownUp } from "@lucide/svelte";
  import { t } from "$lib/i18n";
  export let columns: Column[];
  export let sortConfig: { field: keyof Process; direction: "asc" | "desc" };
  export let onToggleSort: (field: keyof Process) => void;
  export let treeView = false;
  /** Each visible column's width (columnWidths). */
  export let widths: Record<string, number>;
  export let onMoveColumn: (id: string, target: string, after: boolean) => void;
  export let onPreviewColumn: (
    move: { id: string; target: string; after: boolean } | null,
  ) => void;
  export let onStepColumn: (id: string, step: -1 | 1) => void;
  $: shown = columns.filter((c) => c.visible);
  let row: HTMLTableRowElement | undefined;
  const MOVE_KEYS = "Alt+ArrowLeft Alt+ArrowRight"; // i18n-ignore key names
</script>

<Table.Header
  ><!-- The name column stays first; the actions cell is not a column of data. -->
  <tr
    data-slot="table-row"
    class="border-b"
    bind:this={row}
    use:dragReorder={{
      ids: [...shown.map((c) => (c.id === "name" ? null : c.id)), null],
      onMove: onMoveColumn,
      onPreview: onPreviewColumn,
      onStep: onStepColumn,
      cells: (id) => [
        ...(row
          ?.closest("table")
          ?.querySelectorAll<HTMLElement>(`tbody td[data-col="${id}"]`) ?? []),
      ],
    }}
    >{#each shown as column (column.id)}<Table.Head
        class={`${column.numeric ? "text-right" : ""} ${treeView && column.id === "name" ? "tree-name" : ""} ${column.id === "name" ? "pl-4" : ""}`}
        style={`width:${widths[column.id]}px`}
        data-movable={column.id === "name" ? undefined : ""}
        aria-sort={sortConfig.field === column.id
          ? sortConfig.direction === "asc"
            ? "ascending"
            : "descending"
          : "none"}
        ><Button
          variant="ghost"
          size="sm"
          class={`-mx-1 h-8 max-w-[calc(100%+0.5rem)] px-1 text-xs ${sortConfig.field === column.id ? "text-primary" : "text-muted-foreground"}`}
          onclick={() => onToggleSort(column.id)}
          aria-keyshortcuts={column.id === "name" ? undefined : MOVE_KEYS}
          title={`${
            treeView && column.id === "memory_usage"
              ? $t("col.familyRamTitle")
              : treeView && column.id === "cpu_usage"
                ? $t("col.familyCpuTitle")
                : $t("col.sortBy", { v: $t(`col.${column.id}`) })
          }${column.id === "name" ? "" : `\n${$t("col.moveHint")}`}`}
          ><span
            >{treeView && column.id === "cpu_usage"
              ? $t("col.familyCpu")
              : treeView && column.id === "memory_usage"
                ? $t("col.familyRam")
                : $t(`col.${column.id}`)}</span
          >{#if sortConfig.field === column.id}{#if sortConfig.direction === "asc"}<ArrowUp
                class="size-3"
              />{:else}<ArrowDown class="size-3" />{/if}{:else}<ArrowDownUp
              class="size-3 opacity-50"
            />{/if}</Button
        ></Table.Head
      >{/each}<Table.Head class="actions-heading w-28 pr-4 text-right"
      >{$t("col.actions")}</Table.Head
    ></tr
  ></Table.Header
>
