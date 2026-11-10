<script>
  // Mocked disk health in the real Disk detail, not read on Windows.
  import CardDetail from "../../src/lib/components/dashboard/CardDetail.svelte";
  import { miniStyle } from "../../src/lib/utils/mini-layout.js";

  /** @type {{ width?: number }} */
  let { width = 980 } = $props();
  const GB = 1024 ** 3;
  /** @param {string} mount @param {string} name @param {number} total @param {number} available */
  const volume = (mount, name, total, available) => ({
    name,
    mount,
    file_system: "NTFS",
    total: total * GB,
    available: available * GB,
    read_only: false,
    read: 0,
    written: 0,
  });
  /** @type {import("../../src/lib/types").SystemStats} */
  const stats = {
    details: {
      identity: {
        cpu_model: "Example CPU",
        physical_cores: 8,
        architecture: "x86_64",
        platform: "windows",
        os_version: "Windows 11",
        kernel_version: "26100",
      },
      memory_unallocated: 0,
      swap_total: 0,
      swap_used: 0,
      volumes: [
        volume("C:\\", "Windows", 1000, 420),
        volume("D:\\", "Data", 860, 610),
        volume("E:\\", "Games", 930, 200),
      ],
      interfaces: [],
    },
    cpu_usage: [10, 20],
    memory_total: 32 * GB,
    memory_used: 14 * GB,
    memory_free: 18 * GB,
    memory_cached: 0,
    uptime: 3600,
    load_avg: [0, 0, 0],
    network_rx_bytes: 0,
    network_tx_bytes: 0,
    disk_total_bytes: 2790 * GB,
    disk_used_bytes: 1560 * GB,
    disk_free_bytes: 1230 * GB,
  };
  const lists = { cpu: [], gpu: [], memory: [] };
</script>

<main class="preview">
  <section id="detail" style={`width:${width}px`}>
    <CardDetail kind="disk" systemStats={stats} {lists} onBack={() => {}} />
  </section>
  <section id="mini" style={`width:360px;${miniStyle()}`}>
    <CardDetail
      kind="disk"
      compact
      systemStats={stats}
      {lists}
      onBack={() => {}}
    />
  </section>
</main>

<style>
  .preview {
    display: flex;
    gap: 24px;
    align-items: flex-start;
    padding: 16px;
    background: var(--background);
    color: var(--foreground);
  }
</style>
