<script lang="ts">
  import { onMount } from "svelte";
  import { check, type Update } from "@tauri-apps/plugin-updater";
  import { relaunch } from "@tauri-apps/plugin-process";
  import { t } from "../lib/i18n.svelte";
  import { notify } from "../lib/ui.svelte";

  let update = $state<Update | null>(null);
  let installing = $state(false);

  onMount(async () => {
    // Builds from source have no update channel; then check() fails and nothing is shown.
    update = await check().catch(() => null);
  });

  async function install() {
    if (!update) return;
    installing = true;
    try {
      await update.downloadAndInstall();
      await relaunch();
    } catch (e) {
      notify(String(e), "error");
      installing = false;
    }
  }
</script>

{#if update}
  <div class="update">
    <span class="secondary">{t("update.available", update.version)}</span>
    <button class="btn accent" disabled={installing} onclick={install}>
      {installing ? t("update.installing") : t("update.install")}
    </button>
  </div>
{/if}

<style>
  .update {
    display: grid;
    gap: 8px;
    padding: 12px;
    border-radius: 12px;
    border: 1px solid var(--line);
    background: var(--card);
  }
  .update .btn {
    justify-content: center;
  }
</style>
