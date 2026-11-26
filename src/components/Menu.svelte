<script lang="ts">
  import type { Snippet } from "svelte";

  let { label, trigger, children }: { label: string; trigger: Snippet; children: Snippet } = $props();
  let open = $state(false);
  let root: HTMLDivElement;

  function onWindowClick(e: MouseEvent) {
    if (open && !root.contains(e.target as Node)) open = false;
  }
  function onKey(e: KeyboardEvent) {
    if (open && e.key === "Escape") open = false;
  }
</script>

<svelte:window onclick={onWindowClick} onkeydown={onKey} />

<div class="menu" bind:this={root}>
  <button class="btn ghost icon" aria-label={label} title={label} aria-expanded={open} onclick={() => (open = !open)}>
    {@render trigger()}
  </button>
  {#if open}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div class="list" role="menu" tabindex="-1" onclick={() => (open = false)}>
      {@render children()}
    </div>
  {/if}
</div>

<style>
  .menu {
    position: relative;
  }
  .list {
    position: absolute;
    right: 0;
    top: calc(100% + 4px);
    z-index: 20;
    min-width: 200px;
    padding: 4px;
    border-radius: 12px;
    border: 1px solid var(--line-strong);
    background: var(--card);
    box-shadow: var(--shadow-float);
    display: grid;
    animation: rise var(--menu) var(--ease);
  }
  .list :global(button) {
    height: 32px;
    padding: 0 12px;
    border: none;
    border-radius: var(--r-control);
    background: transparent;
    text-align: left;
    cursor: pointer;
    transition: background-color var(--fast) var(--ease);
  }
  .list :global(button:hover) {
    background: var(--raised);
  }
  .list :global(button.danger) {
    color: var(--danger);
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
  }
</style>
