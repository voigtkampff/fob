<script lang="ts">
  // Fob: disk space map page of the main window. Nothing is scanned until a
  // drive is opened without a saved result, or Scan / Rescan is pressed; a
  // saved result shows at once with its time.
  import { onDestroy, onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { Button } from "$lib/components/ui/button/index.js";
  import {
    ArrowLeft,
    ArrowRight,
    ChevronRight,
    FolderSearch,
    RefreshCw,
    Square,
    X,
  } from "@lucide/svelte";
  import SpaceMap from "./SpaceMap.svelte";
  import { revealMessage } from "$lib/utils/space-reveal.js";
  import OtherList from "./OtherList.svelte";
  import {
    folderAt,
    joinPath,
    tileColor,
    type SpaceNode,
    type Block,
  } from "$lib/utils/treemap.js";
  import { formatBytes } from "$lib/utils";
  import { emptyHistory, visit, step } from "$lib/utils/nav-history.js";
  import { t, locale } from "$lib/i18n";

  type Drive = { root: string; total: number; free: number };
  type Scan = {
    root: string;
    sep: string;
    scannedAt: number;
    method?: "mft-helper" | "mft" | "walk";
    elapsedMs: number;
    files: number;
    skipped: number;
    tree: SpaceNode;
  };
  type Status = {
    root: string | null;
    files: number;
    bytes: number;
    skipped: number;
    current: string;
    elapsedMs: number;
    ended: [string, boolean] | null;
  };
  const LAST = "fob.space.root";
  const LEGEND = $derived([
    ["folder", $t("space.kind.folder")],
    ["other", $t("space.kind.other")],
    ["video", $t("space.kind.video")],
    ["media", $t("space.kind.media")],
    ["archive", $t("space.kind.archive")],
    ["program", $t("space.kind.program")],
    ["document", $t("space.kind.document")],
    ["rest", $t("space.kind.rest")],
  ] as const);

  let drives = $state<Drive[]>([]);
  let root = $state<string | null>(null);
  // Raw: the tree can be 300k nodes and is never changed in place.
  let scan = $state.raw<Scan | null>(null);
  let status = $state<Status | null>(null);
  let path = $state<string[]>([]);
  let history = $state.raw(emptyHistory());
  /** Opens a folder as a new visit (Back returns here). */
  function navigate(names: string[]) {
    history = visit(history, path, names);
    path = names;
  }
  function go(dir: -1 | 1) {
    const next = step(history, path, dir);
    if (!next) return;
    history = next.history;
    path = next.path;
  }
  /** Drives with a size: share used, and nearly full (under 10 % free). */
  const usage = (d: Drive) => {
    const used = d.total ? 1 - d.free / d.total : 0;
    return { used, full: d.total > 0 && d.free / d.total < 0.1 };
  };
  // The "other" block whose items are listed (names of its folder).
  let other = $state<{
    names: string[];
    parent: SpaceNode;
    block: Block;
  } | null>(null);
  $effect(() => {
    void path;
    void scan;
    other = null;
  });
  let error = $state("");
  let menu = $state<{ target: string; x: number; y: number } | null>(null);
  let timer: ReturnType<typeof setInterval> | null = null;
  let destroyed = false;

  const scanning = $derived(!!status?.root && status.root === root);
  const folder = $derived(
    scan ? (folderAt(scan.tree, path) ?? scan.tree) : null,
  );

  async function load(target: string) {
    // Raw JSON bytes of the cache file; empty when the drive has none.
    const bytes = await invoke<ArrayBuffer>("space_result", { root: target });
    scan = bytes.byteLength
      ? (JSON.parse(new TextDecoder().decode(bytes)) as Scan)
      : null;
    if (scan && !folderAt(scan.tree, path)) {
      path = [];
      history = emptyHistory();
    }
  }
  async function poll() {
    const next = await invoke<Status>("space_status");
    if (destroyed) return;
    const was = status?.root;
    status = next;
    if (!next.root) {
      stopPolling();
      if (
        was &&
        root &&
        was === root &&
        next.ended?.[0] === root &&
        !next.ended[1]
      )
        await load(root);
    }
  }
  function startPolling() {
    if (!timer) timer = setInterval(() => void poll(), 400);
  }
  function stopPolling() {
    if (timer) clearInterval(timer);
    timer = null;
  }
  async function startScan() {
    if (!root) return;
    error = "";
    try {
      await invoke("space_scan", { root });
      await poll();
      startPolling();
    } catch (e) {
      error = String(e);
    }
  }
  async function stop() {
    await invoke("space_stop");
    await poll();
  }
  /** Opening a drive shows its saved result, or scans it when there is none. */
  async function choose(target: string, scanIfNew: boolean) {
    root = target;
    path = [];
    history = emptyHistory();
    error = "";
    localStorage.setItem(LAST, target);
    await load(target);
    if (!scan && scanIfNew && !scanning) await startScan();
  }
  async function reveal(target: string) {
    menu = null;
    try {
      await invoke("space_reveal", { path: target });
    } catch (e) {
      error = revealMessage(e, $t);
    }
  }

  onMount(async () => {
    try {
      drives = await invoke<Drive[]>("space_drives");
      const saved = localStorage.getItem(LAST);
      const first = drives.find((d) => d.root === saved) ?? drives[0];
      // A scan still running (the page was left) keeps reporting.
      await poll();
      if (status?.root) startPolling();
      if (first) await choose(first.root, false);
    } catch (e) {
      error = String(e);
    }
  });
  onDestroy(() => {
    destroyed = true;
    stopPolling();
  });
  const when = (s: number) =>
    new Date(s * 1000).toLocaleString($locale === "zh" ? "zh-CN" : "en", {
      dateStyle: "medium",
      timeStyle: "short",
      hourCycle: "h23",
    });
</script>

<svelte:window
  onkeydown={(e) => {
    if (e.key === "Escape") menu = null; // i18n-ignore
    if (e.altKey && (e.key === "ArrowLeft" || e.key === "ArrowRight")) {
      e.preventDefault();
      go(e.key === "ArrowLeft" ? -1 : 1); // i18n-ignore
    }
  }}
  onmousedown={(e) => {
    // Mouse side buttons: keep WebView2 from navigating the page itself.
    if (e.button === 3 || e.button === 4) e.preventDefault();
  }}
  onmouseup={(e) => {
    if (e.button === 3 || e.button === 4) {
      e.preventDefault();
      go(e.button === 3 ? -1 : 1);
    }
  }}
  onclick={() => (menu = null)}
/>

<section class="space" aria-labelledby="space-title">
  <header class="bar">
    <h2 id="space-title">{$t("space.title")}</h2>
    <div class="drives" role="group" aria-label={$t("space.drives")}>
      {#each drives as d (d.root)}
        <button
          class="drive"
          class:active={d.root === root}
          aria-pressed={d.root === root}
          onclick={() => void choose(d.root, true)}
          ><span class="drive-line"
            >{d.root}{#if d.total}<small
                >{$t("space.driveFree", {
                  free: formatBytes(d.free),
                  total: formatBytes(d.total),
                })}</small
              >{/if}</span
          >{#if d.total}{@const u = usage(d)}<span
              class="usage"
              class:full={u.full}
              role="meter"
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={Math.round(u.used * 100)}
              aria-label={$t("space.used", { n: Math.round(u.used * 100) })}
              ><i style={`width:${(u.used * 100).toFixed(1)}%`}></i></span
            >{/if}</button
        >
      {:else}
        <span class="muted">{$t("space.noDrives")}</span>
      {/each}
    </div>
    <div class="actions">
      {#if scanning}
        <Button size="sm" variant="outline" onclick={() => void stop()}
          ><Square />{$t("space.stop")}</Button
        >
      {:else if root}
        <Button size="sm" onclick={() => void startScan()}
          >{#if scan}<RefreshCw />{$t("space.rescan")}{:else}<FolderSearch
            />{$t("space.scan")}{/if}</Button
        >
      {/if}
    </div>
  </header>

  <div class="status" aria-live="polite">
    {#if scanning && status}
      <span class="pulse"></span>{$t("space.scanning", {
        n: status.files.toLocaleString(),
        v: formatBytes(status.bytes),
        s: (status.elapsedMs / 1000).toFixed(0),
      })}{#if status.skipped}<span class="muted">
          · {$t("space.skipped", { n: status.skipped })}</span
        >{/if}
      <span class="current" title={status.current}>{status.current}</span>
    {:else if scan}
      {$t("space.scanned", {
        d: when(scan.scannedAt),
        n: scan.files.toLocaleString(),
        s: (scan.elapsedMs / 1000).toFixed(1),
      })}{#if scan.skipped}<span class="muted">
          · {$t("space.skipped", { n: scan.skipped })}</span
        >{/if}
      {#if scan.method}
        <span class="muted">
          · {scan.method === "mft-helper"
            ? $t("space.methodHelper")
            : scan.method === "mft"
              ? "MFT"
              : $t("space.methodWalk")}</span
        >
      {/if}
      {#if status?.ended?.[0] === root && status.ended[1]}<span class="muted">
          · {$t("space.stopped")}</span
        >{/if}
    {:else if root}
      <span class="muted"
        >{status?.ended?.[0] === root && status.ended[1]
          ? $t("space.stoppedNone")
          : $t("space.none")}</span
      >
    {/if}
    {#if error}<span class="error" role="alert">{error}</span>{/if}
  </div>

  {#if scan && folder}
    <nav class="crumbs" aria-label={$t("space.crumbs")}>
      <button
        class="nav"
        onclick={() => go(-1)}
        disabled={!history.back.length}
        aria-label={$t("space.back")}
        title={$t("space.back")}><ArrowLeft class="size-3.5" /></button
      ><button
        class="nav"
        onclick={() => go(1)}
        disabled={!history.forward.length}
        aria-label={$t("space.forward")}
        title={$t("space.forward")}><ArrowRight class="size-3.5" /></button
      >
      <button onclick={() => navigate([])} disabled={!path.length}
        >{scan.root}</button
      >
      {#each path as name, i}
        <ChevronRight class="size-3.5 shrink-0 text-muted-foreground" /><button
          onclick={() => navigate(path.slice(0, i + 1))}
          disabled={i === path.length - 1}>{name}</button
        >
      {/each}
      <span class="total"
        >{formatBytes(folder.s)} · {$t("space.files", {
          n: folder.f.toLocaleString(),
        })}</span
      >
    </nav>
    {#if folder.s > 0}
      <div class="map-wrap">
        <SpaceMap
          {folder}
          root={scan.root}
          {path}
          sep={scan.sep}
          onOpen={navigate}
          onMenu={(target, x, y) => (menu = { target, x, y })}
          onOther={(names, parent, block) => (other = { names, parent, block })}
        />
        {#if other}
          {@const o = other}
          {@const s = scan}
          <OtherList
            title={$t("space.otherTitle", {
              name: o.names.at(-1) ?? s.root,
              n: o.block.count.toLocaleString(),
            })}
            fullPath={joinPath(s.root, o.names, s.sep)}
            parent={o.parent}
            block={o.block}
            onDrill={(name) => navigate([...o.names, name])}
            onMenu={(name, x, y) =>
              (menu = {
                target: joinPath(s.root, [...o.names, name], s.sep),
                x,
                y,
              })}
            onClose={() => (other = null)}
          />
        {/if}
      </div>
    {:else}
      <p class="empty">
        {folder.x ? $t("space.deniedFolder") : $t("space.emptyFolder")}
      </p>
    {/if}
    <footer class="legend" aria-label={$t("space.legend")}>
      {#each LEGEND as [kind, text] (kind)}<span
          ><i style={`background:${tileColor(kind, 2)}`}></i>{text}</span
        >{/each}
      <span class="hint">{$t("space.hint")}</span>
    </footer>
  {/if}

  {#if menu}
    {@const target = menu.target}
    <div class="menu" role="menu" style={`left:${menu.x}px;top:${menu.y}px`}>
      <button role="menuitem" onclick={() => void reveal(target)}
        >{$t("space.reveal")}</button
      >
      <button
        role="menuitem"
        class="close"
        aria-label={$t("space.closeMenu")}
        onclick={() => (menu = null)}><X class="size-3.5" /></button
      >
    </div>
  {/if}
</section>

<style>
  .map-wrap {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
  }
  .space {
    display: flex;
    flex-direction: column;
    gap: 8px;
    height: 100%;
    min-height: 0;
    padding: 12px 16px 12px;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }
  h2 {
    font-size: 15px;
    font-weight: 600;
  }
  .drives {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    flex: 1;
  }
  .drive {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 3px 10px;
    border-radius: 6px;
    border: 1px solid var(--border);
    font-size: 12.5px;
    font-weight: 600;
    background: var(--card);
  }
  .drive-line {
    display: flex;
    align-items: baseline;
    gap: 6px;
  }
  .usage {
    display: block;
    height: 4px;
    border-radius: 2px;
    background: color-mix(in oklab, var(--foreground) 12%, transparent);
    overflow: hidden;
  }
  .usage i {
    display: block;
    height: 100%;
    border-radius: 2px;
    background: var(--primary);
  }
  .usage.full i {
    background: var(--destructive);
  }
  .nav {
    display: inline-flex;
    align-items: center;
    padding: 2px 4px;
    border-radius: 4px;
  }
  .nav:disabled {
    opacity: 0.35;
  }
  .drive small {
    font-weight: 400;
    color: var(--muted-foreground);
    font-size: 11px;
  }
  .drive.active {
    border-color: var(--primary);
    background: color-mix(in oklab, var(--primary) 12%, var(--card));
  }
  .status {
    display: flex;
    align-items: center;
    gap: 4px;
    min-height: 18px;
    font-size: 12px;
    white-space: nowrap;
    overflow: hidden;
  }
  .current {
    margin-left: 8px;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--muted-foreground);
    direction: rtl;
    text-align: left;
  }
  .pulse {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--primary);
    animation: pulse 1s ease-in-out infinite alternate;
    flex-shrink: 0;
  }
  @keyframes pulse {
    to {
      opacity: 0.3;
    }
  }
  .muted,
  .hint,
  .total {
    color: var(--muted-foreground);
  }
  .error {
    margin-left: 8px;
    color: var(--destructive);
  }
  .crumbs {
    display: flex;
    align-items: center;
    gap: 2px;
    font-size: 12.5px;
    min-width: 0;
    overflow: hidden;
  }
  .crumbs button {
    padding: 2px 6px;
    border-radius: 4px;
    white-space: nowrap;
    max-width: 260px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .crumbs button:not(:disabled):hover {
    background: var(--accent);
  }
  .crumbs button:disabled {
    font-weight: 600;
    opacity: 1;
  }
  .total {
    margin-left: auto;
    padding-left: 12px;
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }
  .empty {
    flex: 1;
    display: grid;
    place-items: center;
    color: var(--muted-foreground);
    font-size: 13px;
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 12px;
    font-size: 11px;
  }
  .legend span {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .legend i {
    width: 10px;
    height: 10px;
    border-radius: 2px;
    border: 1px solid color-mix(in oklab, var(--foreground) 18%, transparent);
  }
  .hint {
    margin-left: auto;
  }
  .menu {
    position: fixed;
    z-index: 50;
    display: flex;
    align-items: center;
    padding: 4px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--popover);
    color: var(--popover-foreground);
    box-shadow: 0 6px 20px rgb(0 0 0 / 0.2);
    font-size: 12.5px;
  }
  .menu button {
    padding: 4px 10px;
    border-radius: 4px;
  }
  .menu button:hover {
    background: var(--accent);
  }
  .menu .close {
    padding: 4px;
  }
</style>
