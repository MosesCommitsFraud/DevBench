<script lang="ts">
  import { ExternalLink, Copy, X, ArrowRight, RefreshCw, Plus } from "@lucide/svelte";
  import { api, asAppError, type ListenPort } from "../lib/api";
  import { t } from "../lib/i18n.svelte";
  import { store } from "../lib/store.svelte";
  import { notify, run } from "../lib/ui.svelte";
  import { browse, closeForward, forwardsOf, openForward, ports } from "../lib/ports.svelte";

  // Forwarding ports of one machine or guest to this PC: what is forwarded, what listens there
  // right now (one click to forward it), and any port by hand.

  const target = $derived(ports.dialog);
  const machine = $derived(target ? store.config.machines.find((m) => m.id === target.machine) : undefined);
  const vm = $derived(target?.vm ? machine?.vms.find((v) => v.id === target.vm) : undefined);
  const title = $derived(vm ? `${vm.name} · ${machine?.name}` : (machine?.name ?? ""));
  const active = $derived(target ? forwardsOf(target.machine, target.vm) : []);

  let found = $state<ListenPort[] | null>(null);
  let scanError = $state("");
  let scanning = $state(false);
  let manual = $state("");
  let busy = $state<Record<number, boolean>>({});

  async function scan() {
    if (!target) return;
    const key = target;
    scanning = true;
    scanError = "";
    try {
      const list = await api.listeningPorts(key.machine, key.vm);
      if (ports.dialog === key) found = list;
    } catch (e) {
      if (ports.dialog === key) scanError = asAppError(e).message;
    } finally {
      scanning = false;
    }
  }

  // A fresh scan every time the dialog opens for a system
  $effect(() => {
    if (!target) return;
    found = null;
    manual = "";
    void scan();
  });

  async function forward(port: number, host: string | null = null) {
    if (!target) return;
    busy[port] = true;
    try {
      const f = await run(() => openForward(target.machine, target.vm, port, host === "localhost" ? null : host));
      notify(t("ports.forwarded", port, f.local_port));
    } catch {
      // shown by run()
    } finally {
      busy[port] = false;
    }
  }

  async function addManual(e: SubmitEvent) {
    e.preventDefault();
    const port = Number(manual.trim());
    if (!Number.isInteger(port) || port < 1 || port > 65535) {
      notify(t("ports.invalid"), "error");
      return;
    }
    await forward(port);
    manual = "";
  }

  function copy(port: number) {
    void navigator.clipboard.writeText(`http://localhost:${port}`);
    notify(t("ports.copied", port));
  }

  function close() {
    ports.dialog = null;
  }

  function onKey(e: KeyboardEvent) {
    if (target && e.key === "Escape") close();
  }

  const isForwarded = (p: number) => active.some((f) => f.remote_port === p);
</script>

<svelte:window onkeydown={onKey} />

{#if target && machine}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="scrim" onclick={(e) => e.target === e.currentTarget && close()}>
    <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="ports-title">
      <header>
        <div>
          <h3 id="ports-title">{t("ports.title")}</h3>
          <p class="secondary">{title}</p>
        </div>
        <button class="btn ghost icon" aria-label={t("ports.close")} title={t("ports.close")} onclick={close}><X size={16} strokeWidth={1.5} /></button>
      </header>

      <section>
        <span class="eyebrow">{t("ports.active")}</span>
        {#if active.length}
          <div class="rows">
            {#each active as f (f.id)}
              <div class="fw">
                <span class="dot" class:on={!f.error} class:error={!!f.error}></span>
                <div class="what">
                  <span class="mono">localhost:{f.local_port}</span>
                  <span class="via mono"><ArrowRight size={12} strokeWidth={1.75} />{f.remote_host === "localhost" ? "" : f.remote_host}:{f.remote_port}</span>
                  {#if f.connections}<span class="conns">{t("ports.connections", f.connections)}</span>{/if}
                  {#if f.error}<span class="err">{f.error}</span>{/if}
                </div>
                <button class="btn ghost icon" title={t("ports.open")} aria-label={t("ports.open")} onclick={() => browse(f)}><ExternalLink size={15} strokeWidth={1.5} /></button>
                <button class="btn ghost icon" title={t("ports.copy")} aria-label={t("ports.copy")} onclick={() => copy(f.local_port)}><Copy size={15} strokeWidth={1.5} /></button>
                <button class="btn ghost icon" title={t("ports.stop")} aria-label={t("ports.stop")} onclick={() => run(() => closeForward(f)).catch(() => {})}><X size={15} strokeWidth={1.5} /></button>
              </div>
            {/each}
          </div>
        {:else}
          <p class="hint">{t("ports.none")}</p>
        {/if}
      </section>

      <section>
        <div class="head">
          <span class="eyebrow">{t("ports.listening")}</span>
          <button class="btn ghost icon tiny" title={t("files.refresh")} aria-label={t("files.refresh")} disabled={scanning} onclick={scan}>
            <span class:spin={scanning}><RefreshCw size={13} strokeWidth={1.75} /></span>
          </button>
        </div>
        {#if scanError}
          <p class="hint">{t("ports.scanFailed")} <span class="mono err">{scanError}</span></p>
        {:else if found === null}
          <p class="hint">{t("ports.scanning")} …</p>
        {:else if !found.length}
          <p class="hint">{t("ports.nothing")}</p>
        {:else}
          <div class="rows">
            {#each found as p (p.port)}
              <div class="fw">
                <span class="port mono">{p.port}</span>
                <div class="what">
                  <span>{p.process || t("ports.unknownProcess")}</span>
                  {#if p.host !== "localhost"}<span class="via mono">{p.host}</span>{/if}
                </div>
                {#if isForwarded(p.port)}
                  <span class="done">{t("ports.isForwarded")}</span>
                {:else}
                  <button class="btn" disabled={busy[p.port]} onclick={() => forward(p.port, p.host)}>{t("ports.forward")}</button>
                {/if}
              </div>
            {/each}
          </div>
        {/if}
      </section>

      <form class="manual" onsubmit={addManual}>
        <label class="field">
          <span>{t("ports.manual")}</span>
          <input bind:value={manual} inputmode="numeric" placeholder="3000" spellcheck="false" />
        </label>
        <button type="submit" class="btn primary" disabled={!manual.trim()}><Plus size={14} strokeWidth={1.75} />{t("ports.forward")}</button>
      </form>
      <p class="hint small">{t("ports.explain")}</p>
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
    width: min(520px, 100%);
    max-height: calc(100vh - 48px);
    overflow-y: auto;
    padding: 20px 20px 18px;
    border-radius: var(--r-panel);
    border: 1px solid var(--line-strong);
    background: var(--card);
    box-shadow: var(--shadow-float);
    display: grid;
    gap: 16px;
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
  section {
    display: grid;
    gap: 8px;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .tiny {
    width: 24px;
    height: 24px;
  }
  .fw {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 44px;
    padding: 6px 8px 6px 12px;
  }
  .what {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    gap: 2px 10px;
    font-size: 13px;
  }
  .via {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    color: var(--text-3);
  }
  .conns {
    font-size: 12px;
    color: var(--text-2);
  }
  .err {
    flex-basis: 100%;
    font-size: 12px;
    color: var(--danger);
    overflow-wrap: anywhere;
  }
  .port {
    min-width: 48px;
    font-size: 13px;
    font-weight: 500;
  }
  .done {
    font-size: 12px;
    color: var(--text-3);
    padding-right: 6px;
  }
  .hint {
    font-size: 13px;
    color: var(--text-3);
  }
  .hint.small {
    font-size: 12px;
  }
  .manual {
    display: flex;
    align-items: flex-end;
    gap: 8px;
  }
  .manual .field {
    flex: 1;
  }
  .fw .dot {
    width: 6px;
    height: 6px;
  }
  .spin {
    display: inline-flex;
    animation: spin 1s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
