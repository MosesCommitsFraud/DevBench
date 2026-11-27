<script lang="ts">
  import { RefreshCw, LoaderCircle, ArrowUpCircle } from "@lucide/svelte";
  import { lang, t } from "../lib/i18n.svelte";
  import { checkUpdates, updates } from "../lib/updater.svelte";
  import ErrorNote from "./ErrorNote.svelte";

  // The installed version and whether a newer one is out, as a card in Settings. What a new
  // version changes and installing it live in the update dialog.
  const found = $derived(!!updates.available && updates.status !== "current");

  function since(time: number | null) {
    if (!time) return "";
    const minutes = Math.round((Date.now() - time) / 60000);
    if (minutes < 1) return t("update.justNow");
    return new Intl.RelativeTimeFormat(lang(), { numeric: "auto" }).format(-minutes, "minute");
  }
</script>

<section class="card update">
  <div class="head">
    <h2>{t("update.title")}</h2>
    <button class="btn ghost" disabled={updates.status === "checking" || updates.status === "downloading"} onclick={() => checkUpdates()}>
      {#if updates.status === "checking"}<LoaderCircle size={15} strokeWidth={1.5} class="spin" />{:else}<RefreshCw size={15} strokeWidth={1.5} />{/if}
      {t("update.check")}
    </button>
  </div>

  <div class="rows">
    <div class="row">
      <span class="dot" class:on={updates.status === "current"} class:busy={found}></span>
      <div class="what">
        <span class="name">DevBench <span class="mono">{updates.version ?? "–"}</span></span>
        <span class="detail">
          {#if found}
            {t("update.available", updates.available!.version)}
          {:else if updates.status === "checking"}
            {t("update.checking")} …
          {:else if updates.status === "current"}
            {t("update.current", since(updates.checkedAt))}
          {/if}
        </span>
      </div>
      {#if found}
        <button class="btn primary" onclick={() => (updates.dialog = true)}><ArrowUpCircle size={14} strokeWidth={1.75} />{t("update.showChanges")}</button>
      {/if}
    </div>
  </div>

  {#if updates.error && !updates.dialog}<div class="err"><ErrorNote message={updates.error} /></div>{/if}
</section>

<style>
  .update {
    padding: 20px 24px;
    display: grid;
    gap: 12px;
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  h2 {
    margin: 0;
    font-size: 16px;
    line-height: 24px;
    font-weight: 500;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 52px;
    padding: 8px 8px 8px 14px;
  }
  .row .dot {
    width: 7px;
    height: 7px;
    flex: none;
  }
  .what {
    flex: 1;
    min-width: 0;
    display: grid;
    gap: 2px;
  }
  .name {
    font-size: 13.5px;
    font-weight: 500;
  }
  .detail {
    font-size: 12px;
    color: var(--text-3);
  }
  .err {
    padding: 10px 12px;
    border-radius: var(--r-control);
    background: var(--sunken);
  }
  :global(.spin) {
    animation: spin 0.9s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
