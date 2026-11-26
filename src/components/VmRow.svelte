<script lang="ts">
  import { Monitor, Play, SquareTerminal, Ellipsis, Square, RotateCcw, Power, FolderOpen } from "@lucide/svelte";
  import type { Machine, Vm, VmStatus } from "../lib/api";
  import { gb } from "../lib/format";
  import { t, vmStateLabel } from "../lib/i18n.svelte";
  import { previewKey, previews } from "../lib/previews.svelte";
  import Menu from "./Menu.svelte";
  import Tile from "./Tile.svelte";

  // One VM, or the desktop of the machine itself (vm null), as a row: a small live picture of
  // its screen, name and facts, and its actions. Hovering the picture shows it larger.
  let {
    machine,
    vm,
    vs = null,
    online,
    pending = null,
    busy = {},
    onOpen,
    onFiles,
    onAction,
  }: {
    machine: Machine;
    vm: Vm | null;
    vs?: VmStatus | null;
    online: boolean;
    pending?: string | null;
    busy?: Record<string, boolean>;
    onOpen: (kind: "screen" | "terminal") => void;
    onFiles?: () => void;
    onAction?: (action: "start" | "shutdown" | "reboot" | "destroy") => void;
  } = $props();

  const isDesktop = $derived(vm === null);
  const running = $derived(isDesktop ? online : !!vs?.running);
  const canScreen = $derived(running && !pending && (isDesktop || vm!.vnc_mode !== "none"));
  const canShell = $derived(running && !pending && (isDesktop || !!vm!.guest_ssh_user));
  const preview = $derived(previews[previewKey(machine.id, vm?.id ?? null)]);
  const dot = $derived(
    pending || vs?.state === "in shutdown" || vs?.state === "paused" ? "busy" : vs?.state === "crashed" ? "error" : running ? "on" : "",
  );
  const stateText = $derived(pending ? `${pending} …` : isDesktop ? t("vm.desktopRow") : vs ? vmStateLabel(vs.state) : "–");
  const facts = $derived(
    vm && vs
      ? [vs.vcpus ? t("vm.vcpus", vs.vcpus) : "", vs.memory ? gb(vs.memory) : "", vs.ip, running && vs.cpu_percent != null ? `CPU ${Math.round(vs.cpu_percent)} %` : ""].filter(Boolean)
      : isDesktop
        ? [`VNC :${machine.desktop_vnc_port}`]
        : [vm?.libvirt_name ?? ""],
  );

  // The larger picture floats fixed to the window, so the rounded group around the rows (which
  // clips its children) cannot cut it off. It sits above the thumbnail, or below it when there is
  // no room, and stays inside the window sideways.
  const BIG_W = 360;
  const BIG_H = (BIG_W * 9) / 16;
  let bigPos = $state({ left: 0, top: 0 });
  function placeBig(e: PointerEvent) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const gap = 8;
    const top = r.top - gap - BIG_H >= gap ? r.top - gap - BIG_H : Math.min(r.bottom + gap, window.innerHeight - BIG_H - gap);
    const left = Math.max(gap, Math.min(r.left, window.innerWidth - BIG_W - gap));
    bigPos = { left, top };
  }
</script>

<div class="row" class:off={!running}>
  <button class="thumb" disabled={!canScreen} onclick={() => onOpen("screen")} onpointerenter={placeBig} onpointermove={placeBig} aria-label={t("vm.openScreen")}>
    {#if running && preview?.url}
      <img src={preview.url} alt="" draggable="false" />
      <span class="big" aria-hidden="true" style:left="{bigPos.left}px" style:top="{bigPos.top}px"><img src={preview.url} alt="" draggable="false" /></span>
    {:else}
      <Monitor size={14} strokeWidth={1.5} />
    {/if}
  </button>

  <div class="ident">
    <div class="name">
      {#if vm}<Tile name={vm.name} color={vm.color} size={16} />{/if}
      <span class="label">{isDesktop ? t("machine.desktop") : vm!.name}</span>
    </div>
    <div class="sub">
      <span class="state">{#if dot}<span class="dot {dot}"></span>{/if}{stateText}</span>
      {#each facts as f (f)}<span class="fact mono">{f}</span>{/each}
    </div>
  </div>

  <div class="actions">
    {#if vm && online && vs && !running && !pending}
      <button class="btn" disabled={busy.start} onclick={() => onAction?.("start")}><Play size={14} strokeWidth={1.75} />{t("vm.start")}</button>
    {:else if running}
      {#if canScreen}
        <button class="btn" onclick={() => onOpen("screen")}><Monitor size={14} strokeWidth={1.75} />{t("vm.screen")}</button>
      {/if}
      {#if canShell}
        <button class="btn icon ghost" title={t("machine.terminal")} aria-label={t("machine.terminal")} onclick={() => onOpen("terminal")}>
          <SquareTerminal size={15} strokeWidth={1.5} />
        </button>
        {#if vm && onFiles}
          <button class="btn icon ghost" title={t("nav.files")} aria-label={t("nav.files")} onclick={onFiles}>
            <FolderOpen size={15} strokeWidth={1.5} />
          </button>
        {/if}
      {/if}
      {#if vm}
        <Menu label={t("vm.more")}>
          {#snippet trigger()}<Ellipsis size={15} strokeWidth={1.5} />{/snippet}
          <button disabled={busy.shutdown} onclick={() => onAction?.("shutdown")}><Square size={15} strokeWidth={1.5} />{t("vm.shutdown")}</button>
          <button onclick={() => onAction?.("reboot")}><RotateCcw size={15} strokeWidth={1.5} />{t("vm.reboot")}</button>
          <button class="danger" onclick={() => onAction?.("destroy")}><Power size={15} strokeWidth={1.5} />{t("vm.destroy")}</button>
        </Menu>
      {/if}
    {/if}
  </div>
</div>

<style>
  .row {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 14px;
    padding: 10px 12px 10px 10px;
  }
  .thumb {
    position: relative;
    width: 80px;
    height: 45px;
    padding: 0;
    display: grid;
    place-items: center;
    border-radius: 6px;
    border: 1px solid var(--line);
    background: var(--sunken);
    color: var(--text-3);
    cursor: pointer;
  }
  .thumb:disabled {
    cursor: default;
  }
  .thumb > img {
    width: 100%;
    height: 100%;
    border-radius: 5px;
    object-fit: cover;
    object-position: top center;
  }
  .big {
    position: fixed;
    z-index: 30;
    width: 360px;
    aspect-ratio: 16 / 9;
    border-radius: 10px;
    overflow: hidden;
    background: var(--sunken);
    box-shadow: var(--shadow-float);
    opacity: 0;
    transform: translateY(4px);
    pointer-events: none;
    transition:
      opacity var(--fast) var(--ease),
      transform var(--fast) var(--ease);
  }
  .big img {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }
  .thumb:hover .big {
    opacity: 1;
    transform: none;
    transition-delay: 250ms;
  }
  .ident {
    min-width: 0;
    display: grid;
    gap: 2px;
  }
  .name {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .label {
    font-size: 14px;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .off .label,
  .off .thumb {
    opacity: 0.7;
  }
  .sub {
    display: flex;
    align-items: center;
    gap: 0 8px;
    min-width: 0;
    font-size: 12px;
    color: var(--text-3);
    white-space: nowrap;
    overflow: hidden;
  }
  .state {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--text-2);
  }
  .state .dot {
    width: 6px;
    height: 6px;
  }
  .fact {
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .fact::before {
    content: "·";
    margin-right: 8px;
    font-family: var(--font);
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 4px;
  }
</style>
