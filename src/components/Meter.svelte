<script lang="ts">
  import { Tween } from "svelte/motion";
  import { cubicOut } from "svelte/easing";

  // One resource as a compact row: label, amount, percentage and a thin bar. value is 0..1.
  let { label, value, detail }: { label: string; value: number; detail: string } = $props();

  // The bar and the number glide between polls instead of jumping
  const shown = new Tween(0, { duration: 600, easing: cubicOut });
  $effect(() => {
    shown.target = Math.max(0, Math.min(1, value || 0));
  });
</script>

<div class="meter">
  <span class="label secondary">{label}</span>
  <span class="figure mono">{Math.round(shown.current * 100)}<small>%</small></span>
  <div class="bar"><i style:transform="scaleX({shown.current})" class:high={shown.current > 0.9}></i></div>
  <span class="detail mono muted">{detail}</span>
</div>

<style>
  .meter {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: baseline;
    gap: 6px 12px;
    min-width: 0;
  }
  .label {
    white-space: nowrap;
  }
  .detail {
    grid-column: 1 / -1;
    font-size: 11px;
    line-height: 14px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .figure {
    font-size: 14px;
    line-height: 20px;
    font-weight: 500;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .figure small {
    margin-left: 2px;
    font-size: 11px;
    color: var(--text-3);
    font-weight: 400;
  }
  .bar {
    grid-column: 1 / -1;
    height: 4px;
    border-radius: 2px;
    background: var(--raised);
    overflow: hidden;
  }
  .bar i {
    display: block;
    height: 100%;
    border-radius: 2px;
    background: var(--text-3);
    transform-origin: left;
  }
  .bar i.high {
    background: var(--busy);
  }
</style>
