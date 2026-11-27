<script lang="ts">
  import { X, Download } from "@lucide/svelte";
  import { bytes } from "../lib/format";
  import { lang, t } from "../lib/i18n.svelte";
  import { changes, installUpdate, updates } from "../lib/updater.svelte";
  import ErrorNote from "./ErrorNote.svelte";

  // A new version: what it changes, and installing it with its progress. Opened from the line
  // in the sidebar and from the Version card in Settings.
  const a = $derived(updates.available);
  const points = $derived(changes(a?.notes ?? null));
  const working = $derived(updates.status === "downloading" || updates.status === "ready");
  const released = $derived.by(() => {
    // e.g. "2026-11-27 14:25:49.123 +00:00:00": the day is enough
    const day = a?.date?.match(/^\d{4}-\d\d-\d\d/)?.[0];
    if (!day) return "";
    const d = new Date(`${day}T12:00:00`);
    return isNaN(d.getTime()) ? "" : d.toLocaleDateString(lang() === "de" ? "de-DE" : "en-GB", { day: "numeric", month: "long" });
  });

  function close() {
    if (!working) updates.dialog = false;
  }
  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") close();
  }
</script>

<svelte:window onkeydown={onKey} />

{#if a}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="scrim" onclick={(e) => e.target === e.currentTarget && close()}>
    <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="update-title">
      <header>
        <div>
          <h3 id="update-title">DevBench {a.version}</h3>
          <p class="sub">
            {t("update.installed", updates.version ?? "–")}{#if released}<span class="sep">·</span>{t("update.released", released)}{/if}
          </p>
        </div>
        {#if !working}
          <button class="btn ghost icon" aria-label={t("ports.close")} title={t("ports.close")} onclick={close}><X size={16} strokeWidth={1.5} /></button>
        {/if}
      </header>

      {#if points.length}
        <section>
          <span class="eyebrow">{t("update.changes")}</span>
          <ul class="rows">
            {#each points as p, i (i)}<li>{p}</li>{/each}
          </ul>
        </section>
      {/if}

      {#if updates.error}<div class="err"><ErrorNote message={updates.error} /></div>{/if}

      {#if working}
        <div class="progress">
          <div class="line">
            <span>{updates.status === "ready" ? t("update.restarting") : t("update.downloading")} …</span>
            {#if updates.total}
              <span class="mono">{t("update.of", bytes(updates.received), bytes(updates.total))}</span>
            {:else if updates.received}
              <span class="mono">{bytes(updates.received)}</span>
            {/if}
          </div>
          <div class="bar"><i class:unknown={updates.progress === null} style:transform={updates.progress === null ? undefined : `scaleX(${updates.progress})`}></i></div>
        </div>
      {:else}
        <p class="hint">{t("update.restartHint")}</p>
        <div class="buttons">
          <button class="btn ghost" onclick={close}>{t("update.later")}</button>
          <button class="btn primary" onclick={installUpdate}><Download size={15} strokeWidth={1.75} />{t("update.installRestart")}</button>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    padding: 24px;
    background: rgb(0 0 0 / 0.5);
  }
  .dialog {
    width: min(560px, 100%);
    max-height: calc(100vh - 48px);
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) auto auto;
    gap: 16px;
    padding: 20px 20px 18px;
    border-radius: var(--r-panel);
    border: 1px solid var(--line-strong);
    background: var(--card);
    box-shadow: var(--shadow-float);
  }
  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
  }
  h3 {
    margin: 0;
    font-size: 16px;
    line-height: 24px;
    font-weight: 500;
  }
  p {
    margin: 0;
  }
  .sub {
    font-size: 12px;
    color: var(--text-3);
  }
  .sep {
    margin: 0 6px;
  }
  section {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    gap: 8px;
    min-height: 0;
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    overflow-y: auto;
    max-height: 340px;
  }
  li {
    padding: 8px 14px;
    font-size: 13px;
    line-height: 19px;
    color: var(--text-2);
  }
  .hint {
    font-size: 12px;
    color: var(--text-3);
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .err {
    padding: 10px 12px;
    border-radius: var(--r-control);
    background: var(--sunken);
  }
  .progress {
    display: grid;
    gap: 6px;
    font-size: 12px;
    color: var(--text-2);
  }
  .line {
    display: flex;
    justify-content: space-between;
    gap: 12px;
  }
  .bar {
    height: 3px;
    border-radius: 2px;
    background: var(--raised);
    overflow: hidden;
  }
  .bar i {
    display: block;
    height: 100%;
    background: var(--text-2);
    transform-origin: left;
    transition: transform var(--menu) var(--ease);
  }
  .bar i.unknown {
    width: 30%;
    animation: slide 1.2s var(--ease) infinite;
  }
  @keyframes slide {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(340%);
    }
  }
</style>
