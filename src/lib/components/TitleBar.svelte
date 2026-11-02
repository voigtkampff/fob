<script lang="ts">
  import { platform } from "@tauri-apps/plugin-os";
  import { invoke } from "@tauri-apps/api/core";
  import { settingsStore } from "$lib/stores/settings";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as DropdownMenu from "$lib/components/ui/dropdown-menu/index.js";
  import ThemeSwitcher from "./ThemeSwitcher.svelte";
  import { t } from "$lib/i18n";
  import {
    Pause,
    Play,
    PanelsTopLeft,
    Minimize2,
    Ellipsis,
    Settings2,
    Info,
  } from "@lucide/svelte";
  export let isFrozen: boolean;
  export let onPause: (value: boolean) => void;
  let actionError = "";
  let settingsShortcut = "Ctrl+,";
  try {
    if (platform() === "macos") settingsShortcut = "⌘,";
  } catch {
    /* Browser preview. */
  }
  async function command(name: string, args?: Record<string, unknown>) {
    try {
      await invoke(name, args);
      actionError = "";
    } catch (error) {
      actionError = String(error);
    }
  }
</script>

<div
  class="title-bar flex h-12 shrink-0 items-center gap-3 border-b border-border bg-card px-4"
  data-tauri-drag-region
>
  <div class="h-full min-w-4 flex-1" data-tauri-drag-region></div>
  <div class="flex items-center gap-1">
    <span
      class="mr-2 flex items-center gap-1.5 text-xs text-muted-foreground"
      role="status"
      ><span
        class={`size-1.5 rounded-full ${isFrozen ? "bg-muted-foreground" : "bg-[var(--status-running)]"}`}
      ></span>{isFrozen
        ? $t("mini.paused")
        : $t("title.liveEvery", {
            s:
              $settingsStore.behavior.refreshRate === 1000
                ? 1.5
                : $settingsStore.behavior.refreshRate / 1000,
          })}</span
    >
    <Button
      variant={isFrozen ? "secondary" : "ghost"}
      size="icon-sm"
      aria-label={isFrozen ? $t("mini.resume") : $t("mini.pause")}
      title={isFrozen ? $t("mini.resume") : $t("mini.pause")}
      aria-pressed={isFrozen}
      onclick={() => onPause(!isFrozen)}
      >{#if isFrozen}<Play />{:else}<Pause />{/if}</Button
    >
    <Button
      variant="ghost"
      size="icon-sm"
      aria-label={$t("title.openMini")}
      title={$t("title.mini")}
      onclick={() => command("show_companion")}><PanelsTopLeft /></Button
    >
    <Button
      variant="ghost"
      size="icon-sm"
      aria-label={$t("title.collapse")}
      title={$t("title.collapseTitle")}
      onclick={() => command("collapse_to_tray")}><Minimize2 /></Button
    >
    <div class="mx-1 h-4 w-px bg-border"></div>
    <ThemeSwitcher />
    <DropdownMenu.Root
      ><DropdownMenu.Trigger
        >{#snippet child({ props })}<Button
            {...props}
            variant="ghost"
            size="icon-sm"
            aria-label={$t("title.menu")}><Ellipsis /></Button
          >{/snippet}</DropdownMenu.Trigger
      >
      <DropdownMenu.Content align="end" class="w-60">
        <DropdownMenu.Item onclick={() => command("open_settings")}
          ><Settings2 />{$t("title.settings")}<DropdownMenu.Shortcut
            >{settingsShortcut}</DropdownMenu.Shortcut
          ></DropdownMenu.Item
        >
        <DropdownMenu.Separator />
        <DropdownMenu.Item
          onclick={() => command("open_about", { check: false })}
          ><Info />{$t("title.about")}</DropdownMenu.Item
        >
        <DropdownMenu.Separator /><DropdownMenu.Item
          onclick={() => command("collapse_to_tray")}
          ><Minimize2 />{$t("title.collapse")}</DropdownMenu.Item
        ><DropdownMenu.Item onclick={() => command("quit_app")}
          >{$t("mini.quit")}</DropdownMenu.Item
        >
      </DropdownMenu.Content>
    </DropdownMenu.Root>
  </div>
</div>
{#if actionError}<div
    role="alert"
    class="flex shrink-0 items-center justify-between gap-3 border-b border-border bg-card px-4 py-2 text-xs text-destructive"
  >
    <span>{actionError}</span><Button
      variant="ghost"
      size="sm"
      onclick={() => (actionError = "")}>{$t("title.dismiss")}</Button
    >
  </div>{/if}
