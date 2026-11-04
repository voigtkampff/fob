<script>
  import "../app.css";
  import { onMount } from "svelte";
  import { allowsNativeMenu } from "$lib/utils/context-menu.js";

  // Fob: no WebView menu (Reload, Inspect...) anywhere; text fields keep
  // their copy/paste menu. Components with their own menu still get the event.
  onMount(() => {
    const block = (/** @type {MouseEvent} */ e) => {
      if (!allowsNativeMenu(e.target)) e.preventDefault();
    };
    document.addEventListener("contextmenu", block);
    return () => document.removeEventListener("contextmenu", block);
  });
</script>

<slot />
