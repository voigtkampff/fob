<script>
  // Synthetic GPU readings for the engine-list preview, not measured on a GPU.
  import ResourceDetailsContent from "../../src/lib/components/stats/ResourceDetailsContent.svelte";
  import CardDetail from "../../src/lib/components/dashboard/CardDetail.svelte";

  const GB = 1024 ** 3;
  /** @type {import("../../src/lib/types").GpuDevice} */
  const gpu = {
    id: "GPU-preview",
    name: "NVIDIA Example GPU",
    vendor: "NVIDIA",
    utilization: 42,
    memory_used: 1.7 * GB,
    memory_total: 24 * GB,
    memory_kind: "dedicated",
    temperature: 51,
    power_watts: 42.5,
    core_clock_mhz: 1800,
    cores: null,
    source: "nvml-pdh",
    fan_percent: 30,
    engines: [
      ["3D", 42],
      ["Copy", 8],
      ["LegacyOverlay", 0],
      ["OFA_0", 12],
      ["Security_1", 1],
      ["VR", 0],
      ["VideoDecode_1", 55],
      ["VideoEncode", 18],
      ["Compute_0", 3],
    ],
  };
  /** @type {import("../../src/lib/types").SystemStats} */
  const stats = {
    details: {
      identity: {
        cpu_model: "Example CPU",
        physical_cores: 8,
        architecture: "x86_64",
        platform: "windows",
        os_version: "Windows",
        kernel_version: "26100",
      },
      memory_unallocated: 8 * GB,
      swap_total: 0,
      swap_used: 0,
      volumes: [],
      interfaces: [],
    },
    cpu_usage: [1, 2, 3, 4],
    memory_total: 32 * GB,
    memory_used: 14 * GB,
    memory_free: 18 * GB,
    memory_cached: 0,
    uptime: 3600,
    load_avg: [0, 0, 0],
    network_rx_bytes: 0,
    network_tx_bytes: 0,
    disk_total_bytes: 0,
    disk_used_bytes: 0,
    disk_free_bytes: 0,
  };
  /** @type {import("../../src/lib/types").ResourceSample[]} */
  const history = [];
  const lists = { cpu: [], gpu: [], memory: [] };
</script>

<main class="preview">
  <p class="banner">
    Synthetic fixture data, not a real sensor · 模拟数据，不是真实传感器
  </p>
  <section id="wide" style="width:760px">
    <ResourceDetailsContent
      kind="gpu"
      {stats}
      {history}
      isFrozen={false}
      unavailable={false}
      gpus={[gpu]}
      gpuId={gpu.id}
    />
  </section>
  <section id="narrow" style="width:360px">
    <ResourceDetailsContent
      kind="gpu"
      {stats}
      {history}
      isFrozen={false}
      unavailable={false}
      gpus={[gpu]}
      gpuId={gpu.id}
    />
  </section>
  <section id="card" style="width:420px">
    <CardDetail
      kind="gpu"
      systemStats={stats}
      gpus={[gpu]}
      {history}
      {lists}
      onBack={() => {}}
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
    background: var(--background, #111);
    color: var(--foreground, #eee);
  }
  .banner {
    flex: 1 0 100%;
    margin: 0;
    font-size: 13px;
    color: var(--muted-foreground, #888);
  }
</style>
