<script lang="ts">
  // winperf: no release check (NeoHtop's checked NeoHtop's GitHub releases).
  import { getVersion } from "@tauri-apps/api/app";
  import { onMount } from "svelte";
  import { themeStore } from "$lib/stores/theme";
  import { ASCII_ART, APP_INFO } from "$lib/constants";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import { ExternalLink } from "@lucide/svelte";
  import { t } from "$lib/i18n";
  let version = "";
  onMount(() => {
    let active = true;
    themeStore.init();
    void getVersion()
      .then((next) => {
        if (active) version = next;
      })
      .catch(() => {
        /* Desktop version metadata is not available in a browser. */
      });
    return () => {
      active = false;
    };
  });
</script>

<div class="h-screen space-y-5 overflow-y-auto bg-background p-6">
  <div class="flex items-center gap-3">
    <img src="/32x32.png" alt="" class="size-10" />
    <div>
      <h2 class="font-semibold">
        {$t("app.name")}
        {version ? `v${version}` : ""}
      </h2>
      <p class="mt-1 text-xs text-muted-foreground">
        {$t("about.basedOn", { v: APP_INFO.developer })}
      </p>
    </div>
    <Badge variant="secondary" class="ml-auto font-normal"
      >{$t("about.openSource")}</Badge
    >
  </div>
  <pre
    class="overflow-x-auto rounded-md border border-border bg-background p-3 font-mono text-[8px] leading-tight text-primary">{ASCII_ART}</pre>
  <div class="space-y-2 text-xs">
    <div class="flex justify-between">
      <span class="text-muted-foreground">{$t("about.stack")}</span><span
        >{APP_INFO.stack.join(" · ")}</span
      >
    </div>
    <Button
      variant="link"
      size="sm"
      class="h-auto p-0 text-xs"
      href={APP_INFO.github}
      target="_blank"
      rel="noopener noreferrer"
      >{$t("about.source")}<ExternalLink class="size-3" /></Button
    >
  </div>
  <p class="border-t border-border pt-3 text-xs text-muted-foreground">
    {$t("about.updates")}
  </p>
</div>
