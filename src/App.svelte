<script lang="ts">
  import { onMount } from "svelte";
  import { Server, Activity, Settings, FolderOpen, Sun, Moon } from "@lucide/svelte";
  import { api, type Machine, type Vm } from "./lib/api";
  import { t } from "./lib/i18n.svelte";
  import { live, loadConfig, startPolling, store, wakeAll } from "./lib/store.svelte";
  import { notify } from "./lib/ui.svelte";
  import { theme, toggleTheme } from "./lib/theme.svelte";
  import MachinePanel from "./components/MachinePanel.svelte";
  import SettingsView from "./components/SettingsView.svelte";
  import ActivityView from "./components/ActivityView.svelte";
  import FilesView from "./components/FilesView.svelte";
  import VncView from "./components/VncView.svelte";
  import TermView from "./components/TermView.svelte";
  import Overlays from "./components/Overlays.svelte";
  import UpdateNotice from "./components/UpdateNotice.svelte";
  import Titlebar from "./components/Titlebar.svelte";
  import logo from "../docs/brand/logo.svg?url";

  type View = "machines" | "files" | "activity" | "settings";
  let view = $state<View>("machines");
  /** A screen (VNC) or terminal covering the window; vm null = the machine itself. */
  let session = $state<{ kind: "screen" | "terminal"; machine: Machine; vm: Vm | null } | null>(null);
  let ready = $state(false);

  onMount(async () => {
    const err = await api.startupError();
    if (err) notify(`${t("error.config")}: ${err}`, "error");
    await loadConfig();
    ready = true;
    startPolling();
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

  function scrollTo(id: string) {
    view = "machines";
    requestAnimationFrame(() => document.getElementById(`m-${id}`)?.scrollIntoView({ behavior: "smooth", block: "start" }));
  }

  const sessionTitle = $derived(session ? [session.vm?.name, session.machine.name].filter(Boolean).join(" · ") : "");
</script>

<div class="window">
  <Titlebar title={sessionTitle} />
  <div class="shell">
    <aside>
      <nav>
        <button class="nav" class:active={view === "machines"} onclick={() => (view = "machines")}>
          <Server size={16} strokeWidth={1.5} />{t("nav.machines")}
        </button>
        {#each store.config.machines as m (m.id)}
          {@const l = live(m.id)}
          {@const running = l.status?.vms.filter((v) => v.running).length ?? 0}
          <button class="nav sub" onclick={() => scrollTo(m.id)}>
            <span class="led" class:on={!!l.status?.online} class:busy={!l.status && (l.checking || !!l.pending)}></span>
            <span class="label">{m.name}</span>
            {#if running}<span class="count mono" title={t("machine.vmsRunning", running)}>{running}</span>{/if}
          </button>
        {/each}

        <button class="nav first-gap" class:active={view === "files"} onclick={() => (view = "files")}>
          <FolderOpen size={16} strokeWidth={1.5} />{t("nav.files")}
        </button>
        <button class="nav" class:active={view === "activity"} onclick={() => (view = "activity")}>
          <Activity size={16} strokeWidth={1.5} />{t("nav.activity")}
          {#if store.agents.length}<span class="count mono live">{store.agents.length}</span>{/if}
        </button>
      </nav>

      <div class="foot">
        <UpdateNotice />
        <button class="nav" class:active={view === "settings"} onclick={() => (view = "settings")}>
          <Settings size={16} strokeWidth={1.5} />{t("nav.settings")}
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
            <img src={logo} alt="" width="72" />
            <h1>{t("empty.title")}</h1>
            <p class="secondary">{t("empty.body")}</p>
            <button class="btn primary" onclick={() => (view = "settings")}>{t("empty.action")}</button>
          </div>
        {:else}
          <div class="machines">
            {#each store.config.machines as m (m.id)}
              <div id="m-{m.id}">
                <MachinePanel machine={m} onOpen={(kind, vm) => open(kind, m, vm)} />
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

<Overlays />

<style>
  .window {
    display: flex;
    flex-direction: column;
    height: 100%;
  }
  .shell {
    display: grid;
    grid-template-columns: 232px minmax(0, 1fr);
    flex: 1;
    min-height: 0;
  }
  aside {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 12px;
    background: var(--bg-deep);
    border-right: 1px solid var(--line);
    min-height: 0;
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
  .nav.sub {
    /* LED centred under the 16px icon column */
    padding-left: 12px;
    gap: 14px;
    height: 30px;
  }
  .nav.first-gap {
    margin-top: 16px;
  }
  .nav .label {
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
    justify-items: start;
    gap: 12px;
    padding-top: 64px;
    max-width: 520px !important;
  }
  .empty img {
    margin-bottom: 8px;
  }
  .empty h1 {
    margin: 0;
    font-size: 22px;
    line-height: 28px;
    font-weight: 500;
  }
  .empty p {
    margin: 0 0 8px;
  }
</style>
