<script>
  // Mocked disk health in the real Disk detail, not read on Windows.
  import CardDetail from "../../src/lib/components/dashboard/CardDetail.svelte";
  import { miniStyle } from "../../src/lib/utils/mini-layout.js";
  import { stats } from "./disk-health-fixtures.js";

  /** @type {{ width?: number }} */
  let { width = 980 } = $props();
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
