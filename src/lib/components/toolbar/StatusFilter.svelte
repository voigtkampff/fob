<script lang="ts">
  import { STATUS_OPTIONS } from "$lib/constants";
  import { settingsStore } from "$lib/stores";
  import { Label } from "$lib/components/ui/label/index.js";
  import * as NativeSelect from "$lib/components/ui/native-select/index.js";
  import { t } from "$lib/i18n";
  import { statusLabel } from "$lib/utils/process-status";
  export let statusFilter = "all";
  function save() {
    settingsStore.updateConfig({
      behavior: {
        ...$settingsStore.behavior,
        defaultStatusFilter: statusFilter,
      },
    });
  }
</script>

<Label class="gap-2 text-xs"
  >{$t("col.status")}<NativeSelect.Root
    bind:value={statusFilter}
    onchange={save}
    aria-label={$t("filter.statusAria")}
    >{#each STATUS_OPTIONS as option}<option value={option.value}
        >{option.value === "all"
          ? $t("filter.allStatuses")
          : statusLabel(option.label, $t)}</option
      >{/each}</NativeSelect.Root
  ></Label
>
