<script lang="ts">
  import { X, RefreshCw, Code, ArrowUpCircle, Wrench, ScrollText } from "@lucide/svelte";
  import { api, type Machine, type SetupStep } from "../lib/api";
  import { lang, t } from "../lib/i18n.svelte";
  import { checkServer, servers } from "../lib/server.svelte";
  import { live, setConfig, store } from "../lib/store.svelte";
  import ErrorNote from "./ErrorNote.svelte";
  import { run } from "../lib/ui.svelte";

  // What DevBench needs on a server, what is there, and the updates waiting. Installing and
  // updating run in DevBench's terminal: sudo asks for the password there, every command shows.
  let { machine, onClose, onRun }: { machine: Machine; onClose: () => void; onRun: (step: SetupStep) => void } = $props();

  const s = $derived(servers[machine.id]);
  const c = $derived(s?.check ?? null);
  // Live from the status poll (every few seconds); the check only knows it at the time it ran.
  const updating = $derived(live(machine.id).status?.updating ?? null);
  const running = $derived(!!updating?.devbench || (!!c?.updating && !live(machine.id).status));
  const last = $derived(c?.last_update ?? null);

  function when(secs: number): string {
    return new Date(secs * 1000).toLocaleString(lang() === "de" ? "de-DE" : "en-GB", { day: "2-digit", month: "2-digit", hour: "2-digit", minute: "2-digit" });
  }
  const lastText = $derived.by(() => {
    if (!last?.started || running) return "";
    const at = when(last.finished ?? last.started);
    if (last.code == null) return t("update.lastCut", at);
    return last.code === 0 ? t("update.lastOk", at, last.upgraded) : t("update.lastFailed", at, last.code);
  });
  let shown = $state<SetupStep | null>(null);
  let script = $state("");

  $effect(() => {
    void checkServer(machine.id);
  });

  async function toggleScript(step: SetupStep) {
    if (shown === step) {
      shown = null;
      return;
    }
    script = (await api.setupScript(step)) ?? "";
    shown = step;
  }

  async function start(step: SetupStep) {
    // The virtual desktop listens on 5901: point the machine's desktop there if it has none yet.
    if (step === "desktop" && !machine.desktop_vnc_port) {
      const config = $state.snapshot(store.config);
      const m = config.machines.find((x) => x.id === machine.id);
      if (m) {
        m.desktop_vnc_port = 5901;
        await run(async () => setConfig(await api.saveConfig(config))).catch(() => {});
      }
    }
    onRun(step);
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") onClose();
  }

  const names: Record<string, () => string> = {
    libvirt: () => t("setup.libvirt"),
    power: () => t("setup.power"),
    desktop: () => t("setup.desktop"),
  };
  const what: Record<string, () => string> = {
    libvirt: () => t("setup.libvirt.what"),
    power: () => t("setup.power.what"),
    desktop: () => t("setup.desktop.what"),
  };
</script>

<svelte:window onkeydown={onKey} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="scrim" onclick={(e) => e.target === e.currentTarget && onClose()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="setup-title">
    <header>
      <div>
        <h3 id="setup-title">{t("setup.title")}</h3>
        <p class="secondary">{machine.name}</p>
      </div>
      <div class="head-actions">
        <button class="btn ghost icon" title={t("setup.recheck")} aria-label={t("setup.recheck")} disabled={s?.loading} onclick={() => checkServer(machine.id)}>
          <span class:spin={s?.loading}><RefreshCw size={15} strokeWidth={1.5} /></span>
        </button>
        <button class="btn ghost icon" aria-label={t("ports.close")} title={t("ports.close")} onclick={onClose}><X size={16} strokeWidth={1.5} /></button>
      </div>
    </header>

    {#if s?.error}<div class="err"><ErrorNote message={s.error} /></div>{/if}

    {#if !c}
      <p class="hint">{t("setup.checking")} …</p>
    {:else}
      <section>
        <span class="eyebrow">{t("setup.updates")}</span>
        <div class="rows">
          {#if running}
            <div class="part">
              <span class="dot busy"></span>
              <div class="what">
                <span class="name">{t("update.running")} …</span>
                {#if updating?.line}<span class="detail mono line" title={updating.line}>{updating.line}</span>{/if}
              </div>
              <button class="btn primary" onclick={() => onRun("update-log")}><ScrollText size={14} strokeWidth={1.75} />{t("update.follow")}</button>
            </div>
          {:else}
          <div class="part">
            <span class="dot" class:on={!c.updates && !c.snap_updates} class:busy={!!(c.updates || c.snap_updates)}></span>
            <div class="what">
              <span class="name">
                {#if c.updates || c.snap_updates}
                  {[c.updates ? t("setup.packages", c.updates) : "", c.snap_updates ? t("setup.snaps", c.snap_updates) : ""].filter(Boolean).join(", ")}
                {:else}
                  {t("setup.upToDate")}
                {/if}
              </span>
              {#if c.security_updates}<span class="detail warn">{t("setup.security", c.security_updates)}</span>{/if}
              {#if c.reboot_required}<span class="detail warn">{t("setup.reboot")}</span>{/if}
              {#if updating && !updating.devbench}<span class="detail warn">{t("update.system", updating.process)}</span>{/if}
              {#if c.packages.length}
                <span class="detail mono pkgs" title={c.packages.join(" ")}>{t("update.waiting")}: {c.packages.join(", ")}</span>
              {/if}
              {#if lastText}<span class="detail" class:warn={last?.code !== 0}>{lastText}</span>{/if}
              {#if last?.phased && c.updates}<span class="detail">{t("update.phased")}</span>{/if}
              {#if last?.kept_back && c.updates}<span class="detail">{t("update.keptBack")}</span>{/if}
            </div>
            {#if last}
              <button class="btn ghost icon" title={t("update.log")} aria-label={t("update.log")} onclick={() => onRun("update-log")}><ScrollText size={15} strokeWidth={1.5} /></button>
            {/if}
            <button class="btn ghost icon" title={t("setup.showScript")} aria-label={t("setup.showScript")} onclick={() => toggleScript("updates")}><Code size={15} strokeWidth={1.5} /></button>
            <button class="btn" class:primary={!!(c.updates || c.snap_updates)} disabled={!c.apt} onclick={() => start("updates")}>
              <ArrowUpCircle size={14} strokeWidth={1.75} />{t("setup.update")}
            </button>
          </div>
          {/if}
          {#if shown === "updates"}<pre class="script mono">{script}</pre>{/if}
        </div>
      </section>

      <section>
        <span class="eyebrow">{t("setup.parts")}</span>
        <div class="rows">
          {#each c.parts as p (p.id)}
            <div class="part">
              <span class="dot" class:on={p.ok}></span>
              <div class="what">
                <span class="name">{names[p.id]?.() ?? p.id}</span>
                <span class="detail">{what[p.id]?.()}</span>
                <span class="detail mono" class:warn={!p.ok}>{p.detail}</span>
              </div>
              <button class="btn ghost icon" title={t("setup.showScript")} aria-label={t("setup.showScript")} onclick={() => toggleScript(p.id)}><Code size={15} strokeWidth={1.5} /></button>
              {#if p.ok}
                <span class="done">{t("setup.ready")}</span>
              {:else}
                <button class="btn" disabled={!c.apt} onclick={() => start(p.id)}><Wrench size={14} strokeWidth={1.75} />{t("setup.install")}</button>
              {/if}
            </div>
            {#if shown === p.id}<pre class="script mono">{script}</pre>{/if}
          {/each}
        </div>
      </section>

      <p class="hint small">{c.apt ? t("setup.explain") : t("setup.noApt")}</p>
    {/if}
  </div>
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
  }
  .dialog {
    width: min(600px, 100%);
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
  .head-actions {
    display: flex;
    gap: 2px;
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
  .part {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 52px;
    padding: 8px 8px 8px 14px;
  }
  .part .dot {
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
  .warn {
    color: var(--busy);
  }
  .done {
    font-size: 12px;
    color: var(--text-3);
    padding: 0 8px;
  }
  .script {
    margin: 0;
    padding: 10px 14px;
    max-height: 240px;
    overflow: auto;
    font-size: 11.5px;
    line-height: 1.5;
    background: var(--sunken);
    color: var(--text-2);
    white-space: pre;
  }
  .hint {
    font-size: 13px;
    color: var(--text-3);
  }
  .hint.small {
    font-size: 12px;
  }
  .err {
    padding: 10px 12px;
    border-radius: var(--r-control);
    background: var(--sunken);
  }
  .line,
  .pkgs {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
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
