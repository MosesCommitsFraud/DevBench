<script lang="ts">
  import { onMount } from "svelte";
  import { Server, Activity, Settings, FolderOpen, Sun, Moon, Search, SquareTerminal, Monitor, Play, Zap, Palette } from "@lucide/svelte";
  import { api, type Machine, type Vm } from "./lib/api";
  import { t } from "./lib/i18n.svelte";
  import { live, loadConfig, startPolling, store, wakeAll } from "./lib/store.svelte";
  import { notify, run } from "./lib/ui.svelte";
  import { theme, toggleTheme } from "./lib/theme.svelte";
  import { initUpdates, updates } from "./lib/updater.svelte";
  import { wantPreviews } from "./lib/previews.svelte";
  import MachinePanel from "./components/MachinePanel.svelte";
  import SettingsView from "./components/SettingsView.svelte";
  import ActivityView from "./components/ActivityView.svelte";
  import FilesView from "./components/FilesView.svelte";
  import VncView from "./components/VncView.svelte";
  import TermView from "./components/TermView.svelte";
  import Overlays from "./components/Overlays.svelte";
  import Titlebar from "./components/Titlebar.svelte";
  import CommandPalette, { type Command } from "./components/CommandPalette.svelte";
  import Sketch from "./components/Sketch.svelte";
  import Tile from "./components/Tile.svelte";

  type View = "machines" | "files" | "activity" | "settings";
  let view = $state<View>("machines");
  /** A screen (VNC) or terminal covering the window; vm null = the machine itself. */
  let session = $state<{ kind: "screen" | "terminal"; machine: Machine; vm: Vm | null } | null>(null);
  let ready = $state(false);
  let palette = $state(false);

  onMount(async () => {
    const err = await api.startupError();
    if (err) notify(`${t("error.config")}: ${err}`, "error");
    await loadConfig();
    ready = true;
    startPolling();
    void initUpdates();
  });

  // Screen pictures only while the dashboard is in front
  $effect(() => {
    wantPreviews(ready && view === "machines" && !session);
  });

  function open(kind: "screen" | "terminal", machine: Machine, vm: Vm | null) {
    session = { kind, machine, vm };
    store.vncOpen = kind === "screen";
  }
  function closeSession() {
    session = null;
    store.vncOpen = false;
    wakeAll();
  }

  function openFiles(machine: Machine | null, vm: Vm | null) {
    store.filesSystem = !machine ? "local" : vm ? `v:${machine.id}:${vm.id}` : `m:${machine.id}`;
    view = "files";
  }

  function scrollTo(id: string) {
    view = "machines";
    requestAnimationFrame(() => document.getElementById(`m-${id}`)?.scrollIntoView({ behavior: "smooth", block: "start" }));
  }

  const sessionTitle = $derived(session ? [session.vm?.name, session.machine.name].filter(Boolean).join(" · ") : "");

  // Everything the palette offers, built from the machines and their current state
  const commands = $derived.by<Command[]>(() => {
    const out: Command[] = [];
    const go = t("palette.go");
    out.push(
      { id: "v:machines", group: go, label: t("nav.machines"), icon: Server, run: () => (view = "machines") },
      { id: "v:files", group: go, label: t("nav.files"), icon: FolderOpen, run: () => (view = "files") },
      { id: "v:activity", group: go, label: t("nav.activity"), icon: Activity, run: () => (view = "activity") },
      { id: "v:settings", group: go, label: t("nav.settings"), icon: Settings, run: () => (view = "settings") },
      { id: "theme", group: go, label: t("theme.toggle"), icon: Palette, run: toggleTheme },
    );
    for (const m of store.config.machines) {
      const st = live(m.id).status;
      const tile = { name: m.name, color: m.color };
      if (st?.online) {
        out.push({ id: `t:${m.id}`, group: m.name, label: t("palette.terminal"), hint: m.name, tile, run: () => open("terminal", m, null) });
        out.push({ id: `f:${m.id}`, group: m.name, label: t("palette.files"), hint: m.name, tile, run: () => openFiles(m, null) });
        if (m.desktop_vnc_port) out.push({ id: `d:${m.id}`, group: m.name, label: t("palette.desktop"), hint: m.name, icon: Monitor, run: () => open("screen", m, null) });
      } else if (m.mac) {
        out.push({
          id: `w:${m.id}`,
          group: m.name,
          label: t("palette.wake"),
          hint: m.name,
          icon: Zap,
          run: () => void run(() => api.wake(m.id)).then(() => notify(t("machine.woke", m.name)), () => {}),
        });
      }
      for (const v of m.vms) {
        const vs = st?.vms.find((x) => x.id === v.id);
        const vt = { name: v.name, color: v.color };
        if (vs?.running) {
          if (v.vnc_mode !== "none") out.push({ id: `s:${v.id}`, group: m.name, label: t("palette.screen"), hint: v.name, tile: vt, run: () => open("screen", m, v) });
          if (v.guest_ssh_user) {
            out.push({ id: `vt:${v.id}`, group: m.name, label: t("palette.terminal"), hint: v.name, icon: SquareTerminal, run: () => open("terminal", m, v) });
            out.push({ id: `vf:${v.id}`, group: m.name, label: t("palette.files"), hint: v.name, icon: FolderOpen, run: () => openFiles(m, v) });
          }
        } else if (st?.online && vs) {
          out.push({
            id: `vs:${v.id}`,
            group: m.name,
            label: t("palette.start"),
            hint: v.name,
            icon: Play,
            run: () => void run(() => api.vmAction(m.id, v.id, "start")).catch(() => {}),
          });
        }
      }
    }
    return out;
  });
</script>

<div class="window">
  <Titlebar title={sessionTitle} />
  <div class="shell">
    <aside>
      <button class="search" onclick={() => (palette = true)}>
        <Search size={15} strokeWidth={1.75} />
        <span>{t("palette.short")}</span>
        <span class="keycap">Ctrl K</span>
      </button>

      <nav>
        <button class="nav" class:active={view === "machines"} onclick={() => (view = "machines")}>
          <Server size={16} strokeWidth={1.5} />{t("nav.machines")}
        </button>
        <div class="tree">
          {#each store.config.machines as m (m.id)}
            {@const l = live(m.id)}
            <button class="node" onclick={() => scrollTo(m.id)}>
              <Tile name={m.name} color={m.color} size={22} status={l.status?.online ? "on" : l.checking || l.pending ? "busy" : "off"} />
              <span class="label">{m.name}</span>
            </button>
            {#each m.vms as v (v.id)}
              {@const running = !!l.status?.vms.find((x) => x.id === v.id)?.running}
              <button class="node vm" class:dim={!running} onclick={() => (running && v.vnc_mode !== "none" ? open("screen", m, v) : scrollTo(m.id))}>
                <Tile name={v.name} color={v.color} size={18} status={running ? "on" : null} />
                <span class="label">{v.name}</span>
              </button>
            {/each}
          {/each}
        </div>

        <button class="nav" class:active={view === "files"} onclick={() => (view = "files")}>
          <FolderOpen size={16} strokeWidth={1.5} />{t("nav.files")}
        </button>
        <button class="nav" class:active={view === "activity"} onclick={() => (view = "activity")}>
          <Activity size={16} strokeWidth={1.5} />{t("nav.activity")}
          {#if store.agents.length}<span class="count live">{store.agents.length}</span>{/if}
        </button>
      </nav>

      <div class="foot">
        <button class="nav" class:active={view === "settings"} onclick={() => (view = "settings")}>
          <Settings size={16} strokeWidth={1.5} />{t("nav.settings")}
          {#if updates.status === "available"}<span class="count update">{t("update.badge")}</span>{/if}
        </button>
        <button class="nav" onclick={toggleTheme} title={t("theme.toggle")}>
          {#if theme() === "dark"}<Sun size={16} strokeWidth={1.5} />{t("theme.light")}{:else}<Moon size={16} strokeWidth={1.5} />{t("theme.dark")}{/if}
        </button>
      </div>
    </aside>

    <main class:fill={view === "files"}>
      {#if !ready}
        <div></div>
      {:else if view === "machines"}
        {#if store.config.machines.length === 0}
          <div class="empty">
            <Sketch kind="rack" label={t("empty.sketch")} size={96} />
            <h1>{t("empty.title")}</h1>
            <p class="secondary">{t("empty.body")}</p>
            <button class="btn primary" onclick={() => (view = "settings")}>{t("empty.action")}</button>
          </div>
        {:else}
          <div class="machines">
            {#each store.config.machines as m (m.id)}
              <div id="m-{m.id}">
                <MachinePanel machine={m} onOpen={(kind, vm) => open(kind, m, vm)} onFiles={(vm) => openFiles(m, vm)} />
              </div>
            {/each}
          </div>
        {/if}
      {:else if view === "files"}
        <FilesView />
      {:else if view === "activity"}
        <ActivityView />
      {:else}
        <SettingsView />
      {/if}
    </main>
  </div>
</div>

{#if session?.kind === "screen"}
  <VncView machine={session.machine} vm={session.vm} onClose={closeSession} />
{:else if session?.kind === "terminal"}
  <TermView machine={session.machine} vm={session.vm} onClose={closeSession} />
{/if}

<CommandPalette bind:open={palette} {commands} />
<Overlays />

<style>
  .window {
    display: flex;
    flex-direction: column;
    height: 100%;
  }
  .shell {
    display: grid;
    grid-template-columns: 248px minmax(0, 1fr);
    flex: 1;
    min-height: 0;
  }
  aside {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 12px;
    background: var(--bg-deep);
    border-right: 1px solid var(--line);
    min-height: 0;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 34px;
    padding: 0 6px 0 10px;
    border-radius: 10px;
    border: 1px solid var(--line);
    background: var(--card);
    color: var(--text-3);
    cursor: pointer;
    transition: border-color var(--fast) var(--ease);
  }
  .search:hover {
    border-color: var(--line-strong);
  }
  .search span:first-of-type {
    flex: 1;
    text-align: left;
  }
  nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }
  .foot {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .nav {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 32px;
    padding: 0 8px;
    border: none;
    border-radius: var(--r-control);
    background: transparent;
    color: var(--text-2);
    text-align: left;
    cursor: pointer;
    flex: none;
    transition:
      background-color var(--fast) var(--ease),
      color var(--fast) var(--ease);
  }
  .nav:hover {
    background: var(--raised);
    color: var(--text);
  }
  .nav.active {
    background: var(--card);
    color: var(--text);
    box-shadow:
      0 0 0 1px var(--line),
      var(--highlight);
  }
  .nav :global(svg) {
    color: var(--text-3);
    flex: none;
  }
  .nav.active :global(svg) {
    color: var(--accent-text);
  }
  .tree {
    display: flex;
    flex-direction: column;
    gap: 1px;
    margin: 2px 0 14px;
  }
  .node {
    --dot-ring: var(--bg-deep);
    display: flex;
    align-items: center;
    gap: 10px;
    height: 32px;
    padding: 0 8px 0 5px;
    border: none;
    border-radius: var(--r-control);
    background: transparent;
    color: var(--text);
    text-align: left;
    cursor: pointer;
    flex: none;
    transition: background-color var(--fast) var(--ease);
  }
  .node:hover {
    --dot-ring: var(--raised);
    background: var(--raised);
  }
  .node.vm {
    height: 28px;
    margin-left: 14px;
    color: var(--text-2);
    font-size: 13px;
  }
  .node.dim {
    color: var(--text-3);
  }
  .label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .count {
    margin-left: auto;
    min-width: 20px;
    height: 18px;
    padding: 0 6px;
    border-radius: 9px;
    display: inline-grid;
    place-items: center;
    font-size: 11px;
    color: var(--text-3);
    background: var(--raised);
  }
  .count.live {
    color: var(--on);
    background: var(--on-soft);
  }
  .count.update {
    color: var(--accent-ink);
    background: var(--accent);
  }
  main {
    overflow-y: auto;
    padding: 28px 32px 48px;
  }
  main > :global(*) {
    max-width: 1280px;
    margin: 0 auto;
  }
  main.fill {
    overflow: hidden;
    padding: 0;
  }
  main.fill > :global(*) {
    max-width: none;
  }
  .machines {
    display: grid;
    gap: 20px;
    animation: enter var(--enter) var(--ease);
  }
  .empty {
    display: grid;
    justify-items: center;
    text-align: center;
    gap: 12px;
    padding-top: 72px;
    max-width: 480px !important;
  }
  .empty h1 {
    margin: 12px 0 0;
    font-family: var(--display);
    font-size: 24px;
    line-height: 30px;
    font-weight: 600;
  }
  .empty p {
    margin: 0 0 8px;
  }
</style>
