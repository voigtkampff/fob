<script lang="ts">
  import SearchBox from "./SearchBox.svelte";
  import FilterToggle from "./FilterToggle.svelte";
  import ColumnToggle from "./ColumnToggle.svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import * as ToggleGroup from "$lib/components/ui/toggle-group/index.js";
  import { List, GitBranch, X } from "@lucide/svelte";
  import { t } from "$lib/i18n";
  import { statusLabel } from "$lib/utils/process-status";
  export let searchTerm: string;
  export let columns: {
    id: string;
    label: string;
    visible: boolean;
    required?: boolean;
  }[];
  export let totalProcesses = 0;
  export let matchingProcesses = 0;
  export let viewMode: "flat" | "tree" = "flat";
  export let expandedCount = 0;
  export let onViewChange: (view: "flat" | "tree") => void;
  export let onCollapseAll: () => void;
  export let onClearFilters: () => void;
  export let filters: {
    cpu: { operator: string; value: number; enabled: boolean };
    gpu: { operator: string; value: number; enabled: boolean };
    ram: { operator: string; value: number; enabled: boolean };
    runtime: { operator: string; value: number; enabled: boolean };
    status: { values: string[]; enabled: boolean };
  };
  $: searchActive =
    !!searchTerm.trim() ||
    Object.values(filters).some((filter) => filter.enabled);
  $: activeFilters = [
    ...(searchTerm.trim()
      ? [$t("toolbar.chipSearch", { v: searchTerm.trim() })]
      : []),
    ...(filters.cpu.enabled
      ? [`CPU ${filters.cpu.operator} ${filters.cpu.value}%`]
      : []),
    ...(filters.gpu.enabled
      ? [`GPU ${filters.gpu.operator} ${filters.gpu.value}%`]
      : []),
    ...(filters.ram.enabled
      ? [`${$t("filter.ram")} ${filters.ram.operator} ${filters.ram.value} MB`]
      : []),
    ...(filters.runtime.enabled
      ? [
          `${$t("filter.runtime")} ${filters.runtime.operator} ${filters.runtime.value} ${$t("filter.min")}`,
        ]
      : []),
    ...(filters.status.enabled
      ? [
          $t("toolbar.chipStatus", {
            v: filters.status.values.map((s) => statusLabel(s, $t)).join(", "),
          }),
        ]
      : []),
  ];
</script>

<div class="toolbar shrink-0 border-y border-border bg-card px-4 py-3">
  <div class="flex items-center gap-3">
    <h2 class="mr-1 shrink-0 text-sm font-semibold">
      {$t("toolbar.processes")}
      <span class="ml-1 font-normal text-muted-foreground"
        >{totalProcesses.toLocaleString()}</span
      >
    </h2>
    <SearchBox bind:searchTerm /><FilterToggle bind:filters />
    <div class="ml-auto flex shrink-0 items-center gap-2">
      <ToggleGroup.Root
        type="single"
        value={viewMode}
        onValueChange={(value) => {
          if (value === "flat" || value === "tree") onViewChange(value);
        }}
        variant="outline"
        size="sm"
        aria-label={$t("toolbar.view")}
      >
        <ToggleGroup.Item value="flat" aria-label={$t("toolbar.listView")}
          ><List class="size-3.5" />{$t("toolbar.list")}</ToggleGroup.Item
        >
        <ToggleGroup.Item value="tree" aria-label={$t("toolbar.treeView")}
          ><GitBranch class="size-3.5" />{$t("toolbar.tree")}</ToggleGroup.Item
        >
      </ToggleGroup.Root><ColumnToggle {columns} />
    </div>
  </div>
  {#if searchActive || viewMode === "tree"}
    <div
      class="mt-2 flex flex-wrap items-center gap-2 text-xs text-muted-foreground"
    >
      {#if searchActive}<span
          >{$t("toolbar.matching", {
            n: matchingProcesses.toLocaleString(),
            m: totalProcesses.toLocaleString(),
          })}</span
        >{#each activeFilters as label}<Badge
            variant="secondary"
            class="max-w-64 font-normal"
            ><span class="truncate" title={label}>{label}</span></Badge
          >{/each}<Button
          variant="ghost"
          size="sm"
          class="h-6 px-2 text-xs"
          onclick={onClearFilters}
          ><X class="size-3" />{$t("toolbar.clearAll")}</Button
        >{/if}
      {#if viewMode === "tree"}<span title={$t("toolbar.familyTitle")}
          >{searchActive
            ? $t("toolbar.familySearch")
            : $t("toolbar.families")}</span
        ><Button
          variant="ghost"
          size="sm"
          class="ml-auto h-6 text-xs"
          disabled={searchActive || expandedCount === 0}
          onclick={onCollapseAll}>{$t("toolbar.collapseAll")}</Button
        >{/if}
    </div>
  {/if}
</div>
