<script lang="ts">
  import type { Component } from "svelte";
  import { Tween } from "svelte/motion";
  import { cubicOut } from "svelte/easing";
  import { hue } from "../lib/identity";
  import { theme } from "../lib/theme.svelte";

  // One resource of a machine: an icon in its colour, a big figure, a ring that fills up and
  // a quiet line of detail. `ratio` null shows no ring (e.g. network).
  let {
    icon: Icon,
    tone,
    label,
    value,
    unit = "",
    ratio = null,
    detail = "",
    children,
  }: {
    icon: Component<{ size?: number; strokeWidth?: number }>;
    tone: string;
    label: string;
    value: string;
    unit?: string;
    ratio?: number | null;
    detail?: string;
    children?: import("svelte").Snippet;
  } = $props();

  const c = $derived(theme() === "dark" ? hue("", tone).dark : hue("", tone).light);
  const shown = new Tween(0, { duration: 700, easing: cubicOut });
  $effect(() => {
    shown.target = Math.max(0, Math.min(1, ratio ?? 0));
  });
  const R = 19;
  const C = 2 * Math.PI * R;
  const hot = $derived((ratio ?? 0) > 0.9);
</script>

<div class="stat" style:--c={c} class:hot>
  <div class="top">
    <span class="icon"><Icon size={16} strokeWidth={1.75} /></span>
    <span class="label">{label}</span>
  </div>
  <div class="mid">
    <div class="figure">
      {#if children}{@render children()}{:else}{value}<small>{unit}</small>{/if}
    </div>
    {#if ratio !== null}
      <svg viewBox="0 0 48 48" aria-hidden="true">
        <circle class="track" cx="24" cy="24" r={R} />
        <circle class="arc" cx="24" cy="24" r={R} stroke-dasharray="{C * shown.current} {C}" />
      </svg>
    {/if}
  </div>
  {#if detail}<span class="detail mono">{detail}</span>{/if}
</div>

<style>
  .stat {
    display: grid;
    gap: 10px;
    padding: 14px 16px;
    border-radius: var(--r-card);
    background: color-mix(in srgb, var(--c) 7%, var(--card));
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--c) 16%, var(--line));
    min-width: 0;
    transition: background-color var(--menu) var(--ease);
  }
  .top {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-2);
    font-size: 13px;
  }
  .icon {
    width: 26px;
    height: 26px;
    border-radius: 8px;
    display: grid;
    place-items: center;
    color: var(--c);
    background: color-mix(in srgb, var(--c) 16%, transparent);
  }
  .mid {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    min-height: 44px;
  }
  .figure {
    font-family: var(--display);
    font-size: 30px;
    line-height: 34px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    min-width: 0;
  }
  .figure small {
    margin-left: 2px;
    font-size: 15px;
    font-weight: 500;
    color: var(--text-3);
  }
  svg {
    width: 44px;
    height: 44px;
    flex: none;
    transform: rotate(-90deg);
  }
  circle {
    fill: none;
    stroke-width: 5;
  }
  .track {
    stroke: color-mix(in srgb, var(--c) 16%, transparent);
  }
  .arc {
    stroke: var(--c);
    stroke-linecap: round;
  }
  .hot .arc {
    stroke: var(--danger);
  }
  .detail {
    color: var(--text-3);
    font-size: 11px;
    line-height: 14px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
