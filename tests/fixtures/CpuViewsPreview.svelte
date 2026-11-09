<script>
  // Mocked CPU readings for the CPU views, not measured on Windows.
  import Dashboard from "../../src/lib/components/dashboard/Dashboard.svelte";
  import CardDetail from "../../src/lib/components/dashboard/CardDetail.svelte";
  import CpuPanel from "../../src/lib/components/stats/CpuPanel.svelte";
  import ResourceDetailsContent from "../../src/lib/components/stats/ResourceDetailsContent.svelte";
  import { miniStyle } from "../../src/lib/utils/mini-layout.js";

  /** @type {{ cpu?: string }} */
  let { cpu = "intel" } = $props();
  const GB = 1024 ** 3;
  /** @param {number} a @param {number} b */
  const range = (a, b) => Array.from({ length: b - a }, (_, i) => a + i);
  // Deterministic loads: P-side busy (a game), E-side light background work.
  /** @param {number} i @param {boolean} busy */
  const load = (i, busy) => (busy ? 62 + ((i * 37) % 30) : 4 + ((i * 13) % 9));
  /** @type {Record<string, { model: string, physical: number, usage: number[], topology: import("../../src/lib/types").CpuTopology }>} */
  const SCENARIOS = {
    intel: {
      model: "Example Hybrid CPU",
      physical: 16,
      usage: range(0, 24).map((i) => load(i, i < 16)),
      topology: {
        state: "hybrid",
        vendor: "intel",
        groups: [
          { kind: "performance", cores: 8, threads: range(0, 16) },
          { kind: "efficiency", cores: 8, threads: range(16, 24) },
        ],
      },
    },
    amd: {
      model: "AMD Ryzen AI 9 HX 370 w/ Radeon 890M",
      physical: 12,
      usage: range(0, 24).map((i) => load(i, i < 8)),
      topology: {
        state: "hybrid",
        vendor: "amd",
        groups: [
          { kind: "performance", cores: 4, threads: range(0, 8) },
          { kind: "efficiency", cores: 8, threads: range(8, 24) },
        ],
      },
    },
    uniform: {
      model: "Intel(R) Core(TM) i5-8250U CPU @ 1.60GHz",
      physical: 4,
      usage: range(0, 8).map((i) => load(i, i < 3)),
      topology: { state: "uniform" },
    },
    unknown: {
      model: "Example Hybrid CPU",
      physical: 16,
      usage: range(0, 24).map((i) => load(i, i < 16)),
      topology: { state: "unknown" },
    },
    noreading: {
      model: "Example Hybrid CPU",
      physical: 16,
      usage: range(0, 24).map((i) => (i < 16 ? load(i, true) : NaN)),
      topology: {
        state: "hybrid",
        vendor: "intel",
        groups: [
          { kind: "performance", cores: 8, threads: range(0, 16) },
          { kind: "efficiency", cores: 8, threads: range(16, 24) },
        ],
      },
    },
  };
  // The scenario is fixed for the page's lifetime (one per URL).
  // svelte-ignore state_referenced_locally
  const s = SCENARIOS[cpu] ?? SCENARIOS.intel;
  /** @type {import("../../src/lib/types").SystemStats} */
  const stats = {
    details: {
      identity: {
        cpu_model: s.model,
        physical_cores: s.physical,
        architecture: "x86_64",
        platform: "windows",
        os_version: "Windows 11 Pro 24H2",
        kernel_version: "26100",
      },
      memory_unallocated: 8 * GB,
      swap_total: 4 * GB,
      swap_used: 1 * GB,
      volumes: [
        {
          name: "Windows",
          mount: "C:\\",
          file_system: "NTFS",
          total: 1000 * GB,
          available: 420 * GB,
          read_only: false,
          read: 0,
          written: 0,
        },
      ],
      interfaces: [],
    },
    cpu_usage: s.usage,
    cpu_topology: s.topology,
    memory_total: 32 * GB,
    memory_used: 14 * GB,
    memory_free: 18 * GB,
    memory_cached: 0,
    uptime: 3600 * 5,
    load_avg: [0, 0, 0],
    network_rx_bytes: 120000,
    network_tx_bytes: 30000,
    disk_total_bytes: 1000 * GB,
    disk_used_bytes: 580 * GB,
    disk_free_bytes: 420 * GB,
    disk_read_bytes: 0,
    disk_write_bytes: 0,
  };
  /** @type {import("../../src/lib/types").ResourceSample[]} */
  const history = [];
  const processes = [
    { pid: 9120, name: "Game.exe", cpu_usage: 31.2, memory_usage: 3.1 * GB },
    { pid: 4412, name: "chrome.exe", cpu_usage: 3.1, memory_usage: 0.6 * GB },
  ];
  const lists = { cpu: processes, gpu: [], memory: processes };
</script>

<main class="preview">
  <section id="dashboard" style="width:980px">
    <Dashboard systemStats={stats} {history} {processes} />
  </section>
  <section id="mini" style={`width:360px;${miniStyle()}`}>
    <Dashboard compact systemStats={stats} {history} {processes} />
  </section>
  <section id="cards" class="cards">
    <div id="card-narrow" style="width:144px">
      <CpuPanel {stats} cpuUsage={s.usage} {history} />
    </div>
    <div id="card-wide" style="width:200px">
      <CpuPanel {stats} cpuUsage={s.usage} {history} />
    </div>
  </section>
  <section id="detail" style="width:560px">
    <CardDetail
      kind="cpu"
      systemStats={stats}
      {history}
      {lists}
      onBack={() => {}}
    />
  </section>
  <section id="mini-detail" style={`width:360px;${miniStyle()}`}>
    <CardDetail
      compact
      kind="cpu"
      systemStats={stats}
      {history}
      {lists}
      onBack={() => {}}
    />
  </section>
  <section id="resource" style="width:760px">
    <ResourceDetailsContent
      kind="cpu"
      {stats}
      {history}
      isFrozen={false}
      unavailable={false}
    />
  </section>
</main>

<style>
  .preview {
    display: flex;
    flex-wrap: wrap;
    gap: 24px;
    padding: 16px;
    align-items: flex-start;
  }
  .cards {
    display: flex;
    gap: 12px;
  }
</style>
