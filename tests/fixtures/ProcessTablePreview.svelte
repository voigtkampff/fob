<script>
  import ProcessTable from "../../src/lib/components/process/ProcessTable.svelte";
  import { column_definitions } from "../../src/lib/definitions/columns";
  import { sortProcesses } from "../../src/lib/utils";
  import {
    buildProcessTree,
    processIdentity,
  } from "../../src/lib/utils/process-tree.js";
  import { settingsStore } from "../../src/lib/stores/settings";

  const MB = 1024 * 1024;
  // ?view=tree shows the tree view, ?wide adds the optional columns and a
  // row with the longest value each column can show.
  const params = new URLSearchParams(location.search);
  const viewMode = params.get("view") === "tree" ? "tree" : "flat";
  const wide = params.has("wide");
  const wideColumns = new Set(["gpu_memory", "ppid", "command"]);
  const now = Math.floor(Date.now() / 1000);
  // Mocked readings, not measured on Windows. null is "no reading".
  const base = {
    status: "Running",
    user: "S-1-5-21",
    threads: 0,
    environ: [],
    root: "",
    session_id: 1,
    ports: [],
    ppid: 4,
  };
  const processes = [
    {
      pid: 9120,
      name: "Game.exe",
      cpu_usage: 18.2,
      memory_usage: 3100 * MB,
      virtual_memory: 6.2e9,
      gpu_usage: 87.4,
      gpu_memory: 5400 * MB,
      disk_read: 12.6 * MB,
      disk_write: 0.4 * MB,
    },
    {
      pid: 4412,
      name: "chrome.exe",
      cpu_usage: 3.1,
      memory_usage: 640 * MB,
      virtual_memory: 2.1e9,
      gpu_usage: 12.5,
      gpu_memory: 310 * MB,
      disk_read: 180 * 1024,
      disk_write: 2.3 * MB,
    },
    {
      pid: 3020,
      name: "MsMpEng.exe",
      cpu_usage: 6.4,
      memory_usage: 210 * MB,
      virtual_memory: 1.1e9,
      gpu_usage: 0,
      gpu_memory: 0,
      disk_read: 35.2 * MB,
      disk_write: 64 * 1024,
    },
    {
      pid: 7788,
      name: "OneDrive.exe",
      cpu_usage: 0.4,
      memory_usage: 120 * MB,
      virtual_memory: 0.8e9,
      gpu_usage: 0,
      gpu_memory: 0,
      disk_read: 0,
      disk_write: 9.8 * MB,
    },
    {
      pid: 2200,
      name: "explorer.exe",
      cpu_usage: 0.2,
      memory_usage: 150 * MB,
      virtual_memory: 1.4e9,
      gpu_usage: 0.8,
      gpu_memory: 48 * MB,
      disk_read: 0,
      disk_write: 0,
    },
    {
      pid: 15004,
      name: "setup.exe",
      cpu_usage: 1.5,
      memory_usage: 30 * MB,
      virtual_memory: 0.2e9,
      gpu_usage: 0,
      gpu_memory: 0,
      disk_read: null,
      disk_write: null,
    },
    {
      pid: 4,
      name: "System",
      cpu_usage: 0.3,
      memory_usage: 0.1 * MB,
      virtual_memory: 0,
      gpu_usage: 0,
      gpu_memory: 0,
      disk_read: null,
      disk_write: null,
    },
    ...(wide
      ? [
          {
            pid: 1234567,
            ppid: 9120,
            name: "LongestValues.exe",
            cpu_usage: 100,
            memory_usage: 24575.9 * MB,
            virtual_memory: 1023.9 * 1024 * MB,
            gpu_usage: 100,
            gpu_memory: 24575 * MB,
            disk_read: 1023.9 * 1024,
            disk_write: 1023.9 * MB,
            run_time: 9999 * 3600 + 59 * 60 + 59,
          },
        ]
      : []),
  ].map((p, i) => ({
    ...base,
    command: p.name,
    start_time: now - 600 * (i + 1),
    run_time: 600 * (i + 1),
    ...p,
  }));
  /** @type {import("../../src/lib/types").SortConfig} */
  let sortConfig = $state({ field: "cpu_usage", direction: "desc" });
  const columns = $derived(
    column_definitions.map((col) => ({
      ...col,
      visible:
        col.required ||
        (wide && wideColumns.has(col.id)) ||
        ($settingsStore.appearance.columnVisibility[col.id] ?? col.visible),
    })),
  );
  const sorted = $derived(sortProcesses(processes, sortConfig, new Set()));
  const treeRows = $derived(
    viewMode === "tree"
      ? buildProcessTree(
          sorted,
          new Set(processes.map(processIdentity)),
          null,
          {
            sortConfig,
          },
        ).rows
      : [],
  );
</script>

<p class="preview-note">
  Local preview with mocked readings, not a Windows run
</p>
<div class="preview-main">
  <ProcessTable
    processes={sorted}
    {columns}
    {viewMode}
    {treeRows}
    systemStats={{ memory_total: 32 * 1024 * MB }}
    {sortConfig}
    pinnedProcesses={new Set()}
    onToggleSort={(field) =>
      (sortConfig =
        sortConfig.field === field
          ? {
              field,
              direction: sortConfig.direction === "asc" ? "desc" : "asc",
            }
          : { field, direction: "desc" })}
    onTogglePin={() => {}}
    onShowDetails={() => {}}
    onKillProcess={() => {}}
  />
</div>

<style>
  .preview-note {
    font: 12px system-ui;
    padding: 6px 20px;
    margin: 0;
    color: #a00;
  }
  .preview-main {
    display: flex;
    flex-direction: column;
    height: calc(100vh - 30px);
    min-width: 0;
  }
</style>
