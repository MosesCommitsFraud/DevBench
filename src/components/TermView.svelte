<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { Terminal } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import "@xterm/xterm/css/xterm.css";
  import { ArrowLeft, RefreshCw, Maximize, Minimize, ClipboardPaste } from "@lucide/svelte";
  import { api, type Machine, type SetupStep, type Vm } from "../lib/api";
  import { t } from "../lib/i18n.svelte";
  import { run } from "../lib/ui.svelte";
  import { theme, token } from "../lib/theme.svelte";

  // A shell on the machine (vm null) or in a guest, over SSH, inside the window. With `step`
  // the first connection runs that setup step's script (sudo asks for the password here);
  // reconnecting opens a plain shell.
  let { machine, vm, step = null, onClose }: { machine: Machine; vm: Vm | null; step?: SetupStep | null; onClose: () => void } = $props();
  // svelte-ignore state_referenced_locally
  let pendingStep: SetupStep | null = step;

  let root: HTMLDivElement;
  let host: HTMLDivElement;
  let term: Terminal | null = null;
  let fit: FitAddon | null = null;
  let ws: WebSocket | null = null;
  let observer: ResizeObserver | null = null;
  let phase = $state<"connecting" | "connected" | "closed">("connecting");
  let problem = $state("");
  let fullscreen = $state(false);

  const title = $derived([vm?.name, machine.name].filter(Boolean).join(" · "));

  function palette() {
    const dark = theme() === "dark";
    return {
      background: token("--sunken"),
      foreground: token("--text"),
      cursor: token("--on"),
      cursorAccent: token("--sunken"),
      selectionBackground: dark ? "rgba(58,108,240,0.35)" : "rgba(36,87,230,0.2)",
      black: dark ? "#1a2238" : "#0f1b3d",
      brightBlack: dark ? "#5b6480" : "#6b7287",
      blue: dark ? "#6b8ff2" : "#2457e6",
      brightBlue: dark ? "#8aa8f7" : "#1d47c0",
      cyan: dark ? "#2dd4bf" : "#0f8f80",
      brightCyan: dark ? "#5eead4" : "#12a594",
      green: dark ? "#7fd1a0" : "#1f8a4c",
      brightGreen: dark ? "#a3e4bc" : "#2a9d5c",
      yellow: dark ? "#e8c26a" : "#9a6a00",
      brightYellow: dark ? "#f0d48f" : "#b88300",
      red: dark ? "#ef7a7e" : "#c2363b",
      brightRed: dark ? "#f59a9d" : "#d94c51",
      magenta: dark ? "#c39af0" : "#7c3fc2",
      brightMagenta: dark ? "#d6b8f5" : "#9256d6",
      white: dark ? "#c8cdd9" : "#475170",
      brightWhite: dark ? "#eceef4" : "#0f1b3d",
    };
  }

  function sendSize() {
    if (term && ws?.readyState === WebSocket.OPEN) ws.send(JSON.stringify({ cols: term.cols, rows: term.rows }));
  }

  async function connect() {
    teardownSocket();
    phase = "connecting";
    problem = "";
    term!.reset();
    fit!.fit();
    let url: string;
    try {
      const s = pendingStep;
      pendingStep = null;
      url = (await run(() => api.termOpen(machine.id, vm?.id ?? null, term!.cols, term!.rows, s), true)).url;
    } catch (e) {
      phase = "closed";
      problem = (e as { message?: string }).message ?? String(e);
      return;
    }
    const s = new WebSocket(url);
    s.binaryType = "arraybuffer";
    s.onopen = () => {
      phase = "connected";
      sendSize();
      term?.focus();
    };
    s.onmessage = (e) => {
      if (e.data instanceof ArrayBuffer) term?.write(new Uint8Array(e.data));
      else term?.write(e.data);
    };
    s.onclose = () => {
      if (ws !== s) return;
      phase = "closed";
    };
    ws = s;
  }

  function teardownSocket() {
    const s = ws;
    ws = null;
    s?.close();
  }

  async function paste() {
    const text = await navigator.clipboard.readText().catch(() => "");
    if (text) term?.paste(text);
    term?.focus();
  }

  async function toggleFullscreen() {
    if (document.fullscreenElement) await document.exitFullscreen();
    else await root.requestFullscreen();
  }

  onMount(() => {
    const encoder = new TextEncoder();
    term = new Terminal({
      fontFamily: '"JetBrains Mono", "Cascadia Mono", Consolas, monospace',
      fontSize: 13,
      lineHeight: 1.25,
      cursorBlink: true,
      scrollback: 5000,
      allowProposedApi: false,
      theme: palette(),
    });
    fit = new FitAddon();
    term.loadAddon(fit);
    term.open(host);
    term.onData((d) => {
      if (ws?.readyState === WebSocket.OPEN) ws.send(encoder.encode(d));
    });
    term.onBinary((d) => {
      if (ws?.readyState === WebSocket.OPEN) ws.send(Uint8Array.from(d, (c) => c.charCodeAt(0)));
    });
    // Ctrl+Shift+C / V copy and paste, like other terminals; plain Ctrl+C stays an interrupt
    term.attachCustomKeyEventHandler((e) => {
      if (e.type === "keydown" && e.ctrlKey && e.shiftKey && e.code === "KeyC") {
        void navigator.clipboard.writeText(term!.getSelection());
        return false;
      }
      if (e.type === "keydown" && e.ctrlKey && e.shiftKey && e.code === "KeyV") {
        void paste();
        return false;
      }
      return true;
    });
    term.onResize(sendSize);
    observer = new ResizeObserver(() => fit?.fit());
    observer.observe(host);
    void connect();
  });

  // Follow the app's theme while open
  $effect(() => {
    void theme();
    if (term) term.options.theme = palette();
  });

  onDestroy(() => {
    observer?.disconnect();
    teardownSocket();
    term?.dispose();
  });
</script>

<svelte:document onfullscreenchange={() => (fullscreen = !!document.fullscreenElement)} />

<div class="term" bind:this={root}>
  <header>
    <button class="btn ghost" onclick={onClose}><ArrowLeft size={16} strokeWidth={1.5} />{t("vnc.close")}</button>
    <div class="title">
      <span class="led" class:on={phase === "connected"} class:busy={phase === "connecting"}></span>
      <span>{title}</span>
      <span class="mono muted">{t("machine.terminal")}</span>
    </div>
    <div class="tools">
      <button class="btn ghost icon" title={t("term.paste")} aria-label={t("term.paste")} onclick={paste}>
        <ClipboardPaste size={16} strokeWidth={1.5} />
      </button>
      <button class="btn ghost icon" title={t("vnc.fullscreen")} aria-label={t("vnc.fullscreen")} onclick={toggleFullscreen}>
        {#if fullscreen}<Minimize size={16} strokeWidth={1.5} />{:else}<Maximize size={16} strokeWidth={1.5} />{/if}
      </button>
    </div>
  </header>
  <div class="stage">
    <div class="host" bind:this={host}></div>
    {#if phase === "closed"}
      <div class="state">
        <p class="secondary">{t("term.closed")}</p>
        {#if problem}<p class="mono muted problem">{problem}</p>{/if}
        <button class="btn primary" onclick={connect}><RefreshCw size={16} strokeWidth={1.5} />{t("vnc.reconnect")}</button>
      </div>
    {/if}
  </div>
</div>

<style>
  .term {
    position: fixed;
    inset: 40px 0 0 0;
    z-index: 40;
    display: grid;
    grid-template-rows: 48px minmax(0, 1fr);
    background: var(--sunken);
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
  .stage {
    position: relative;
    min-height: 0;
    padding: 12px 4px 4px 16px;
  }
  .host {
    width: 100%;
    height: 100%;
  }
  .host :global(.xterm-viewport) {
    background: transparent !important;
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
    background: color-mix(in srgb, var(--sunken) 88%, transparent);
  }
  .state p {
    margin: 0;
  }
  .problem {
    max-width: 560px;
    overflow-wrap: anywhere;
  }
</style>
