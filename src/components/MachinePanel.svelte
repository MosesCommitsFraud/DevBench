<script lang="ts">
  import { Power, RotateCcw, Zap, KeyRound, SquareTerminal, Cpu, MemoryStick, HardDrive, ArrowDownUp, FolderOpen, Ellipsis, EthernetPort, ArrowUpRight, Wrench, ArrowUpCircle, ScrollText } from "@lucide/svelte";
  import { api, type Machine, type SetupStep, type Vm } from "../lib/api";
  import { checkServer, pendingUpdates, servers } from "../lib/server.svelte";
  import { explain } from "../lib/errors";
  import ErrorNote from "./ErrorNote.svelte";
  import SetupDialog from "./SetupDialog.svelte";
  import { bytes, duration, rate } from "../lib/format";
  import { t } from "../lib/i18n.svelte";
  import { boost, GRACE_MS, live, poll } from "../lib/store.svelte";
  import { askHostKey, confirm, notify, run } from "../lib/ui.svelte";
  import { browse, forwardsOf, openPorts } from "../lib/ports.svelte";
  import Menu from "./Menu.svelte";
  import Sketch from "./Sketch.svelte";
  import Tile from "./Tile.svelte";
  import VmRow from "./VmRow.svelte";

  let {
    machine,
    onOpen,
    onFiles,
  }: {
    machine: Machine;
    onOpen: (kind: "screen" | "terminal", vm: Vm | null, step?: SetupStep | null) => void;
    onFiles: (vm: Vm | null) => void;
  } = $props();

  const l = $derived(live(machine.id));
  const forwards = $derived(forwardsOf(machine.id, null));
  let setupOpen = $state(false);
  const updates = $derived(pendingUpdates(machine.id));
  const missing = $derived(servers[machine.id]?.check?.parts.filter((p) => !p.ok).length ?? 0);
  const st = $derived(l.status);
  const online = $derived(!!st?.online);
  const hostKeyError = $derived(l.error && l.error.kind !== "error" ? l.error : null);
  // Polls fail, but the machine was online a moment ago: keep its figures, say it does not answer.
  const stale = $derived(online && !!l.lostAt);
  // Offline, but the error shows it is switched on (connection reset, refused, login failed ...).
  const upButLocked = $derived(!online && !!l.error && explain(l.error.message).up === true);
  const updating = $derived(online ? (st?.updating ?? null) : null);

  const ago = (ms: number) => (ms < 60_000 ? `${Math.max(1, Math.round(ms / 1000))} s` : duration(ms / 1000));
  // A tick for "values from 20 s ago" while the machine does not answer.
  let now = $state(Date.now());
  $effect(() => {
    if (!stale) return;
    const id = setInterval(() => (now = Date.now()), 1000);
    return () => clearInterval(id);
  });

  // When DevBench's update service ends, check the server again (counts, result) and say so.
  let wasUpdating = false;
  $effect(() => {
    const running = !!updating?.devbench;
    if (wasUpdating && !running && online && !stale) {
      void checkServer(machine.id).then(() => {
        const last = servers[machine.id]?.check?.last_update;
        if (last?.code === 0) notify(t("update.done", machine.name));
        else if (last) notify(t("update.failed", machine.name), "error");
      });
    }
    if (!stale) wasUpdating = running;
  });

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
    // Expected to go quiet now: no grace period of "not responding" before it counts as off.
    if (ok && (await act(action, () => api.power(machine.id, action)))) l.lostAt = Date.now() - GRACE_MS;
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

  const memUsed = $derived(st ? st.mem_total - st.mem_available : 0);
  const ratio = (used: number, total: number) => (total > 0 ? Math.min(1, used / total) : 0);
  const target = $derived(machine.ssh_user ? `${machine.ssh_user}@${machine.ssh_host}` : machine.ssh_host);
  const runningVms = $derived(st?.vms.filter((v) => v.running).length ?? 0);
  const dot = $derived(stale ? "busy" : online ? "on" : l.checking || l.pending ? "busy" : "");
  const status = $derived(
    stale
      ? `${t("machine.notResponding")} · ${t("machine.lastSeen", ago(now - l.lostAt!))}`
      : online
      ? t("machine.upFor", duration(st!.uptime_secs))
      : l.pending
        ? `${l.pending} …`
        : l.checking
          ? `${t("machine.checking")} …`
          : t("machine.offline"),
  );
  const facts = $derived(online && st ? [st.os, st.ip || st.hostname] : [target]);

  const stats = $derived(
    st
      ? [
          { icon: Cpu, label: t("machine.cpu"), value: `${Math.round(st.cpu_percent)} %`, ratio: st.cpu_percent / 100, detail: st.cpu_model },
          {
            icon: MemoryStick,
            label: t("machine.memory"),
            value: bytes(memUsed),
            ratio: ratio(memUsed, st.mem_total),
            detail: t("machine.ofTotal", bytes(st.mem_total)),
          },
          {
            icon: HardDrive,
            label: t("machine.disk"),
            value: bytes(st.disk_used),
            ratio: ratio(st.disk_used, st.disk_total),
            detail: t("machine.freeOf", bytes(Math.max(0, st.disk_total - st.disk_used)), bytes(st.disk_total)),
          },
        ]
      : [],
  );
</script>

<section class="machine">
  <header>
    <Tile name={machine.name} color={machine.color} size={20} />
    <div class="ident">
      <h2>{machine.name}</h2>
      <div class="sub">
        <span class="state">{#if dot}<span class="dot {dot}"></span>{/if}{status}</span>
        {#each facts as f (f)}<span class="fact mono">{f}</span>{/each}
        {#each forwards as f (f.id)}
          <button class="chip mono" class:broken={!!f.error} title={f.error ?? t("ports.openAt", f.local_port)} onclick={() => browse(f)}>
            :{f.remote_port}{#if f.local_port !== f.remote_port}<span class="arrow">→ {f.local_port}</span>{/if}<ArrowUpRight size={11} strokeWidth={2} />
          </button>
        {/each}
      </div>
    </div>
    <div class="actions">
      {#if online}
        <button class="btn" onclick={() => onOpen("terminal", null)}><SquareTerminal size={14} strokeWidth={1.75} />{t("machine.terminal")}</button>
        <button class="btn" onclick={() => onFiles(null)}><FolderOpen size={14} strokeWidth={1.75} />{t("nav.files")}</button>
        <button class="btn" onclick={() => openPorts(machine.id, null)}><EthernetPort size={14} strokeWidth={1.75} />{t("ports.title")}</button>
        {#if updates && !updating}
          <button class="btn updates" title={t("setup.title")} onclick={() => (setupOpen = true)}>
            <ArrowUpCircle size={14} strokeWidth={1.75} />{t("setup.updatesShort", updates)}
          </button>
        {/if}
        <Menu label={t("machine.power")}>
          {#snippet trigger()}<Ellipsis size={15} strokeWidth={1.5} />{/snippet}
          <button onclick={() => (setupOpen = true)}>
            <Wrench size={15} strokeWidth={1.5} />{t("setup.title")}{#if missing}<span class="badge">{missing}</span>{/if}
          </button>
          <button disabled={busy.reboot} onclick={() => power("reboot")}><RotateCcw size={15} strokeWidth={1.5} />{t("machine.reboot")}</button>
          <button class="danger" disabled={busy.poweroff} onclick={() => power("poweroff")}><Power size={15} strokeWidth={1.5} />{t("machine.poweroff")}</button>
        </Menu>
      {:else if !l.checking}
        {#if hostKeyError}
          <button class="btn" onclick={checkHostKey}><KeyRound size={14} strokeWidth={1.75} />{t("machine.trust")}</button>
        {/if}
        {#if machine.mac}
          <button class="btn primary" disabled={busy.wake || !!l.pending} onclick={wake}><Zap size={14} strokeWidth={1.75} />{t("machine.wake")}</button>
        {/if}
      {/if}
    </div>
  </header>

  {#if stale && l.error}
    <div class="rows band">
      <span class="dot busy"></span>
      <ErrorNote message={l.error.message} />
    </div>
  {/if}

  {#if updating}
    <div class="rows band">
      <span class="dot busy"></span>
      <div class="band-text">
        <p>{updating.devbench ? t("update.running") : t("update.system", updating.process)} …</p>
        {#if updating.line}<p class="mono line" title={updating.line}>{updating.line}</p>{/if}
      </div>
      {#if updating.devbench}
        <button class="btn" onclick={() => onOpen("terminal", null, "update-log")}><ScrollText size={14} strokeWidth={1.75} />{t("update.follow")}</button>
      {/if}
    </div>
  {/if}

  {#if online && st}
    <div class="rows stats" class:stale>
      {#each stats as s (s.label)}
        {@const Icon = s.icon}
        <div class="stat" class:hot={s.ratio > 0.9}>
          <div class="eyebrow"><Icon size={12} strokeWidth={2} />{s.label}</div>
          <div class="value mono">{s.value}</div>
          <div class="meter"><i style:transform="scaleX({Math.max(0, Math.min(1, s.ratio))})"></i></div>
          <div class="detail">{s.detail}</div>
        </div>
      {/each}
      <div class="stat">
        <div class="eyebrow"><ArrowDownUp size={12} strokeWidth={2} />{t("machine.network")}</div>
        <div class="value mono net">
          <span title={t("machine.download")}>↓ {rate(st.net_rx_rate)}</span>
          <span title={t("machine.upload")}>↑ {rate(st.net_tx_rate)}</span>
        </div>
        <div class="detail">{st.iface}</div>
      </div>
    </div>
  {:else if !l.checking}
    <div class="rows asleep">
      {#if upButLocked && !l.pending}
        <Sketch kind="lost" size={46} />
        <div class="asleep-text">
          <p>{t("machine.upNoAccess")}</p>
          <ErrorNote message={l.error!.message} />
        </div>
      {:else}
        <Sketch kind="sleeping" size={46} />
        <div class="asleep-text">
          <p>{l.pending ? t("machine.wakingUp") : machine.mac ? t("machine.asleepWake") : t("machine.noWake")}</p>
          <!-- "does not answer" is what asleep looks like; other reasons (no network path, unknown name) are worth a line -->
          {#if l.error && !l.pending && explain(l.error.message).title !== t("err.timeout")}<ErrorNote message={l.error.message} />{/if}
        </div>
      {/if}
    </div>
  {/if}

  {#if online && (machine.vms.length || machine.desktop_vnc_port)}
    <div class="section-head">
      <span class="eyebrow">{t("section.vms")}</span>
      {#if machine.vms.length}<span class="eyebrow count">{t("machine.vmsOf", runningVms, machine.vms.length)}</span>{/if}
    </div>
    <div class="rows">
      {#if machine.desktop_vnc_port}
        <VmRow {machine} vm={null} online onOpen={(kind) => onOpen(kind, null)} />
      {/if}
      {#each machine.vms as vm (vm.id)}
        <VmRow
          {machine}
          {vm}
          vs={vmStatus(vm)}
          {online}
          pending={vmPending[vm.id]?.label ?? null}
          busy={{ start: !!busy[`${vm.id}:start`], shutdown: !!busy[`${vm.id}:shutdown`], autostart: !!busy[`${vm.id}:autostart`] }}
          onOpen={(kind) => onOpen(kind, vm)}
          onFiles={() => onFiles(vm)}
          onAction={(a) => vmAction(vm, a)}
          onAutostart={(on) => act(`${vm.id}:autostart`, () => api.vmAutostart(machine.id, vm.id, on), t(on ? "vm.autostart.enabled" : "vm.autostart.disabled", vm.name))}
        />
      {/each}
    </div>
  {/if}
</section>

{#if setupOpen}
  <SetupDialog
    {machine}
    onClose={() => (setupOpen = false)}
    onRun={(step) => {
      setupOpen = false;
      onOpen("terminal", null, step);
    }}
  />
{/if}

<style>
  .updates {
    color: var(--busy);
  }
  .badge {
    margin-left: auto;
    min-width: 18px;
    height: 18px;
    padding: 0 5px;
    border-radius: 9px;
    background: var(--raised);
    color: var(--text-2);
    font-size: 11px;
    line-height: 18px;
    text-align: center;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 20px;
    padding: 0 6px 0 7px;
    border: 1px solid var(--line);
    border-radius: 999px;
    background: var(--sunken);
    color: var(--text-2);
    font-size: 11px;
    cursor: pointer;
  }
  .chip:hover {
    color: var(--text);
    border-color: var(--line-strong);
  }
  .chip.broken {
    color: var(--danger);
  }
  .chip .arrow {
    color: var(--text-3);
  }
  .machine {
    display: grid;
    gap: 10px;
  }
  header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 2px 2px;
  }
  .ident {
    flex: 1;
    min-width: 0;
  }
  h2 {
    margin: 0;
    font-size: 15px;
    line-height: 22px;
    font-weight: 600;
  }
  .sub {
    display: flex;
    align-items: center;
    gap: 8px;
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
    gap: 6px;
    flex: none;
  }
  .stats {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
  }
  .stats > * + * {
    border-top: none;
    border-left: 1px solid var(--line);
  }
  .stat {
    min-width: 0;
    padding: 14px 18px 14px;
  }
  .eyebrow {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .eyebrow :global(svg) {
    opacity: 0.8;
  }
  .value {
    margin-top: 8px;
    font-size: 20px;
    line-height: 26px;
    font-weight: 600;
    letter-spacing: -0.03em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .value.net {
    display: grid;
    font-size: 14px;
    line-height: 19px;
    letter-spacing: -0.01em;
    margin-top: 6px;
  }
  .meter {
    margin-top: 8px;
    height: 3px;
    border-radius: 2px;
    background: var(--raised);
    overflow: hidden;
  }
  .meter i {
    display: block;
    height: 100%;
    background: var(--text-2);
    transform-origin: left;
    transition: transform 600ms var(--ease);
  }
  .hot .meter i {
    background: var(--busy);
  }
  .hot .value {
    color: var(--busy);
  }
  .detail {
    margin-top: 6px;
    font-size: 11px;
    line-height: 14px;
    color: var(--text-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .asleep {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 12px 16px;
  }
  /* Picture and text sit side by side, not as rows: no divider between them */
  .asleep > :global(*) {
    border-top: none;
  }
  .asleep p {
    margin: 0;
    color: var(--text-2);
  }
  .asleep-text {
    display: grid;
    gap: 6px;
    min-width: 0;
  }
  .stats.stale {
    opacity: 0.55;
  }
  .band {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 10px 10px 16px;
  }
  .band > .dot {
    width: 7px;
    height: 7px;
    flex: none;
    align-self: flex-start;
    margin-top: 7px;
  }
  .band > :global(.note),
  .band-text {
    flex: 1;
    min-width: 0;
  }
  .band-text p {
    margin: 0;
  }
  .band-text .line {
    margin-top: 2px;
    font-size: 11px;
    color: var(--text-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .section-head {
    display: flex;
    justify-content: space-between;
    padding: 10px 2px 0;
  }
  .count {
    letter-spacing: 0.04em;
  }
  @media (max-width: 900px) {
    .stats {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
    .stats > :nth-child(3) {
      border-left: none;
    }
    .stats > :nth-child(n + 3) {
      border-top: 1px solid var(--line);
    }
  }
</style>
