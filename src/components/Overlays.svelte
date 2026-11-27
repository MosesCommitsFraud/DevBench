<script lang="ts">
  import { X } from "@lucide/svelte";
  import { t } from "../lib/i18n.svelte";
  import { dismiss, ui } from "../lib/ui.svelte";
  import ErrorNote from "./ErrorNote.svelte";

  function onKey(e: KeyboardEvent) {
    if (e.key !== "Escape") return;
    if (ui.prompt) ui.prompt.resolve(null);
    else if (ui.confirm) ui.confirm.resolve(false);
    else if (ui.hostKey) ui.hostKey.resolve(false);
  }
</script>

<svelte:window onkeydown={onKey} />

{#if ui.confirm}
  {@const c = ui.confirm}
  <div class="scrim">
    <div class="dialog" role="alertdialog" aria-modal="true" aria-labelledby="confirm-title">
      <h3 id="confirm-title">{c.title}</h3>
      <p class="secondary">{c.body}</p>
      <div class="buttons">
        <!-- Focus starts on Cancel, so a stray Enter never confirms -->
        <!-- svelte-ignore a11y_autofocus -->
        <button class="btn ghost" autofocus onclick={() => c.resolve(false)}>{t("confirm.cancel")}</button>
        <!-- svelte-ignore a11y_autofocus -->
        <button class="btn" class:danger={c.danger} class:primary={!c.danger} onclick={() => c.resolve(true)}>{c.action}</button>
      </div>
    </div>
  </div>
{/if}

{#if ui.prompt}
  {@const p = ui.prompt}
  <div class="scrim">
    <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="prompt-title">
    <form class="form" onsubmit={(e) => { e.preventDefault(); p.resolve(p.value); }}>
      <h3 id="prompt-title">{p.title}</h3>
      <label class="field">
        <span>{p.label}</span>
        <!-- svelte-ignore a11y_autofocus -->
        <input bind:value={p.value} autofocus spellcheck="false" onfocus={(e) => (e.currentTarget as HTMLInputElement).select()} />
      </label>
      <div class="buttons">
        <button type="button" class="btn ghost" onclick={() => p.resolve(null)}>{t("confirm.cancel")}</button>
        <button type="submit" class="btn primary">{p.action}</button>
      </div>
    </form>
    </div>
  </div>
{/if}

{#if ui.hostKey}
  {@const h = ui.hostKey}
  <div class="scrim">
    <div class="dialog" role="alertdialog" aria-modal="true" aria-labelledby="hk-title">
      {#if h.error.kind === "host_key_unknown"}
        <h3 id="hk-title">{t("hostkey.title")}</h3>
        <p class="secondary">{t("hostkey.body", h.error.port === 22 ? h.error.host : `${h.error.host}:${h.error.port}`)}</p>
        <code class="fp mono">{h.error.fingerprint}</code>
        <div class="buttons">
          <button class="btn ghost" onclick={() => h.resolve(false)}>{t("confirm.cancel")}</button>
          <button class="btn primary" onclick={() => h.resolve(true)}>{t("hostkey.trust")}</button>
        </div>
      {:else}
        <h3 id="hk-title">{t("hostkey.changed.title")}</h3>
        <p class="secondary">{h.error.message}</p>
        <code class="fp mono">{h.error.fingerprint}</code>
        <div class="buttons">
          <button class="btn primary" onclick={() => h.resolve(false)}>OK</button>
        </div>
      {/if}
    </div>
  </div>
{/if}

<div class="notices" aria-live="polite">
  {#each ui.notices as n (n.id)}
    <div class="notice" class:error={n.tone === "error"}>
      {#if n.tone === "error"}<div class="text"><ErrorNote message={n.text} /></div>{:else}<span class="text">{n.text}</span>{/if}
      <button class="btn ghost icon" aria-label="Dismiss" onclick={() => dismiss(n.id)}><X size={16} strokeWidth={1.5} /></button>
    </div>
  {/each}
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    padding: 24px;
    background: rgb(0 0 0 / 0.5);
    animation: fade var(--menu) var(--ease);
  }
  .dialog {
    width: min(440px, 100%);
    padding: 24px;
    border-radius: var(--r-panel);
    border: 1px solid var(--line-strong);
    background: var(--card);
    box-shadow: var(--shadow-float);
    display: grid;
    gap: 12px;
    animation: rise var(--enter) var(--ease);
  }
  .form {
    display: grid;
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
  .fp {
    padding: 8px 12px;
    border-radius: var(--r-control);
    background: var(--raised);
    word-break: break-all;
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 8px;
  }
  .notices {
    position: fixed;
    right: 24px;
    bottom: 24px;
    z-index: 60;
    display: grid;
    gap: 8px;
    width: min(420px, calc(100vw - 48px));
  }
  .notice {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
    padding: 10px 8px 10px 16px;
    border-radius: 12px;
    border: 1px solid var(--line-strong);
    background: var(--card);
    box-shadow: var(--shadow-float);
    animation: rise var(--enter) var(--ease);
    overflow-wrap: anywhere;
  }
  .notice .text {
    flex: 1;
    min-width: 0;
    padding-top: 6px;
  }
  .notice.error {
    border-color: color-mix(in srgb, var(--danger) 40%, transparent);
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
  }
</style>
