<script lang="ts">
  import { dragReorder } from "$lib/utils/drag-reorder.js";
  import type { Process, Column } from "$lib/types";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Table from "$lib/components/ui/table/index.js";
  import { ArrowDown, ArrowUp, ArrowDownUp } from "@lucide/svelte";
  import { t } from "$lib/i18n";
  import { clampManual } from "$lib/utils/table-layout.js";
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
  /** Empty room before the actions column (columnLayout). */
  export let filler = 0;
  /** A column edge drag in progress (null when it ends or is cancelled). */
  export let onResizePreview: (
    resize: { id: string; width: number } | null,
  ) => void = () => {};
  /** Release saves the column's new width once. */
  export let onResizeColumn: (id: string, width: number) => void = () => {};
  /** A double click on the edge gives the column back its own width. */
  export let onResetColumn: (id: string) => void = () => {};
  $: shown = columns.filter((c) => c.visible);
  let row: HTMLTableRowElement | undefined;
  let resizing = "";

  /** Drag a column's right edge: the width follows the pointer, release
   * saves it, Escape puts it back. The edge is outside the title, so it never
   * sorts or reorders. */
  function resize(e: PointerEvent, id: string) {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    const edge = e.currentTarget as HTMLElement;
    const startX = e.clientX;
    const startWidth = widths[id];
    let width = startWidth;
    resizing = id;
    document.documentElement.dataset.resizing = "";
    edge.setPointerCapture(e.pointerId);
    const move = (m: PointerEvent) => {
      width = clampManual(startWidth + m.clientX - startX);
      onResizePreview({ id, width });
    };
    const finish = (save: boolean) => {
      edge.removeEventListener("pointermove", move);
      edge.removeEventListener("pointerup", up);
      edge.removeEventListener("pointercancel", cancel);
      window.removeEventListener("keydown", escape, true);
      resizing = "";
      delete document.documentElement.dataset.resizing;
      onResizePreview(null);
      if (save && width !== startWidth) onResizeColumn(id, width);
    };
    const up = () => finish(true);
    const cancel = () => finish(false);
    const escape = (k: KeyboardEvent) => {
      if (k.key !== "Escape") return;
      k.preventDefault();
      k.stopPropagation();
      finish(false);
    };
    edge.addEventListener("pointermove", move);
    edge.addEventListener("pointerup", up);
    edge.addEventListener("pointercancel", cancel);
    window.addEventListener("keydown", escape, true);
  }
</script>

<Table.Header
  ><!-- The name column stays first; the actions cell is not a column of data. -->
  <tr
    data-slot="table-row"
    class="border-b"
    bind:this={row}
    use:dragReorder={{
      ids: [...shown.map((c) => (c.id === "name" ? null : c.id)), null, null],
      onMove: onMoveColumn,
      onPreview: onPreviewColumn,
      handle: "button",
      cells: (id) => [
        ...(row
          ?.closest("table")
          ?.querySelectorAll<HTMLElement>(`tbody td[data-col="${id}"]`) ?? []),
      ],
    }}
    >{#each shown as column (column.id)}<Table.Head
        class={`${column.numeric ? "text-right" : ""} ${treeView && column.id === "name" ? "tree-name" : ""} ${column.id === "name" ? "pl-4" : ""}`}
        style={`width:${widths[column.id]}px`}
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
        ><span
          class="col-resize"
          aria-hidden="true"
          data-active={resizing === column.id ? "" : undefined}
          onpointerdown={(e) => resize(e, column.id)}
          ondblclick={(e) => {
            e.stopPropagation();
            onResetColumn(column.id);
          }}
        ></span></Table.Head
      >{/each}<Table.Head
      class="col-filler p-0"
      aria-hidden="true"
      style={`width:${filler}px`}
    ></Table.Head><Table.Head class="actions-heading w-28 pr-4 text-right"
      >{$t("col.actions")}</Table.Head
    ></tr
  ></Table.Header
>
