<script lang="ts" module>
  import type { Component } from "svelte";

  export interface Command {
    id: string;
    label: string;
    /** Smaller text after the label, e.g. the machine. */
    hint?: string;
    group: string;
    icon?: Component<{ size?: number; strokeWidth?: number }>;
    /** Identity tile instead of an icon. */
    tile?: { name: string; color: string };
    keys?: string;
    run: () => void;
  }
</script>

<script lang="ts">
  import { Search, CornerDownLeft } from "@lucide/svelte";
  import { t } from "../lib/i18n.svelte";
  import Tile from "./Tile.svelte";

  // Everything DevBench can do, by keyboard: Ctrl+K, type a few letters, Enter.
  let { open = $bindable(false), commands }: { open: boolean; commands: Command[] } = $props();

  let query = $state("");
  let index = $state(0);
  let input = $state<HTMLInputElement | null>(null);
  let list = $state<HTMLDivElement | null>(null);

  /** Letters of the query in order inside the text; earlier and tighter matches score higher. */
  function score(text: string, q: string): number {
    if (!q) return 1;
    const s = text.toLowerCase();
    const direct = s.indexOf(q);
    if (direct >= 0) return 1000 - direct;
    let at = -1;
    let gaps = 0;
    for (const ch of q) {
      const next = s.indexOf(ch, at + 1);
      if (next < 0) return 0;
      gaps += next - at - 1;
      at = next;
    }
    return 500 - gaps;
  }

  const results = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return commands
      .map((c) => ({ c, s: score(`${c.label} ${c.hint ?? ""} ${c.group}`, q) }))
      .filter((x) => x.s > 0)
      .sort((a, b) => (q ? b.s - a.s : 0))
      .map((x) => x.c);
  });

  $effect(() => {
    void query;
    index = 0;
  });

  $effect(() => {
    if (open) {
      query = "";
      index = 0;
      queueMicrotask(() => input?.focus());
    }
  });

  function choose(c: Command | undefined) {
    if (!c) return;
    open = false;
    c.run();
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      index = Math.min(results.length - 1, index + 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      index = Math.max(0, index - 1);
    } else if (e.key === "Enter") {
      e.preventDefault();
      choose(results[index]);
    } else if (e.key === "Escape") {
      e.preventDefault();
      open = false;
    } else return;
    list?.querySelector(`[data-i="${index}"]`)?.scrollIntoView({ block: "nearest" });
  }

  function onWindowKey(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
      e.preventDefault();
      open = !open;
    }
  }
</script>

<svelte:window onkeydown={onWindowKey} />

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="scrim" onclick={() => (open = false)}>
    <div class="palette" role="dialog" tabindex="-1" aria-modal="true" aria-label={t("palette.title")} onclick={(e) => e.stopPropagation()}>
      <label class="search">
        <Search size={18} strokeWidth={1.75} />
        <input bind:this={input} bind:value={query} placeholder={t("palette.placeholder")} spellcheck="false" onkeydown={onKey} />
        <span class="keycap">Esc</span>
      </label>
      <div class="results" bind:this={list}>
        {#each results as c, i (c.id)}
          {#if i === 0 || results[i - 1].group !== c.group}
            <div class="group">{c.group}</div>
          {/if}
          <button class="item" class:active={i === index} data-i={i} onmouseenter={() => (index = i)} onclick={() => choose(c)}>
            {#if c.tile}
              <Tile name={c.tile.name} color={c.tile.color} size={22} />
            {:else if c.icon}
              {@const Icon = c.icon}
              <span class="icon"><Icon size={16} strokeWidth={1.5} /></span>
            {/if}
            <span class="label">{c.label}</span>
            {#if c.hint}<span class="hint">{c.hint}</span>{/if}
            {#if c.keys}<span class="keycap">{c.keys}</span>{/if}
            {#if i === index}<span class="enter"><CornerDownLeft size={14} strokeWidth={1.75} /></span>{/if}
          </button>
        {:else}
          <p class="none">{t("palette.none")}</p>
        {/each}
      </div>
    </div>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 80;
    display: grid;
    justify-items: center;
    align-items: start;
    padding-top: 12vh;
    background: rgb(5 8 16 / 0.35);
    animation: fade var(--menu) var(--ease);
  }
  .palette {
    width: min(620px, calc(100vw - 48px));
    border-radius: 18px;
    background: color-mix(in srgb, var(--card) 86%, transparent);
    backdrop-filter: blur(18px) saturate(1.1);
    box-shadow:
      0 0 0 1px var(--line-strong),
      0 24px 64px -24px rgb(0 0 0 / 0.6);
    overflow: hidden;
    animation: rise var(--menu) var(--ease);
  }
  .search {
    display: flex;
    align-items: center;
    gap: 12px;
    height: 56px;
    padding: 0 16px 0 18px;
    border-bottom: 1px solid var(--line);
    color: var(--text-3);
  }
  .search input {
    flex: 1;
    height: 100%;
    border: none;
    background: transparent;
    color: var(--text);
    font-size: 16px;
    outline: none;
  }
  .search input::placeholder {
    color: var(--text-3);
  }
  .results {
    max-height: min(420px, 60vh);
    overflow-y: auto;
    padding: 6px 8px 8px;
  }
  .group {
    padding: 10px 10px 4px;
    font-size: 12px;
    color: var(--text-3);
  }
  .item {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    height: 38px;
    padding: 0 10px;
    border: none;
    border-radius: 10px;
    background: transparent;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .item.active {
    background: var(--accent-soft);
  }
  .icon {
    width: 22px;
    display: grid;
    place-items: center;
    color: var(--text-3);
  }
  .item.active .icon {
    color: var(--accent-text);
  }
  .label {
    white-space: nowrap;
  }
  .hint {
    flex: 1;
    min-width: 0;
    color: var(--text-3);
    font-size: 13px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .enter {
    margin-left: auto;
    color: var(--text-3);
  }
  .hint + .enter,
  .keycap + .enter {
    margin-left: 0;
  }
  .none {
    margin: 16px 10px;
    color: var(--text-3);
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(-6px) scale(0.98);
    }
  }
</style>
