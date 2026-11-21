<script lang="ts">
  import type { Theme } from "$lib/types";
  let { theme }: { theme: Theme } = $props();
</script>

<span class="flex shrink-0 gap-0.5" aria-hidden="true"
  >{#each [theme.colors.blue, theme.colors.red, theme.colors.green] as color}<span
      class="size-2 rounded-full"
      style:background={color}
    ></span>{/each}</span
>
