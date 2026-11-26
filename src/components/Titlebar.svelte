<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { Minus, Square, Copy, X } from "@lucide/svelte";
  import logo from "../../docs/brand/logo.svg?url";

  let { title = "" }: { title?: string } = $props();
  const win = getCurrentWindow();
  let maximized = $state(false);

  onMount(() => {
    const update = async () => (maximized = await win.isMaximized());
    void update();
    const un = win.onResized(update);
    return () => void un.then((f) => f());
  });
</script>

<header class="titlebar" data-tauri-drag-region>
  <div class="brand" data-tauri-drag-region>
    <img class="mark" src={logo} alt="" draggable="false" />
    <span data-tauri-drag-region>DevBench</span>
    {#if title}<span class="sep" data-tauri-drag-region>/</span><span class="secondary" data-tauri-drag-region>{title}</span>{/if}
  </div>
  <div class="controls">
    <button aria-label="Minimize" onclick={() => win.minimize()}><Minus size={16} strokeWidth={1.5} /></button>
    <button aria-label={maximized ? "Restore" : "Maximize"} onclick={() => win.toggleMaximize()}>
      {#if maximized}<Copy size={14} strokeWidth={1.5} />{:else}<Square size={14} strokeWidth={1.5} />{/if}
    </button>
    <button class="close" aria-label="Close" onclick={() => win.close()}><X size={16} strokeWidth={1.5} /></button>
  </div>
</header>

<style>
  .titlebar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 40px;
    flex: none;
    background: var(--bg-deep);
    border-bottom: 1px solid var(--line);
    user-select: none;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 100%;
    flex: 1;
    padding-left: 16px;
    font-weight: 500;
  }
  .mark {
    height: 14px;
    width: auto;
    pointer-events: none;
  }
  .sep {
    color: var(--text-3);
  }
  .controls {
    display: flex;
    height: 100%;
  }
  .controls button {
    width: 46px;
    height: 100%;
    display: grid;
    place-items: center;
    border: none;
    background: transparent;
    color: var(--text-2);
    cursor: default;
    transition:
      background-color var(--fast) var(--ease),
      color var(--fast) var(--ease);
  }
  .controls button:hover {
    background: var(--raised);
    color: var(--text);
  }
  .controls .close:hover {
    background: #c42b1c;
    color: #fff;
  }
</style>
