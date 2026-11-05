<script lang="ts">
  // Fob: Settings → mini window hotkey. Click the field, press a combo; Fob
  // registers it (RegisterHotKey on Windows) and only then saves it.
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Keyboard, X } from "@lucide/svelte";
  import { fromKeydown, validate } from "$lib/utils/hotkey.js";
  import { t } from "$lib/i18n";
  /** Which hotkey it records (hotkey.rs Target): the mini window or the
   * status bar. */
  let {
    target = "mini",
    id = "mini-hotkey",
    describedby = "mini-hotkey-note",
    label = "",
  }: {
    target?: "mini" | "overlay" | "main";
    id?: string;
    describedby?: string;
    /** Accessible name; the mini window's by default. */
    label?: string;
  } = $props();
  let combo = $state("");
  let recording = $state(false);
  let held = $state("");
  // What went wrong, kept as a code so the text follows the UI language.
  let fault = $state<{ code: string; value?: string; launch?: boolean } | null>(
    null,
  );
  let busy = $state(false);
  let field = $state<HTMLButtonElement | null>(null);
  // Literal keys so the i18n check can see every one.
  const message = (code: string, value = "") =>
    ({
      taken: $t("hotkey.taken", { v: value }),
      needsModifier: $t("hotkey.needsModifier"),
      invalid: $t("hotkey.invalid"),
      noKey: $t("hotkey.noKey"),
    })[code] ?? code;
  const error = $derived(
    fault
      ? `${fault.launch ? `${$t("hotkey.atLaunch")} ` : ""}${message(fault.code, fault.value)}`
      : "",
  );
  onMount(() => {
    void invoke<{ combo: string; error: string | null; failed: string }>(
      "get_hotkey",
      { target },
    ).then((status) => {
      combo = status.combo;
      if (status.error)
        fault = { code: status.error, value: status.failed, launch: true };
    });
  });
  // `resume`: give the current combo back now (not when a new one is about
  // to be set; that resumes after set_hotkey has answered).
  function record(on: boolean, resume = true) {
    if (recording === on) return;
    recording = on;
    held = "";
    if (on) fault = null;
    if (on || resume) void invoke("suspend_hotkey", { target, suspend: on });
  }
  async function apply(next: string) {
    busy = true;
    try {
      combo = await invoke<string>("set_hotkey", { target, combo: next });
      fault = null;
    } catch (e) {
      // The previous combo stays registered and saved.
      fault = { code: String(e), value: next };
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
    if (refused) fault = { code: refused };
    void (refused ? Promise.resolve() : apply(next.combo)).finally(
      () => void invoke("suspend_hotkey", { target, suspend: false }),
    );
  }
  // A combo another app owns never reaches this page (Windows hands the key
  // to that app), so all we see is the modifiers going up again.
  function keyup(event: KeyboardEvent) {
    if (!recording || !held) return;
    if (event.ctrlKey || event.altKey || event.shiftKey || event.metaKey)
      return;
    record(false);
    fault = { code: "noKey" };
  }
</script>

<!-- One fixed width for the whole field: box (w-44), gap, clear slot. The
  error wraps inside it, under the box, so a long message can't widen the
  column and push the box left of the other rows -->
<div class="w-[13.5rem]">
  <div class="flex items-center gap-2">
    <button
      bind:this={field}
      {id}
      type="button"
      class={`flex h-8 w-44 items-center gap-2 rounded-md border px-3 text-sm tabular-nums transition-colors ${recording ? "border-ring ring-2 ring-ring/40" : "border-input"} ${combo || recording ? "" : "text-muted-foreground"}`}
      aria-label={label || $t("hotkey.fieldAria")}
      aria-describedby={describedby}
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
      >{:else}<!-- Same slot as the clear button, so every field has one width
      and the boxes line up whether or not a combo is set -->
      <span class="size-8 shrink-0" aria-hidden="true"></span>{/if}
  </div>
  {#if error}<p
      role="alert"
      class="mt-2 text-xs leading-relaxed text-destructive"
    >
      {error}
    </p>{/if}
</div>
