<script lang="ts">
  import { Power, RotateCcw, Zap, KeyRound, SquareTerminal, Cpu, MemoryStick, HardDrive, ArrowDownUp, ArrowDown, ArrowUp, FolderOpen } from "@lucide/svelte";
  import { api, type Machine, type Vm } from "../lib/api";
  import { bytes, duration, rate } from "../lib/format";
  import { t } from "../lib/i18n.svelte";
  import { boost, live, poll } from "../lib/store.svelte";
  import { askHostKey, confirm, notify, run } from "../lib/ui.svelte";
  import Menu from "./Menu.svelte";
  import StatTile from "./StatTile.svelte";
  import ScreenCard from "./ScreenCard.svelte";
  import Sketch from "./Sketch.svelte";
  import Tile from "./Tile.svelte";

  let {
    machine,
    onOpen,
    onFiles,
  }: {
    machine: Machine;
    onOpen: (kind: "screen" | "terminal", vm: Vm | null) => void;
    onFiles: (vm: Vm | null) => void;
  } = $props();

  const l = $derived(live(machine.id));
  const st = $derived(l.status);
  const online = $derived(!!st?.online);
  const hostKeyError = $derived(l.error && l.error.kind !== "error" ? l.error : null);

  let busy = $state<Record<string, boolean>>({});

  async function act(key: string, fn: () => Promise<unknown>, done?: string): Promise<boolean> {
    busy[key] = true;
    try {
      await run(fn);
      if (done) notify(done);
      boost(machine.id);
      return true;
    } catch {
      // shown by run()
      return false;
    } finally {
      busy[key] = false;
    }
  }

  async function wake() {
    await act("wake", () => api.wake(machine.id), t("machine.woke", machine.name));
    l.pending = t("machine.waking");
    setTimeout(() => (l.pending = null), 90000);
  }

  async function power(action: "poweroff" | "reboot") {
    const ok = await confirm(
      t(action === "poweroff" ? "machine.poweroff.confirm" : "machine.reboot.confirm", machine.name),
      t(action === "poweroff" ? "machine.poweroff.body" : "machine.reboot.body"),
      t(action === "poweroff" ? "machine.poweroff" : "machine.reboot"),
    );
    if (ok) await act(action, () => api.power(machine.id, action));
  }

  interface Pending {
    label: string;
    wasRunning: boolean;
    action: string;
    since: number;
  }
  let vmPending = $state<Record<string, Pending>>({});

  async function vmAction(vm: Vm, action: "start" | "shutdown" | "reboot" | "destroy") {
    if (action === "destroy" || action === "shutdown") {
      const ok = await confirm(
        t(action === "destroy" ? "vm.destroy.confirm" : "vm.shutdown.confirm", vm.name),
        t(action === "destroy" ? "vm.destroy.body" : "vm.shutdown.body"),
        t(action === "destroy" ? "vm.destroy" : "vm.shutdown"),
        action === "destroy",
      );
      if (!ok) return;
    }
    // Show the transition right away; it clears once libvirt reports the new state.
    const label = { start: "vm.starting", shutdown: "vm.stopping", reboot: "vm.restarting", destroy: "vm.stopping" } as const;
    const wasRunning = !!vmStatus(vm)?.running;
    vmPending[vm.id] = { label: t(label[action]), wasRunning, action, since: Date.now() };
    if (!(await act(`${vm.id}:${action}`, () => api.vmAction(machine.id, vm.id, action)))) delete vmPending[vm.id];
  }

  // A pending transition ends when the VM reached the other state (or after two minutes).
  $effect(() => {
    for (const [id, p] of Object.entries(vmPending)) {
      const vs = st?.vms.find((v) => v.id === id);
      const reboot = p.action === "reboot";
      const done = reboot ? Date.now() - p.since > 8000 && !!vs?.running : vs && vs.running !== p.wasRunning;
      if (done || Date.now() - p.since > 120000) delete vmPending[id];
    }
  });

  async function checkHostKey() {
    if (hostKeyError && (await askHostKey(hostKeyError))) void poll(machine.id);
  }

  function vmStatus(vm: Vm) {
    return st?.vms.find((v) => v.id === vm.id) ?? null;
  }

  function vmBusy(vm: Vm) {
    return { start: !!busy[`${vm.id}:start`], shutdown: !!busy[`${vm.id}:shutdown`] };
  }

  const memUsed = $derived(st ? st.mem_total - st.mem_available : 0);
  const ratio = (used: number, total: number) => (total > 0 ? Math.min(1, used / total) : 0);
  const facts = $derived(st ? [st.os, st.ip || st.hostname].filter(Boolean) : []);
  const target = $derived(machine.ssh_user ? `${machine.ssh_user}@${machine.ssh_host}` : machine.ssh_host);
  const runningVms = $derived(st?.vms.filter((v) => v.running).length ?? 0);
  const ledState = $derived<"on" | "off" | "busy">(online ? "on" : l.checking || l.pending ? "busy" : "off");
  const screens = $derived(machine.vms.length + (machine.desktop_vnc_port ? 1 : 0));
</script>

<section class="panel" class:offline={!online && !l.checking}>
  <header>
    <Tile name={machine.name} color={machine.color} size={44} status={ledState} />
    <div class="ident">
      <div class="title">
        <h2>{machine.name}</h2>
        {#if online}
          <span class="pill on"><i></i>{t("machine.up", duration(st!.uptime_secs))}</span>
        {:else if l.pending}
          <span class="pill busy"><i></i>{l.pending} …</span>
        {:else if l.checking}
          <span class="pill"><i></i>{t("machine.checking")} …</span>
        {:else}
          <span class="pill"><i></i>{t("machine.offline")}</span>
        {/if}
      </div>
      <p class="facts mono">
        {#if online}{#each facts as f, i (i)}<span>{f}</span>{/each}{:else}<span>{target}</span>{/if}
      </p>
    </div>

    <div class="actions">
      {#if online}
        <button class="btn" onclick={() => onOpen("terminal", null)}>
          <SquareTerminal size={16} strokeWidth={1.5} />{t("machine.terminal")}
        </button>
        <button class="btn" onclick={() => onFiles(null)}>
          <FolderOpen size={16} strokeWidth={1.5} />{t("nav.files")}
        </button>
        <Menu label={t("machine.power")}>
          {#snippet trigger()}<Power size={16} strokeWidth={1.5} />{/snippet}
          <button disabled={busy.reboot} onclick={() => power("reboot")}><RotateCcw size={16} strokeWidth={1.5} />{t("machine.reboot")}</button>
          <button class="danger" disabled={busy.poweroff} onclick={() => power("poweroff")}><Power size={16} strokeWidth={1.5} />{t("machine.poweroff")}</button>
        </Menu>
      {:else if !l.checking && hostKeyError}
        <button class="btn" onclick={checkHostKey}><KeyRound size={16} strokeWidth={1.5} />{t("machine.trust")}</button>
      {/if}
    </div>
  </header>

  {#if online && st}
    <div class="stats">
      <StatTile icon={Cpu} tone="blue" label={t("machine.cpu")} value={String(Math.round(st.cpu_percent))} unit="%" ratio={st.cpu_percent / 100} detail={st.cpu_model} />
      <StatTile
        icon={MemoryStick}
        tone="violet"
        label={t("machine.memory")}
        value={String(Math.round(ratio(memUsed, st.mem_total) * 100))}
        unit="%"
        ratio={ratio(memUsed, st.mem_total)}
        detail={`${bytes(memUsed)} / ${bytes(st.mem_total)}`}
      />
      <StatTile
        icon={HardDrive}
        tone="amber"
        label={t("machine.disk")}
        value={String(Math.round(ratio(st.disk_used, st.disk_total) * 100))}
        unit="%"
        ratio={ratio(st.disk_used, st.disk_total)}
        detail={t("machine.freeOf", bytes(Math.max(0, st.disk_total - st.disk_used)), bytes(st.disk_total))}
      />
      <StatTile icon={ArrowDownUp} tone="teal" label={t("machine.network")} value="" detail={st.iface}>
        <div class="rates">
          <span title={t("machine.download")}><ArrowDown size={15} strokeWidth={2} />{rate(st.net_rx_rate)}</span>
          <span title={t("machine.upload")}><ArrowUp size={15} strokeWidth={2} />{rate(st.net_tx_rate)}</span>
        </div>
      </StatTile>
    </div>
  {:else if !l.checking}
    <div class="asleep">
      <Sketch kind="sleeping" label={l.pending ? t("machine.wakingUp") : t("machine.sleeping")} size={84} />
      <div class="asleep-side">
        {#if machine.mac}
          <button class="btn primary big" disabled={busy.wake || !!l.pending} onclick={wake}>
            <Zap size={16} strokeWidth={1.75} />{t("machine.wake")}
          </button>
        {:else}
          <p class="secondary">{t("machine.noWake")}</p>
        {/if}
        {#if l.error && !l.pending}<p class="problem mono">{l.error.message}</p>{/if}
      </div>
    </div>
  {/if}

  {#if online && screens}
    <div class="screens-head">
      <h3>{t("machine.screens")}</h3>
      {#if machine.vms.length}<span class="mono muted">{t("machine.vmsOf", runningVms, machine.vms.length)}</span>{/if}
    </div>
    <div class="screens">
      {#if machine.desktop_vnc_port}
        <ScreenCard {machine} vm={null} online onOpen={(kind) => onOpen(kind, null)} />
      {/if}
      {#each machine.vms as vm (vm.id)}
        <ScreenCard
          {machine}
          {vm}
          vs={vmStatus(vm)}
          {online}
          pending={vmPending[vm.id]?.label ?? null}
          busy={vmBusy(vm)}
          onOpen={(kind) => onOpen(kind, vm)}
          onAction={(a) => vmAction(vm, a)}
        />
      {/each}
    </div>
  {/if}
</section>

<style>
  .panel {
    display: grid;
    gap: 20px;
    padding: 22px 24px 24px;
    border-radius: 24px;
    background: var(--card);
    box-shadow:
      0 0 0 1px var(--line),
      var(--shadow-card);
  }
  header {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .ident {
    display: grid;
    gap: 2px;
    flex: 1;
    min-width: 0;
  }
  .title {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  h2 {
    margin: 0;
    font-family: var(--display);
    font-size: 22px;
    line-height: 28px;
    font-weight: 600;
    letter-spacing: -0.01em;
    white-space: nowrap;
  }
  .pill {
    height: 22px;
    padding: 0 9px 0 8px;
    border-radius: 11px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--text-2);
    background: var(--raised);
    white-space: nowrap;
  }
  .pill i {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--text-3);
  }
  .pill.on {
    color: var(--on);
    background: var(--on-soft);
  }
  .pill.on i {
    background: var(--on);
    animation: beat 2.4s var(--ease) infinite;
  }
  .pill.busy {
    color: var(--busy);
  }
  .pill.busy i {
    background: var(--busy);
  }
  .facts {
    margin: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 0 8px;
    color: var(--text-3);
    font-size: 12px;
    line-height: 18px;
  }
  .facts span + span::before {
    content: "·";
    margin-right: 8px;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: none;
  }
  .stats {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 12px;
  }
  .rates {
    display: grid;
    font-size: 17px;
    line-height: 22px;
    font-weight: 600;
  }
  .rates span {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    white-space: nowrap;
  }
  .rates :global(svg) {
    color: var(--text-3);
  }
  .asleep {
    display: flex;
    align-items: center;
    gap: 32px;
    padding: 12px 8px 4px;
  }
  .asleep-side {
    display: grid;
    gap: 8px;
    justify-items: start;
    min-width: 0;
  }
  .asleep p {
    margin: 0;
  }
  .btn.big {
    height: 40px;
    padding: 0 18px;
    border-radius: 12px;
  }
  .problem {
    color: var(--text-3);
    overflow-wrap: anywhere;
  }
  .screens-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    margin-bottom: -8px;
  }
  h3 {
    margin: 0;
    font-family: var(--display);
    font-size: 15px;
    font-weight: 600;
  }
  .screens {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 14px;
  }
  @keyframes beat {
    0%,
    100% {
      box-shadow: 0 0 0 0 color-mix(in srgb, var(--on) 60%, transparent);
    }
    50% {
      box-shadow: 0 0 0 4px transparent;
    }
  }
  @media (max-width: 980px) {
    .stats {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
</style>
