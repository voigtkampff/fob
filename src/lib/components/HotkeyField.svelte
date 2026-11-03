<script lang="ts">
  // Fob: Settings → mini window hotkey. Click the field, press a combo; Fob
  // registers it (RegisterHotKey on Windows) and only then saves it.
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Keyboard, X } from "@lucide/svelte";
  import { fromKeydown, validate } from "$lib/utils/hotkey.js";
  import { t } from "$lib/i18n";
  let combo = $state("");
  let recording = $state(false);
  let held = $state("");
  let error = $state("");
  let busy = $state(false);
  let field = $state<HTMLButtonElement | null>(null);
  // Literal keys so the i18n check can see every one.
  const message = (code: string, value = "") =>
    ({
      taken: $t("hotkey.taken", { v: value }),
      needsModifier: $t("hotkey.needsModifier"),
      invalid: $t("hotkey.invalid"),
    })[code] ?? code;
  onMount(() => {
    void invoke<{ combo: string; error: string | null; failed: string }>(
      "get_mini_hotkey",
    ).then((status) => {
      combo = status.combo;
      if (status.error)
        error = `${$t("hotkey.atLaunch")} ${message(status.error, status.failed)}`;
    });
  });
  // `resume`: give the current combo back now (not when a new one is about
  // to be set; that resumes after set_mini_hotkey has answered).
  function record(on: boolean, resume = true) {
    if (recording === on) return;
    recording = on;
    held = "";
    if (on) error = "";
    if (on || resume) void invoke("suspend_mini_hotkey", { suspend: on });
  }
  async function apply(next: string) {
    busy = true;
    try {
      combo = await invoke<string>("set_mini_hotkey", { combo: next });
      error = "";
    } catch (e) {
      // The previous combo stays registered and saved.
      error = message(String(e), next);
    } finally {
      busy = false;
    }
  }
  function keydown(event: KeyboardEvent) {
    if (!recording) return;
    event.preventDefault();
    event.stopPropagation();
    if (event.code === "Escape" && !event.ctrlKey && !event.altKey) {
      record(false);
      return;
    }
    const next = fromKeydown(event);
    held = next.held;
    if (!next.combo) return;
    record(false, false);
    const refused = validate(next.combo);
    if (refused) error = message(refused);
    void (refused ? Promise.resolve() : apply(next.combo)).finally(
      () => void invoke("suspend_mini_hotkey", { suspend: false }),
    );
  }
  // A combo another app owns never reaches this page (Windows hands the key
  // to that app), so all we see is the modifiers going up again.
  function keyup(event: KeyboardEvent) {
    if (!recording || !held) return;
    if (event.ctrlKey || event.altKey || event.shiftKey || event.metaKey)
      return;
    record(false);
    error = $t("hotkey.noKey");
  }
</script>

<div class="flex items-center gap-2">
  <button
    bind:this={field}
    id="mini-hotkey"
    type="button"
    class={`flex h-8 min-w-44 items-center gap-2 rounded-md border px-3 text-sm tabular-nums transition-colors ${recording ? "border-ring ring-2 ring-ring/40" : "border-input"} ${combo || recording ? "" : "text-muted-foreground"}`}
    aria-label={$t("hotkey.fieldAria")}
    aria-describedby="mini-hotkey-note"
    disabled={busy}
    onclick={() => record(!recording)}
    onkeydown={keydown}
    onkeyup={keyup}
    onblur={() => record(false)}
    ><Keyboard class="size-3.5 shrink-0 text-muted-foreground" /><span
      class="truncate"
      >{recording
        ? held || $t("hotkey.press")
        : combo || $t("hotkey.none")}</span
    ></button
  >
  {#if combo}<Button
      variant="ghost"
      size="icon-sm"
      aria-label={$t("hotkey.clear")}
      title={$t("hotkey.clear")}
      disabled={busy}
      onclick={() => void apply("")}><X /></Button
    >{/if}
</div>
{#if error}<p role="alert" class="mt-2 text-xs text-destructive">
    {error}
  </p>{/if}
