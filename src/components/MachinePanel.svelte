<script lang="ts">
  import { Power, RotateCcw, Zap, Monitor, Play, Square, Ellipsis, KeyRound } from "@lucide/svelte";
  import { api, type Machine, type Vm } from "../lib/api";
  import { bytes, duration } from "../lib/format";
  import { t, vmStateLabel } from "../lib/i18n.svelte";
  import { boost, live, poll } from "../lib/store.svelte";
  import { askHostKey, confirm, notify, run } from "../lib/ui.svelte";
  import Menu from "./Menu.svelte";
  import Meter from "./Meter.svelte";

  let { machine, onOpenScreen }: { machine: Machine; onOpenScreen: (vm: Vm) => void } = $props();

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
    l.pending = t("machine.wake");
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

  function memUsed() {
    return st ? st.mem_total - st.mem_available : 0;
  }

  function ratio(used: number, total: number) {
    return total > 0 ? Math.min(1, used / total) : 0;
  }

  const target = $derived(machine.ssh_user ? `${machine.ssh_user}@${machine.ssh_host}` : machine.ssh_host);
</script>

<section class="panel">
  <header>
    <div class="title">
      <span class="led" class:on={online} class:busy={!online && (l.checking || !!l.pending)}></span>
      <h2>{machine.name}</h2>
      <span class="mono muted">{target}</span>
    </div>
    <div class="actions">
      {#if online}
        <span class="meta mono muted">{t("machine.up", duration(st!.uptime_secs))}</span>
        <button class="btn ghost" disabled={busy.reboot} onclick={() => power("reboot")}>
          <RotateCcw size={16} strokeWidth={1.5} />{t("machine.reboot")}
        </button>
        <button class="btn" disabled={busy.poweroff} onclick={() => power("poweroff")}>
          <Power size={16} strokeWidth={1.5} />{t("machine.poweroff")}
        </button>
      {:else if !l.checking}
        {#if hostKeyError}
          <button class="btn" onclick={checkHostKey}><KeyRound size={16} strokeWidth={1.5} />{t("machine.trust")}</button>
        {/if}
        {#if machine.mac}
          <button class="btn primary" disabled={busy.wake} onclick={wake}>
            <Zap size={16} strokeWidth={1.5} />{t("machine.wake")}
          </button>
        {/if}
      {/if}
    </div>
  </header>

  {#if online && st}
    <div class="meters">
      <Meter
        label={t("machine.cpu")}
        value={st.cpu_percent / 100}
        detail={`${t("machine.cores", st.cpus)} · ${t("machine.load", st.load.map((x) => x.toFixed(2)).join(" "))}`}
      />
      <Meter label={t("machine.memory")} value={ratio(memUsed(), st.mem_total)} detail={`${bytes(memUsed())} / ${bytes(st.mem_total)}`} />
      <Meter label={t("machine.disk")} value={ratio(st.disk_used, st.disk_total)} detail={`${bytes(st.disk_used)} / ${bytes(st.disk_total)}`} />
    </div>
  {:else if !l.checking}
    <p class="offline">
      <span class="secondary">{l.pending ? `${l.pending} …` : t("machine.offline")}</span>
      {#if l.error && !l.pending}<span class="mono muted">{l.error.message}</span>{/if}
    </p>
  {/if}

  {#if machine.vms.length}
    <ul class="vms">
      {#each machine.vms as vm (vm.id)}
        {@const vs = vmStatus(vm)}
        {@const running = !!vs?.running}
        {@const pending = vmPending[vm.id]}
        <li>
          <span class="led" class:on={running && !pending} class:busy={!!pending || vs?.state === "in shutdown" || vs?.state === "paused"} class:error={vs?.state === "crashed"}></span>
          <span class="vm-name">{vm.name}</span>
          <span class="mono muted vm-meta">{vm.libvirt_name}</span>
          <span class="vm-state secondary" title={vs?.error ?? ""}>{pending ? `${pending.label} …` : online && vs ? vmStateLabel(vs.state) : "–"}</span>
          <div class="vm-actions">
            {#if online && vs}
              {#if running}
                {#if vm.vnc_mode !== "none"}
                  <button class="btn" onclick={() => onOpenScreen(vm)}><Monitor size={16} strokeWidth={1.5} />{t("vm.screen")}</button>
                {/if}
                <button class="btn ghost" disabled={busy[`${vm.id}:shutdown`]} onclick={() => vmAction(vm, "shutdown")}>
                  <Square size={16} strokeWidth={1.5} />{t("vm.shutdown")}
                </button>
                <Menu label={t("vm.more")}>
                  {#snippet trigger()}<Ellipsis size={16} strokeWidth={1.5} />{/snippet}
                  <button onclick={() => vmAction(vm, "reboot")}>{t("vm.reboot")}</button>
                  <button class="danger" onclick={() => vmAction(vm, "destroy")}>{t("vm.destroy")}</button>
                </Menu>
              {:else}
                <button class="btn" disabled={busy[`${vm.id}:start`]} onclick={() => vmAction(vm, "start")}>
                  <Play size={16} strokeWidth={1.5} />{t("vm.start")}
                </button>
              {/if}
            {/if}
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .panel {
    background: var(--card);
    border: 1px solid var(--line);
    border-radius: var(--r-panel);
    padding: 20px 24px;
    display: grid;
    gap: 20px;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    min-height: 32px;
  }
  .title {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .title .mono {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  h2 {
    margin: 0;
    font-size: 16px;
    line-height: 24px;
    font-weight: 500;
    white-space: nowrap;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .meta {
    margin-right: 8px;
    white-space: nowrap;
  }
  .meters {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 24px;
  }
  .offline {
    margin: 0;
    display: grid;
    gap: 4px;
  }
  .vms {
    list-style: none;
    margin: 0;
    padding: 4px 0 0;
    border-top: 1px solid var(--line);
    display: grid;
  }
  .vms li {
    display: grid;
    grid-template-columns: 8px minmax(0, auto) minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 12px;
    min-height: 48px;
    border-bottom: 1px solid var(--line);
  }
  .vms li:last-child {
    border-bottom: none;
  }
  .vm-name {
    font-weight: 500;
    white-space: nowrap;
  }
  .vm-meta {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .vm-state {
    min-width: 96px;
    text-align: right;
  }
  .vm-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    min-width: 200px;
  }
  @media (max-width: 760px) {
    .meters {
      grid-template-columns: 1fr;
      gap: 16px;
    }
    .vm-meta {
      display: none;
    }
  }
</style>
