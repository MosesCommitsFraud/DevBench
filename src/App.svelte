<script lang="ts">
  import { onMount } from "svelte";
  import { Server, Activity, Settings } from "@lucide/svelte";
  import { api, type Machine, type Vm } from "./lib/api";
  import { t } from "./lib/i18n.svelte";
  import { live, loadConfig, startPolling, store, wakeAll } from "./lib/store.svelte";
  import { notify } from "./lib/ui.svelte";
  import MachinePanel from "./components/MachinePanel.svelte";
  import SettingsView from "./components/SettingsView.svelte";
  import ActivityView from "./components/ActivityView.svelte";
  import VncView from "./components/VncView.svelte";
  import Overlays from "./components/Overlays.svelte";
  import UpdateNotice from "./components/UpdateNotice.svelte";
  import Titlebar from "./components/Titlebar.svelte";

  type View = "machines" | "activity" | "settings";
  let view = $state<View>("machines");
  let screen = $state<{ machine: Machine; vm: Vm } | null>(null);
  let ready = $state(false);

  onMount(async () => {
    const err = await api.startupError();
    if (err) notify(`${t("error.config")}: ${err}`, "error");
    await loadConfig();
    ready = true;
    startPolling();
  });

  function openScreen(machine: Machine, vm: Vm) {
    screen = { machine, vm };
    store.vncOpen = true;
  }
  function closeScreen() {
    screen = null;
    store.vncOpen = false;
    wakeAll();
  }

  function scrollTo(id: string) {
    view = "machines";
    requestAnimationFrame(() => document.getElementById(`m-${id}`)?.scrollIntoView({ behavior: "smooth", block: "start" }));
  }
</script>

<div class="window">
<Titlebar title={screen ? `${screen.vm.name} · ${screen.machine.name}` : ""} />
<div class="shell">
  <aside>
    <nav>
      <button class="nav" class:active={view === "machines"} onclick={() => (view = "machines")}>
        <Server size={16} strokeWidth={1.5} />{t("nav.machines")}
      </button>
      {#each store.config.machines as m (m.id)}
        {@const l = live(m.id)}
        <button class="nav sub" onclick={() => scrollTo(m.id)}>
          <span class="led" class:on={!!l.status?.online} class:busy={!l.status && (l.checking || !!l.pending)}></span>
          <span class="label">{m.name}</span>
        </button>
      {/each}
      <div class="gap"></div>
      <button class="nav" class:active={view === "activity"} onclick={() => (view = "activity")}>
        <Activity size={16} strokeWidth={1.5} />{t("nav.activity")}
      </button>
      <button class="nav" class:active={view === "settings"} onclick={() => (view = "settings")}>
        <Settings size={16} strokeWidth={1.5} />{t("nav.settings")}
      </button>
    </nav>
    <UpdateNotice />
  </aside>

  <main>
    {#if !ready}
      <div></div>
    {:else if view === "machines"}
      {#if store.config.machines.length === 0}
        <div class="empty">
          <h1>{t("empty.title")}</h1>
          <p class="secondary">{t("empty.body")}</p>
          <button class="btn primary" onclick={() => (view = "settings")}>{t("empty.action")}</button>
        </div>
      {:else}
        <div class="machines">
          {#each store.config.machines as m (m.id)}
            <div id="m-{m.id}"><MachinePanel machine={m} onOpenScreen={(vm) => openScreen(m, vm)} /></div>
          {/each}
        </div>
      {/if}
    {:else if view === "activity"}
      <ActivityView />
    {:else}
      <SettingsView />
    {/if}
  </main>
</div>
</div>

{#if screen}
  <VncView machine={screen.machine} vm={screen.vm} onClose={closeScreen} />
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
    grid-template-columns: 240px minmax(0, 1fr);
    flex: 1;
    min-height: 0;
  }
  aside {
    display: flex;
    flex-direction: column;
    gap: 24px;
    padding: 16px 12px;
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
  .gap {
    height: 16px;
    flex: none;
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
    background: var(--raised);
    color: var(--text);
    box-shadow: var(--highlight);
  }
  .nav :global(svg) {
    color: var(--text-3);
  }
  .nav.active :global(svg) {
    color: var(--text-2);
  }
  .nav.sub {
    padding-left: 12px;
    gap: 14px;
  }
  .nav.sub .label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  main {
    overflow-y: auto;
    padding: 32px;
  }
  main > :global(*) {
    max-width: 1200px;
    margin: 0 auto;
  }
  .machines {
    display: grid;
    gap: 16px;
    animation: enter var(--enter) var(--ease);
  }
  .empty {
    display: grid;
    justify-items: start;
    gap: 12px;
    padding-top: 64px;
  }
  .empty h1 {
    margin: 0;
    font-size: 24px;
    line-height: 32px;
    font-weight: 500;
  }
  .empty p {
    margin: 0 0 8px;
    max-width: 480px;
  }
  @keyframes enter {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
  }
</style>
