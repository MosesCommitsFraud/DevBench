<script lang="ts">
  import { hue, monogram } from "../lib/identity";
  import { theme } from "../lib/theme.svelte";

  // A machine's or VM's monogram on a tint of its colour, with an optional status dot in the corner.
  let {
    name,
    color = "",
    size = 16,
    status = null,
  }: { name: string; color?: string; size?: number; status?: "on" | "off" | "busy" | "error" | null } = $props();

  const h = $derived(hue(name, color));
  const c = $derived(theme() === "dark" ? h.dark : h.light);
  const text = $derived(monogram(name));
</script>

<span class="tile" style:--c={c} style:width="{size}px" style:height="{size}px" aria-hidden="true">
  <svg viewBox="0 0 16 16">
    <text x="8" y="10.8" text-anchor="middle" font-size="8.25" font-weight="700" textLength={text.length === 1 ? 6 : 12} lengthAdjust="spacingAndGlyphs">{text}</text>
  </svg>
  {#if status}<i class="corner {status}"></i>{/if}
</span>

<style>
  .tile {
    position: relative;
    flex: none;
    display: inline-flex;
    color: var(--c);
  }
  svg {
    width: 100%;
    height: 100%;
    border-radius: 25%;
    background: color-mix(in srgb, currentColor 14%, transparent);
    font-family: var(--mono);
    user-select: none;
  }
  text {
    fill: currentColor;
  }
  .corner {
    position: absolute;
    right: -2px;
    bottom: -2px;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: color-mix(in srgb, var(--text-3) 70%, var(--card));
    box-shadow: 0 0 0 1.5px var(--dot-ring, var(--card));
  }
  .corner.on {
    background: var(--on);
  }
  .corner.busy {
    background: var(--busy);
  }
  .corner.error {
    background: var(--danger);
  }
</style>
