<script lang="ts">
  import { Monitor, Play, SquareTerminal, Ellipsis, Square, RotateCcw, Power, Maximize2 } from "@lucide/svelte";
  import type { Machine, Vm, VmStatus } from "../lib/api";
  import { gb } from "../lib/format";
  import { t, vmStateLabel } from "../lib/i18n.svelte";
  import { previewKey, previews } from "../lib/previews.svelte";
  import Menu from "./Menu.svelte";
  import Sketch from "./Sketch.svelte";
  import Tile from "./Tile.svelte";

  // A VM, or the desktop of the machine itself (vm null), as a card with a live picture of
  // its screen. Clicking the picture opens the screen.
  let {
    machine,
    vm,
    vs = null,
    online,
    pending = null,
    busy = {},
    onOpen,
    onAction,
  }: {
    machine: Machine;
    vm: Vm | null;
    vs?: VmStatus | null;
    online: boolean;
    pending?: string | null;
    busy?: Record<string, boolean>;
    onOpen: (kind: "screen" | "terminal") => void;
    onAction?: (action: "start" | "shutdown" | "reboot" | "destroy") => void;
  } = $props();

  const isDesktop = $derived(vm === null);
  const running = $derived(isDesktop ? online : !!vs?.running);
  const canScreen = $derived(running && !pending && (isDesktop || vm!.vnc_mode !== "none"));
  const canTerminal = $derived(running && !pending && (isDesktop || !!vm!.guest_ssh_user));
  const preview = $derived(previews[previewKey(machine.id, vm?.id ?? null)]);
  const name = $derived(vm ? vm.name : t("machine.desktop"));
  const state = $derived(
    pending ? `${pending} …` : isDesktop ? t("machine.desktopOf", machine.name) : online && vs ? vmStateLabel(vs.state) : "–",
  );
  const led = $derived<"on" | "off" | "busy" | "error">(
    pending || vs?.state === "in shutdown" || vs?.state === "paused" ? "busy" : vs?.state === "crashed" ? "error" : running ? "on" : "off",
  );
</script>

<article class="card-shell" class:running>
  <button class="preview" disabled={!canScreen} onclick={() => onOpen("screen")} aria-label={t("vm.screen")}>
    {#if running && preview?.url}
      <img src={preview.url} alt="" draggable="false" />
      <span class="open"><Maximize2 size={14} strokeWidth={1.75} />{t("vm.openScreen")}</span>
    {:else if running && preview?.error}
      <div class="placeholder"><Sketch kind="lost" label={t("vm.noPicture")} size={52} /></div>
    {:else if running}
      <div class="placeholder shimmer"><Tile name={vm?.name ?? machine.name} color={vm?.color ?? machine.color} size={44} /></div>
    {:else}
      <div class="placeholder"><Sketch kind="off" label={pending ? `${pending} …` : t("vm.isOff")} size={52} /></div>
    {/if}
    {#if running && vs?.cpu_percent != null && !pending}
      <span class="cpu mono" title={t("machine.cpu")}>CPU {Math.round(vs.cpu_percent)} %</span>
    {/if}
  </button>

  <div class="info">
    {#if vm}
      <Tile name={vm.name} color={vm.color} size={30} status={led} />
    {:else}
      <span class="desk-icon"><Monitor size={16} strokeWidth={1.75} /></span>
    {/if}
    <div class="text">
      <span class="name">{name}</span>
      <span class="meta mono">
        {#if vm && vs?.vcpus}{t("vm.vcpus", vs.vcpus)} · {gb(vs.memory)}{#if vs.ip} · {vs.ip}{/if}{:else}{state}{/if}
      </span>
    </div>
    <div class="actions">
      {#if vm && online && vs && !running}
        <button class="btn primary small" disabled={busy.start || !!pending} onclick={() => onAction?.("start")}>
          <Play size={14} strokeWidth={2} />{t("vm.start")}
        </button>
      {:else if running}
        {#if canTerminal}
          <button class="btn icon small" title={t("machine.terminal")} aria-label={t("machine.terminal")} onclick={() => onOpen("terminal")}>
            <SquareTerminal size={16} strokeWidth={1.5} />
          </button>
        {/if}
        {#if vm}
          <Menu label={t("vm.more")}>
            {#snippet trigger()}<Ellipsis size={16} strokeWidth={1.5} />{/snippet}
            <button disabled={busy.shutdown} onclick={() => onAction?.("shutdown")}><Square size={16} strokeWidth={1.5} />{t("vm.shutdown")}</button>
            <button onclick={() => onAction?.("reboot")}><RotateCcw size={16} strokeWidth={1.5} />{t("vm.reboot")}</button>
            <button class="danger" onclick={() => onAction?.("destroy")}><Power size={16} strokeWidth={1.5} />{t("vm.destroy")}</button>
          </Menu>
        {/if}
      {/if}
    </div>
  </div>
</article>

<style>
  .card-shell {
    display: grid;
    grid-template-rows: auto auto;
    border-radius: var(--r-panel);
    background: var(--card);
    box-shadow:
      0 0 0 1px var(--line),
      var(--shadow-card);
    overflow: hidden;
    min-width: 0;
    transition:
      transform var(--fast) var(--ease),
      box-shadow var(--fast) var(--ease);
  }
  .card-shell.running:hover {
    box-shadow:
      0 0 0 1px var(--line-strong),
      var(--shadow-float);
  }
  .preview {
    position: relative;
    aspect-ratio: 16 / 9;
    display: block;
    width: 100%;
    padding: 0;
    border: none;
    border-bottom: 1px solid var(--line);
    background:
      radial-gradient(circle at 1px 1px, var(--line-strong) 1px, transparent 0) 0 0 / 14px 14px,
      var(--sunken);
    overflow: hidden;
    cursor: pointer;
  }
  .preview:disabled {
    cursor: default;
  }
  .preview img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    object-position: top center;
    transition: transform 400ms var(--ease);
  }
  .preview:not(:disabled):hover img {
    transform: scale(1.02);
  }
  .placeholder {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
  }
  .shimmer::after {
    content: "";
    position: absolute;
    inset: 0;
    background: linear-gradient(100deg, transparent 30%, color-mix(in srgb, var(--text) 6%, transparent) 50%, transparent 70%);
    background-size: 200% 100%;
    animation: shimmer 1.6s linear infinite;
  }
  .open {
    position: absolute;
    left: 50%;
    bottom: 12px;
    transform: translate(-50%, 6px);
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 12px;
    border-radius: 14px;
    font-size: 12px;
    font-weight: 500;
    color: #fff;
    background: rgb(10 14 26 / 0.72);
    backdrop-filter: blur(8px);
    opacity: 0;
    transition:
      opacity var(--fast) var(--ease),
      transform var(--fast) var(--ease);
  }
  .preview:not(:disabled):hover .open {
    opacity: 1;
    transform: translate(-50%, 0);
  }
  .cpu {
    position: absolute;
    top: 10px;
    right: 10px;
    height: 22px;
    padding: 0 8px;
    border-radius: 11px;
    display: inline-flex;
    align-items: center;
    font-size: 11px;
    color: #fff;
    background: rgb(10 14 26 / 0.62);
    backdrop-filter: blur(8px);
  }
  .info {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 12px 12px 14px;
    min-width: 0;
  }
  .desk-icon {
    width: 30px;
    height: 30px;
    flex: none;
    border-radius: 9px;
    display: grid;
    place-items: center;
    color: var(--text-2);
    background: var(--raised);
  }
  .text {
    display: grid;
    flex: 1;
    min-width: 0;
  }
  .name {
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .meta {
    color: var(--text-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .actions {
    display: flex;
    gap: 6px;
    flex: none;
  }
  .actions :global(.btn.small) {
    height: 30px;
  }
  .actions :global(.btn.icon.small) {
    width: 30px;
  }
  @keyframes shimmer {
    from {
      background-position: 200% 0;
    }
    to {
      background-position: -200% 0;
    }
  }
</style>
