<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { emit, listen } from "@tauri-apps/api/event";
  import { settingsStore, settingsStatus } from "$lib/stores/settings";
  import { REFRESH_RATE_OPTIONS } from "$lib/constants";
  import ThemeGallery from "./ThemeGallery.svelte";
  import HotkeyField from "./HotkeyField.svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Checkbox } from "$lib/components/ui/checkbox/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import * as NativeSelect from "$lib/components/ui/native-select/index.js";
  import * as ToggleGroup from "$lib/components/ui/toggle-group/index.js";
  import { Settings2, Palette } from "@lucide/svelte";
  import { t } from "$lib/i18n";
  let pane = "general";
  let runtimeError = "";
  $: rate = $settingsStore.behavior.refreshRate;
  $: options = REFRESH_RATE_OPTIONS.some((o) => o.value === rate)
    ? REFRESH_RATE_OPTIONS
    : [{ value: rate, label: `${rate / 1000}s` }, ...REFRESH_RATE_OPTIONS];
  function select(value: string) {
    if (!["general", "appearance"].includes(value)) return;
    pane = value;
    try {
      localStorage.setItem("settings-pane", value);
    } catch {
      /* Session selection remains available. */
    }
  }
  function save(
    key:
      | "refreshRate"
      | "menuBarEnabled"
      | "keepRunning"
      | "startInTray"
      | "language",
    value: number | boolean | string,
  ) {
    settingsStore.updateConfig({
      behavior: { ...$settingsStore.behavior, [key]: value },
    });
  }
  onMount(() => {
    let disposed = false;
    const stops: (() => void)[] = [];
    void (async () => {
      const stopPane = await listen<string>("settings-pane", ({ payload }) =>
        select(payload),
      );
      const stopError = await listen<string>(
        "preferences-runtime-error",
        ({ payload }) => {
          runtimeError = payload;
        },
      );
      if (disposed) {
        stopPane();
        stopError();
        return;
      }
      stops.push(stopPane, stopError);
      select(
        (await invoke<string>("settings_pane")) ||
          localStorage.getItem("settings-pane") ||
          "general",
      );
      await settingsStore.init();
      await emit("preferences-runtime-status-request");
    })();
    return () => {
      disposed = true;
      stops.forEach((stop) => stop());
    };
  });
</script>

<div class="flex h-screen flex-col bg-background">
  <header
    class="flex shrink-0 justify-center border-b border-border bg-card px-6 py-4"
  >
    <ToggleGroup.Root
      type="single"
      value={pane}
      onValueChange={select}
      variant="outline"
      aria-label={$t("settings.section")}
    >
      <ToggleGroup.Item value="general" class="px-5"
        ><Settings2 />{$t("settings.general")}</ToggleGroup.Item
      >
      <ToggleGroup.Item value="appearance" class="px-5"
        ><Palette />{$t("settings.appearance")}</ToggleGroup.Item
      >
    </ToggleGroup.Root>
  </header>
  <div class="min-h-0 flex-1 overflow-y-auto p-6">
    {#if pane === "general"}
      <h1 class="text-lg font-semibold tracking-tight">
        {$t("settings.general")}
      </h1>
      <p class="mt-1 mb-5 text-sm text-muted-foreground">
        {$t("settings.generalLead")}
      </p>
      <fieldset disabled={!$settingsStatus.ready} class="space-y-5">
        <Card.Root size="sm"
          ><Card.Header
            ><Card.Title>{$t("settings.monitoring")}</Card.Title></Card.Header
          ><Card.Content>
            <div class="flex items-center justify-between gap-6">
              <div>
                <Label for="refresh-interval">{$t("settings.refresh")}</Label>
                <p class="mt-1 text-xs leading-relaxed text-muted-foreground">
                  {$t("settings.refreshNote")}
                </p>
              </div>
              <NativeSelect.Root
                id="refresh-interval"
                class="w-28"
                value={rate}
                onchange={(event) =>
                  save("refreshRate", Number(event.currentTarget.value))}
              >
                {#each options as option}<option value={option.value}
                    >{option.label}</option
                  >{/each}
              </NativeSelect.Root>
            </div>
            <p class="mt-1 text-xs text-muted-foreground">
              {$t("settings.refreshMore")}
            </p>
          </Card.Content></Card.Root
        >
        <Card.Root size="sm"
          ><Card.Header
            ><Card.Title>{$t("settings.language")}</Card.Title></Card.Header
          ><Card.Content>
            <div class="flex items-center justify-between gap-6">
              <div>
                <Label for="language">{$t("settings.languageLabel")}</Label>
                <p class="mt-1 text-xs leading-relaxed text-muted-foreground">
                  {$t("settings.languageNote")}
                </p>
              </div>
              <NativeSelect.Root
                id="language"
                class="w-44"
                value={$settingsStore.behavior.language ?? "auto"}
                onchange={(event) =>
                  save("language", event.currentTarget.value)}
              >
                {#each ["auto", "en", "zh"] as option}<option value={option}
                    >{$t(`lang.${option}`)}</option
                  >{/each}
              </NativeSelect.Root>
            </div>
          </Card.Content></Card.Root
        >
        <Card.Root size="sm"
          ><Card.Header
            ><Card.Title>{$t("settings.tray")}</Card.Title></Card.Header
          ><Card.Content class="gap-5">
            <div class="flex items-start gap-3">
              <Checkbox
                id="show-menu-icon"
                class="mt-0.5"
                checked={$settingsStore.behavior.menuBarEnabled}
                onCheckedChange={(value) => save("menuBarEnabled", value)}
              />
              <div>
                <Label for="show-menu-icon">{$t("settings.showTray")}</Label>
                <p class="mt-1 text-xs leading-relaxed text-muted-foreground">
                  {$t("settings.showTrayNote")}
                </p>
              </div>
            </div>
            <div class="flex items-start gap-3">
              <Checkbox
                id="start-in-tray"
                class="mt-0.5"
                disabled={!$settingsStore.behavior.menuBarEnabled}
                checked={$settingsStore.behavior.startInTray}
                onCheckedChange={(value) => save("startInTray", value)}
              />
              <div>
                <Label for="start-in-tray">{$t("settings.startInTray")}</Label>
                <p class="mt-1 text-xs leading-relaxed text-muted-foreground">
                  {$t("settings.startInTrayNote")}
                </p>
              </div>
            </div>
            <p class="text-xs leading-relaxed text-muted-foreground">
              {$t("settings.closeNote")}
            </p>
          </Card.Content></Card.Root
        >
        <Card.Root size="sm"
          ><Card.Header
            ><Card.Title>{$t("hotkey.title")}</Card.Title></Card.Header
          ><Card.Content>
            <div class="flex items-start justify-between gap-6">
              <div>
                <Label for="mini-hotkey">{$t("hotkey.label")}</Label>
                <p
                  id="mini-hotkey-note"
                  class="mt-1 text-xs leading-relaxed text-muted-foreground"
                >
                  {$t("hotkey.note")}
                </p>
              </div>
              <div class="shrink-0"><HotkeyField /></div>
            </div>
          </Card.Content></Card.Root
        >
      </fieldset>
    {:else}
      <h1 class="text-lg font-semibold tracking-tight">
        {$t("settings.appearance")}
      </h1>
      <p class="mt-1 mb-5 text-sm text-muted-foreground">
        {$t("settings.appearanceLead")}
      </p>
      <ThemeGallery embedded />
    {/if}
  </div>
  <footer class="shrink-0 border-t border-border bg-card px-6 py-3">
    {#if $settingsStatus.error || runtimeError}
      <div
        role="alert"
        class="flex items-center justify-between gap-3 text-xs text-destructive"
      >
        <span>{$settingsStatus.error || runtimeError}</span><Button
          variant="outline"
          size="sm"
          onclick={() => {
            void settingsStore.retry();
            void emit("preferences-runtime-retry");
          }}>{$t("settings.retry")}</Button
        >
      </div>
    {:else}<p role="status" class="text-xs text-muted-foreground">
        {!$settingsStatus.ready
          ? $t("settings.loading")
          : $settingsStatus.saving
            ? $t("settings.saving")
            : $t("settings.saved")}
      </p>{/if}
  </footer>
</div>
