<script lang="ts">
  import { Power, RotateCcw, Zap, Monitor, Play, Square, Ellipsis, KeyRound, SquareTerminal, ArrowDown, ArrowUp } from "@lucide/svelte";
  import { api, type Machine, type Vm } from "../lib/api";
  import { bytes, duration, gb, rate } from "../lib/format";
  import { t, vmStateLabel } from "../lib/i18n.svelte";
  import { boost, live, poll } from "../lib/store.svelte";
  import { askHostKey, confirm, notify, run } from "../lib/ui.svelte";
  import Menu from "./Menu.svelte";
  import Meter from "./Meter.svelte";
  import CpuChart from "./CpuChart.svelte";

  let { machine, onOpen }: { machine: Machine; onOpen: (kind: "screen" | "terminal", vm: Vm | null) => void } = $props();

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

  interface Pending {
    label: string;
    wasRunning: boolean;
    action: string;
    since: number;
  }
  let vmPending = $state<Record<string, Pending>>({});

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

  const memUsed = $derived(st ? st.mem_total - st.mem_available : 0);
  const ratio = (used: number, total: number) => (total > 0 ? Math.min(1, used / total) : 0);

  const facts = $derived(
    st ? [st.os, st.cpu_model, st.ip || st.hostname].filter(Boolean) : [],
  );
  const target = $derived(machine.ssh_user ? `${machine.ssh_user}@${machine.ssh_host}` : machine.ssh_host);
  const runningVms = $derived(st?.vms.filter((v) => v.running).length ?? 0);
</script>

<section class="panel card" class:offline={!online && !l.checking}>
  <header>
    <div class="ident">
      <div class="title">
        <span class="led" class:on={online} class:busy={!online && (l.checking || !!l.pending)}></span>
        <h2>{machine.name}</h2>
        {#if online}
          <span class="state on">{t("machine.up", duration(st!.uptime_secs))}</span>
        {:else if l.pending}
          <span class="state busy">{l.pending} …</span>
        {:else if !l.checking}
          <span class="state">{t("machine.offline")}</span>
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
        {#if machine.desktop_vnc_port}
          <button class="btn" onclick={() => onOpen("screen", null)}>
            <Monitor size={16} strokeWidth={1.5} />{t("machine.desktop")}
          </button>
        {/if}
        <Menu label={t("machine.power")}>
          {#snippet trigger()}<Power size={16} strokeWidth={1.5} />{/snippet}
          <button disabled={busy.reboot} onclick={() => power("reboot")}><RotateCcw size={16} strokeWidth={1.5} />{t("machine.reboot")}</button>
          <button class="danger" disabled={busy.poweroff} onclick={() => power("poweroff")}><Power size={16} strokeWidth={1.5} />{t("machine.poweroff")}</button>
        </Menu>
      {:else if !l.checking}
        {#if hostKeyError}
          <button class="btn" onclick={checkHostKey}><KeyRound size={16} strokeWidth={1.5} />{t("machine.trust")}</button>
        {/if}
        {#if machine.mac}
          <button class="btn primary" disabled={busy.wake || !!l.pending} onclick={wake}>
            <Zap size={16} strokeWidth={1.5} />{t("machine.wake")}
          </button>
        {/if}
      {/if}
    </div>
  </header>

  {#if online && st}
    <div class="body">
      <div class="cpu">
        <div class="cpu-head">
          <span class="secondary">{t("machine.cpu")}</span>
          <span class="figure mono">{Math.round(st.cpu_percent)}<small>%</small></span>
        </div>
        <div class="chart"><CpuChart values={l.cpu} /></div>
        <span class="mono muted caption">{t("machine.cpuWindow")}</span>
      </div>
      <div class="side">
        <Meter label={t("machine.memory")} value={ratio(memUsed, st.mem_total)} detail={`${bytes(memUsed)} / ${bytes(st.mem_total)}`} />
        <Meter label={t("machine.disk")} value={ratio(st.disk_used, st.disk_total)} detail={`${bytes(st.disk_used)} / ${bytes(st.disk_total)}`} />
        <div class="net">
          <span class="secondary">{t("machine.network")}</span>
          <span class="rates mono">
            <span title={t("machine.download")}><ArrowDown size={14} strokeWidth={1.5} />{rate(st.net_rx_rate)}</span>
            <span title={t("machine.upload")}><ArrowUp size={14} strokeWidth={1.5} />{rate(st.net_tx_rate)}</span>
          </span>
        </div>
      </div>
    </div>
  {:else if !l.checking && l.error && !l.pending}
    <p class="problem mono muted">{l.error.message}</p>
  {/if}

  {#if machine.vms.length}
    <div class="vms">
      <div class="vms-head">
        <span class="secondary">{t("section.vms")}</span>
        {#if online}<span class="mono muted">{t("machine.vmsOf", runningVms, machine.vms.length)}</span>{/if}
      </div>
      <ul>
        {#each machine.vms as vm (vm.id)}
          {@const vs = vmStatus(vm)}
          {@const running = !!vs?.running}
          {@const pending = vmPending[vm.id]}
          <li class:running>
            <span class="led" class:on={running && !pending} class:busy={!!pending || vs?.state === "in shutdown" || vs?.state === "paused"} class:error={vs?.state === "crashed"}></span>
            <div class="vm-id">
              <span class="vm-name">{vm.name}</span>
              <span class="vm-specs mono muted">
                {#if vs?.vcpus}{t("vm.vcpus", vs.vcpus)} · {gb(vs.memory)}{:else}{vm.libvirt_name}{/if}{#if vs?.ip} · {vs.ip}{/if}
              </span>
            </div>
            <div class="vm-load">
              {#if running && vs?.cpu_percent != null && !pending}
                <div class="mini"><i style:transform="scaleX({vs.cpu_percent / 100})"></i></div>
                <span class="mono">{Math.round(vs.cpu_percent)}<small>%</small></span>
              {/if}
            </div>
            <span class="vm-state" class:on={running && !pending} title={vs?.error ?? ""}>
              {pending ? `${pending.label} …` : online && vs ? vmStateLabel(vs.state) : "–"}
            </span>
            <div class="vm-actions">
              {#if online && vs}
                {#if running}
                  {#if vm.vnc_mode !== "none"}
                    <button class="btn" onclick={() => onOpen("screen", vm)}><Monitor size={16} strokeWidth={1.5} />{t("vm.screen")}</button>
                  {/if}
                  {#if vm.guest_ssh_user}
                    <button class="btn icon" title={t("machine.terminal")} aria-label={t("machine.terminal")} onclick={() => onOpen("terminal", vm)}>
                      <SquareTerminal size={16} strokeWidth={1.5} />
                    </button>
                  {/if}
                  <Menu label={t("vm.more")}>
                    {#snippet trigger()}<Ellipsis size={16} strokeWidth={1.5} />{/snippet}
                    <button disabled={busy[`${vm.id}:shutdown`]} onclick={() => vmAction(vm, "shutdown")}><Square size={16} strokeWidth={1.5} />{t("vm.shutdown")}</button>
                    <button onclick={() => vmAction(vm, "reboot")}><RotateCcw size={16} strokeWidth={1.5} />{t("vm.reboot")}</button>
                    <button class="danger" onclick={() => vmAction(vm, "destroy")}><Power size={16} strokeWidth={1.5} />{t("vm.destroy")}</button>
                  </Menu>
                {:else}
                  <button class="btn" disabled={busy[`${vm.id}:start`] || !!pending} onclick={() => vmAction(vm, "start")}>
                    <Play size={16} strokeWidth={1.5} />{t("vm.start")}
                  </button>
                {/if}
              {/if}
            </div>
          </li>
        {/each}
      </ul>
    </div>
  {/if}
</section>

<style>
  .panel {
    padding: 20px 24px 8px;
    display: grid;
    gap: 20px;
  }
  .panel.offline {
    padding-bottom: 8px;
  }
  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
  }
  .ident {
    display: grid;
    gap: 4px;
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
    font-size: 18px;
    line-height: 28px;
    font-weight: 500;
    white-space: nowrap;
  }
  .state {
    margin-left: 4px;
    height: 22px;
    padding: 0 8px;
    border-radius: 11px;
    display: inline-flex;
    align-items: center;
    font-size: 12px;
    color: var(--text-2);
    background: var(--raised);
    white-space: nowrap;
  }
  .state.on {
    color: var(--on);
    background: var(--on-soft);
  }
  .state.busy {
    color: var(--busy);
  }
  .facts {
    margin: 0 0 0 18px;
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
    color: var(--text-3);
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: none;
  }
  .body {
    display: grid;
    grid-template-columns: minmax(0, 1.6fr) minmax(240px, 1fr);
    gap: 32px;
    padding: 4px 0;
  }
  .cpu {
    display: grid;
    grid-template-rows: auto 120px auto;
    gap: 8px;
    min-width: 0;
  }
  .cpu-head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }
  .figure {
    font-size: 28px;
    line-height: 32px;
    font-weight: 500;
    font-variant-numeric: tabular-nums;
  }
  .figure small {
    margin-left: 2px;
    font-size: 14px;
    color: var(--text-3);
    font-weight: 400;
  }
  .chart {
    position: relative;
    margin-right: 4px;
  }
  .caption {
    font-size: 11px;
  }
  .side {
    display: grid;
    align-content: center;
    gap: 16px;
    padding-left: 32px;
    border-left: 1px solid var(--line);
  }
  .net {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 12px;
  }
  .rates {
    display: flex;
    gap: 16px;
    font-size: 13px;
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
  .problem {
    margin: -8px 0 12px 18px;
    overflow-wrap: anywhere;
  }
  .vms {
    margin: 0 -24px;
    border-top: 1px solid var(--line);
  }
  .vms-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    height: 40px;
    padding: 0 24px;
    font-size: 13px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0 12px 4px;
    display: grid;
  }
  li {
    display: grid;
    grid-template-columns: 8px minmax(0, 1fr) 96px 120px auto;
    align-items: center;
    gap: 16px;
    min-height: 56px;
    padding: 0 12px;
    border-radius: var(--r-card);
    transition: background-color var(--fast) var(--ease);
  }
  li:hover {
    background: var(--sunken);
  }
  .vm-id {
    display: grid;
    min-width: 0;
  }
  .vm-name {
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .vm-specs {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .vm-load {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
  }
  .vm-load small {
    color: var(--text-3);
  }
  .mini {
    flex: 1;
    height: 4px;
    border-radius: 2px;
    background: var(--raised);
    overflow: hidden;
  }
  .mini i {
    display: block;
    height: 100%;
    background: var(--text-3);
    transform-origin: left;
    transition: transform 600ms var(--ease);
  }
  .vm-state {
    color: var(--text-2);
    text-align: right;
    white-space: nowrap;
  }
  .vm-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    min-width: 180px;
  }
  @media (max-width: 900px) {
    .body {
      grid-template-columns: 1fr;
      gap: 20px;
    }
    .side {
      padding-left: 0;
      border-left: none;
    }
    li {
      grid-template-columns: 8px minmax(0, 1fr) auto auto;
    }
    .vm-load {
      display: none;
    }
  }
</style>
