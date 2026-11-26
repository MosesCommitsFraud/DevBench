<script lang="ts">
  import { hue, monogram } from "../lib/identity";
  import { theme } from "../lib/theme.svelte";

  // A machine's or VM's identity: its monogram on its colour, with an optional status dot.
  let {
    name,
    color = "",
    size = 32,
    status = null,
  }: { name: string; color?: string; size?: number; status?: "on" | "off" | "busy" | "error" | null } = $props();

  const h = $derived(hue(name, color));
  const c = $derived(theme() === "dark" ? h.dark : h.light);
</script>

<span
  class="tile"
  style:--c={c}
  style:width="{size}px"
  style:height="{size}px"
  style:font-size="{Math.round(size * (monogram(name).length > 1 ? 0.38 : 0.46))}px"
  style:border-radius="{Math.round(size * 0.3)}px"
  aria-hidden="true"
>
  {monogram(name)}
  {#if status}<i class="dot {status}" style:width="{Math.max(8, size * 0.28)}px" style:height="{Math.max(8, size * 0.28)}px"></i>{/if}
</span>

<style>
  .tile {
    position: relative;
    flex: none;
    display: inline-grid;
    place-items: center;
    font-family: var(--display);
    font-weight: 600;
    letter-spacing: 0.01em;
    color: var(--c);
    background: color-mix(in srgb, var(--c) 16%, var(--card));
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--c) 28%, transparent);
    user-select: none;
  }
  .dot {
    position: absolute;
    right: -2px;
    bottom: -2px;
    border-radius: 50%;
    background: var(--raised-2);
    box-shadow: 0 0 0 2px var(--dot-ring, var(--card));
  }
  .dot.on {
    background: var(--on);
  }
  .dot.busy {
    background: var(--busy);
  }
  .dot.error {
    background: var(--danger);
  }
</style>
