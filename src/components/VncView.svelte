<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import RFB from "@novnc/novnc";
  import { ArrowLeft, Maximize, Minimize, Scan, ClipboardPaste, RefreshCw, Keyboard } from "@lucide/svelte";
  import { api, type Machine, type Vm } from "../lib/api";
  import { t } from "../lib/i18n.svelte";
  import { run } from "../lib/ui.svelte";

  // A VM's screen, or the machine's own desktop when vm is null
  let { machine, vm, onClose }: { machine: Machine; vm: Vm | null; onClose: () => void } = $props();

  let root: HTMLDivElement;
  let screen: HTMLDivElement;
  let rfb: RFB | null = null;
  let phase = $state<"connecting" | "connected" | "disconnected" | "password">("connecting");
  let problem = $state("");
  let desktopTitle = $state("");
  const title = $derived(desktopTitle || [vm?.name, machine.name].filter(Boolean).join(" · "));
  let fit = $state(localStorage.getItem("devbench.vnc.fit") !== "0");
  let fullscreen = $state(false);
  let password = $state("");

  async function connect() {
    teardown();
    phase = "connecting";
    problem = "";
    let url: string;
    try {
      const target = await run(() => api.vncOpen(machine.id, vm?.id ?? null), true);
      url = target.url;
      desktopTitle = target.title;
    } catch (e) {
      phase = "disconnected";
      problem = (e as { message?: string }).message ?? String(e);
      return;
    }
    const r = new RFB(screen, url);
    r.scaleViewport = fit;
    r.clipViewport = !fit;
    r.resizeSession = false;
    r.focusOnClick = true;
    r.background = "transparent";
    r.addEventListener("connect", () => {
      phase = "connected";
      r.focus();
    });
    r.addEventListener("disconnect", (e) => {
      if (rfb !== r) return;
      phase = "disconnected";
      if (!e.detail.clean && !problem) problem = t("vnc.disconnected");
    });
    r.addEventListener("credentialsrequired", () => (phase = "password"));
    r.addEventListener("securityfailure", (e) => (problem = t("vnc.security", e.detail.reason ?? String(e.detail.status))));
    r.addEventListener("desktopname", (e) => (desktopTitle = `${e.detail.name} · ${machine.name}`));
    r.addEventListener("clipboard", (e) => {
      void navigator.clipboard?.writeText(e.detail.text).catch(() => {});
    });
    rfb = r;
  }

  function teardown() {
    const r = rfb;
    rfb = null;
    if (r) {
      try {
        r.disconnect();
      } catch {
        // already closed
      }
    }
  }

  function sendPassword(e: SubmitEvent) {
    e.preventDefault();
    rfb?.sendCredentials({ password });
    password = "";
    phase = "connecting";
  }

  function toggleFit() {
    fit = !fit;
    localStorage.setItem("devbench.vnc.fit", fit ? "1" : "0");
    if (rfb) {
      rfb.scaleViewport = fit;
      rfb.clipViewport = !fit;
    }
  }

  async function toggleFullscreen() {
    if (document.fullscreenElement) await document.exitFullscreen();
    else await root.requestFullscreen();
  }

  async function pasteClipboard() {
    const text = await navigator.clipboard.readText().catch(() => "");
    if (text && rfb) {
      rfb.clipboardPasteFrom(text);
      rfb.focus();
    }
  }

  function onFullscreenChange() {
    fullscreen = !!document.fullscreenElement;
  }

  onMount(() => {
    void connect();
  });
  onDestroy(teardown);
</script>

<svelte:document onfullscreenchange={onFullscreenChange} />

<div class="vnc" bind:this={root}>
  <header>
    <button class="btn ghost" onclick={onClose}><ArrowLeft size={16} strokeWidth={1.5} />{t("vnc.close")}</button>
    <div class="title">
      <span class="led" class:on={phase === "connected"} class:busy={phase === "connecting" || phase === "password"}></span>
      <span>{title}</span>
    </div>
    <div class="tools">
      {#if phase === "connected"}
        <button class="btn ghost" title={t("vnc.cad")} onclick={() => rfb?.sendCtrlAltDel()}>
          <Keyboard size={16} strokeWidth={1.5} /><span class="keycap">{t("vnc.cad")}</span>
        </button>
        <button class="btn ghost icon" title={t("vnc.paste")} aria-label={t("vnc.paste")} onclick={pasteClipboard}>
          <ClipboardPaste size={16} strokeWidth={1.5} />
        </button>
        <button class="btn ghost icon" title={fit ? t("vnc.actual") : t("vnc.fit")} aria-label={fit ? t("vnc.actual") : t("vnc.fit")} aria-pressed={fit} onclick={toggleFit}>
          <Scan size={16} strokeWidth={1.5} />
        </button>
      {/if}
      <button class="btn ghost icon" title={t("vnc.fullscreen")} aria-label={t("vnc.fullscreen")} onclick={toggleFullscreen}>
        {#if fullscreen}<Minimize size={16} strokeWidth={1.5} />{:else}<Maximize size={16} strokeWidth={1.5} />{/if}
      </button>
    </div>
  </header>

  <div class="stage">
    <div class="screen" bind:this={screen}></div>
    {#if phase !== "connected"}
      <div class="state">
        {#if phase === "connecting"}
          <p class="secondary">{t("vnc.connecting", vm?.name ?? machine.name)}</p>
        {:else if phase === "password"}
          <form onsubmit={sendPassword}>
            <label class="field">
              <span>{t("vnc.password")}</span>
              <!-- svelte-ignore a11y_autofocus -->
              <input type="password" bind:value={password} autofocus />
              <small>{t("vnc.password.body")}</small>
            </label>
            <button class="btn primary" type="submit">{t("vnc.connect")}</button>
          </form>
        {:else}
          <p class="secondary">{t("vnc.disconnected")}</p>
          {#if problem}<p class="mono muted problem">{problem}</p>{/if}
          <button class="btn primary" onclick={connect}><RefreshCw size={16} strokeWidth={1.5} />{t("vnc.reconnect")}</button>
        {/if}
      </div>
    {/if}
  </div>
</div>

<style>
  .vnc {
    position: fixed;
    inset: 40px 0 0 0;
    z-index: 40;
    display: grid;
    grid-template-rows: 48px minmax(0, 1fr);
    background: var(--bg-deep);
    animation: enter var(--enter) var(--ease);
  }
  header {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    gap: 16px;
    padding: 0 8px;
    border-bottom: 1px solid var(--line);
    background: var(--bg);
  }
  .title {
    display: flex;
    align-items: center;
    gap: 10px;
    font-weight: 500;
    white-space: nowrap;
  }
  .tools {
    display: flex;
    justify-content: flex-end;
    gap: 4px;
  }
  .tools .keycap {
    height: 20px;
  }
  .stage {
    position: relative;
    min-height: 0;
  }
  .screen {
    position: absolute;
    inset: 0;
  }
  .state {
    position: absolute;
    inset: 0;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 12px;
    text-align: center;
    padding: 24px;
  }
  .state p {
    margin: 0;
  }
  .problem {
    max-width: 560px;
    overflow-wrap: anywhere;
  }
  form {
    display: grid;
    gap: 12px;
    width: 280px;
    text-align: left;
  }
</style>
