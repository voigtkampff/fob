<script lang="ts">
  import type { Process, Column, ProcessTreeRow } from "$lib/types";
  import {
    columnLayout,
    manualWidths,
    tableWidth,
  } from "$lib/utils/table-layout.js";
  import { previewOrder } from "$lib/utils/layout-order.js";
  import { TableHeader, ProcessRow } from "$lib/components";
  import ProcessMenu from "./ProcessMenu.svelte";
  import {
    locateOutcome,
    procMenuItems,
    revealLocation,
    staleResponse,
  } from "$lib/utils/process-menu.js";
  import type {
    ProcLocationReply,
    ProcMenuLocation,
  } from "$lib/utils/process-menu.js";
  import { invoke } from "@tauri-apps/api/core";
  import { processIdentity } from "$lib/utils/process-tree.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Table from "$lib/components/ui/table/index.js";
  import { Search } from "@lucide/svelte";
  import { t } from "$lib/i18n";
  export let onClearFilters: () => void = () => {};

  export let processes: Process[];
  export let columns: Column[];
  export let systemStats: { memory_total: number } | null;
  export let sortConfig: { field: keyof Process; direction: "asc" | "desc" };
  export let pinnedProcesses: Set<string>;
  export let viewMode: "flat" | "tree" = "flat";
  export let treeRows: ProcessTreeRow[] = [];
  export let searchActive = false;
  export let onToggleExpand: (key: string) => void = () => {};

  export let onToggleSort: (field: keyof Process) => void;
  export let onTogglePin: (command: string) => void;
  export let onShowDetails: (process: Process) => void;
  export let onKillProcess: (process: Process) => void;
  export let onMoveColumn: (
    id: string,
    target: string,
    after: boolean,
  ) => void = () => {};

  export let savedWidths: unknown = {};
  export let onResizeColumn: (id: string, width: number) => void = () => {};
  export let onResetColumn: (id: string) => void = () => {};

  let regionWidth = 0;
  let resizing: { id: string; width: number } | null = null;
  let menu: {
    process: Process;
    x: number;
    y: number;
    row: HTMLElement;
    location: ProcMenuLocation;
    path: string;
  } | null = null;
  let ticket = 0;

  function openMenu(next: {
    process: Process;
    x: number;
    y: number;
    row: HTMLElement;
  }) {
    menu = { ...next, location: { kind: "checking" }, path: "" };
    ticket += 1;
    void locate(next.process, ticket);
  }

  function closeMenu() {
    menu = null;
    ticket += 1;
  }

  async function locate(process: Process, mine: number) {
    let reply: ProcLocationReply;
    try {
      reply = await invoke<ProcLocationReply>("process_location", {
        pid: process.pid,
        startTime: process.start_time,
      });
    } catch {
      reply = { kind: "unreadable" };
    }
    if (staleResponse(mine, ticket) || !menu) return;
    menu = { ...menu, ...locateOutcome(reply) };
  }

  async function openLocation(process: Process) {
    const outcome = await revealLocation(menu, (path) =>
      invoke("space_reveal", { path }),
    );
    if (menu && outcome.location)
      menu = { ...menu, location: outcome.location };
    return !outcome.keepOpen;
  }

  async function copyText(text: string) {
    try {
      await navigator.clipboard.writeText(text);
    } catch {}
  }
  $: menuCtx = {
    onShowDetails,
    onTogglePin,
    onKillProcess,
    copyText,
    onOpenLocation: openLocation,
  };
  $: tree = viewMode === "tree";
  $: manual = {
    ...manualWidths(
      savedWidths,
      columns.map((c) => c.id),
    ),
    ...(resizing ? { [resizing.id]: resizing.width } : {}),
  };
  $: ({ widths, filler } = columnLayout(columns, tree, regionWidth, manual));
  let move: { id: string; target: string; after: boolean } | null = null;
  $: shownColumns = previewOrder(
    columns.map((c) => c.id),
    move,
  ).map((id) => columns.find((c) => c.id === id)!);

  $: rows =
    viewMode === "tree"
      ? treeRows
      : processes.map((process) => ({
          process,
          key: processIdentity(process),
          depth: 0,
          childCount: 0,
          expanded: false,
          rootIndex: 0,
          familyCpuUsage: process.cpu_usage,
          familyMemoryUsage: process.memory_usage,
          familyProcessCount: 1,
        }));
</script>

<section
  class="flex min-h-0 min-w-0 flex-1 flex-col"
  aria-label={$t("toolbar.processes")}
>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex a11y_no_noninteractive_element_interactions (The labeled scroll region supports WebKit keyboard scrolling.) -->
  <div
    class="process-table-region focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-ring"
    role="region"
    aria-label={$t("proc.tableAria")}
    tabindex="0"
    bind:clientWidth={regionWidth}
    onkeydown={(event) => {
      if (
        event.target === event.currentTarget &&
        (event.key === "ArrowLeft" || event.key === "ArrowRight")
      ) {
        event.preventDefault();
        event.currentTarget.scrollBy({
          left: event.key === "ArrowRight" ? 160 : -160,
        });
      }
    }}
  >
    <Table.Root style={`width:max(100%,${tableWidth(columns, tree, manual)}px)`}
      ><TableHeader
        columns={shownColumns}
        {sortConfig}
        {onToggleSort}
        {onMoveColumn}
        {widths}
        {filler}
        onResizePreview={(next) => (resizing = next)}
        {onResizeColumn}
        {onResetColumn}
        onPreviewColumn={(next) => (move = next)}
        treeView={tree}
      /><Table.Body
        >{#each rows as row (row.key)}<ProcessRow
            process={row.process}
            columns={shownColumns}
            tree={viewMode === "tree" ? row : null}
            expansionLocked={searchActive}
            {onToggleExpand}
            isPinned={pinnedProcesses.has(row.process.command)}
            isHighUsage={((viewMode === "tree"
              ? row.familyCpuUsage
              : row.process.cpu_usage) ?? 0) > 50 ||
              (viewMode === "tree"
                ? row.familyMemoryUsage
                : row.process.memory_usage) /
                (systemStats?.memory_total || 0) >
                0.1}
            {onTogglePin}
            {onShowDetails}
            {onKillProcess}
            onRowMenu={openMenu}
          />{:else}<Table.Row
            ><Table.Cell
              colspan={columns.filter((c) => c.visible).length + 2}
              class="h-48 text-center"
              ><Search class="mx-auto mb-3 size-6 text-muted-foreground" />
              <p class="text-sm font-medium">
                {searchActive ? $t("proc.noMatch") : $t("proc.none")}
              </p>
              <p class="mt-1 text-xs text-muted-foreground">
                {searchActive ? $t("proc.noMatchHint") : $t("proc.noneHint")}
              </p>
              {#if searchActive}<Button
                  variant="link"
                  size="sm"
                  onclick={onClearFilters}>{$t("proc.clearFilters")}</Button
                >{/if}</Table.Cell
            ></Table.Row
          >{/each}</Table.Body
      ></Table.Root
    >
  </div>
  {#if menu}<ProcessMenu
      items={procMenuItems(menu.process, {
        pinned: pinnedProcesses.has(menu.process.command),
        location: menu.location,
      })}
      process={menu.process}
      anchor={menu}
      ctx={menuCtx}
      onClose={closeMenu}
    />{/if}
</section>
