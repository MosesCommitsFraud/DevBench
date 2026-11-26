<script lang="ts">
  import { RefreshCw, Download, LoaderCircle } from "@lucide/svelte";
  import { lang, t } from "../lib/i18n.svelte";
  import { checkUpdates, installUpdate, updates } from "../lib/updater.svelte";

  // Version and updates from the GitHub releases, as a card in Settings.
  const busy = $derived(updates.status === "checking" || updates.status === "downloading" || updates.status === "ready");

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
    <span class="mono version">{updates.version ?? "–"}</span>
  </div>

  {#if updates.available && updates.status !== "current"}
    <div class="found">
      <strong>{t("update.available", updates.available.version)}</strong>
      {#if updates.available.notes}<p class="notes">{updates.available.notes}</p>{/if}
      {#if updates.status === "downloading" || updates.status === "ready"}
        <div class="progress">
          <div class="row">
            <span>{updates.status === "ready" ? t("update.restarting") : t("update.downloading")}</span>
            {#if updates.progress !== null}<span class="mono">{Math.round(updates.progress * 100)} %</span>{/if}
          </div>
          <div class="bar"><i class:indeterminate={updates.progress === null} style:transform={updates.progress === null ? undefined : `scaleX(${updates.progress})`}></i></div>
        </div>
      {:else}
        <button class="btn primary" onclick={installUpdate}><Download size={16} strokeWidth={1.5} />{t("update.install")}</button>
      {/if}
    </div>
  {:else if updates.status === "checking"}
    <p class="muted">{t("update.checking")}</p>
  {:else if updates.status === "current"}
    <p class="muted">{t("update.current", since(updates.checkedAt))}</p>
  {/if}

  {#if updates.error}<p class="error mono">{updates.error}</p>{/if}

  <div>
    <button class="btn" disabled={busy} onclick={() => checkUpdates()}>
      {#if updates.status === "checking"}<LoaderCircle size={16} strokeWidth={1.5} class="spin" />{:else}<RefreshCw size={16} strokeWidth={1.5} />{/if}
      {t("update.check")}
    </button>
  </div>
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
    align-items: baseline;
  }
  h2 {
    margin: 0;
    font-size: 16px;
    line-height: 24px;
    font-weight: 600;
  }
  .version {
    font-size: 13px;
  }
  p {
    margin: 0;
  }
  .found {
    display: grid;
    gap: 8px;
    justify-items: start;
    padding: 12px 14px;
    border-radius: var(--r-card);
    background: var(--accent-soft);
  }
  .notes {
    color: var(--text-2);
    font-size: 13px;
    white-space: pre-line;
    max-height: 120px;
    overflow-y: auto;
  }
  .progress {
    width: 100%;
    display: grid;
    gap: 6px;
    font-size: 12px;
    color: var(--text-2);
  }
  .row {
    display: flex;
    justify-content: space-between;
  }
  .bar {
    height: 6px;
    border-radius: 3px;
    background: var(--raised);
    overflow: hidden;
  }
  .bar i {
    display: block;
    height: 100%;
    border-radius: 3px;
    background: var(--accent);
    transform-origin: left;
    transition: transform var(--menu) var(--ease);
  }
  .bar i.indeterminate {
    width: 33%;
    animation: slide 1.2s var(--ease) infinite;
  }
  .error {
    color: var(--danger);
    overflow-wrap: anywhere;
  }
  :global(.spin) {
    animation: spin 0.9s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  @keyframes slide {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(300%);
    }
  }
</style>
