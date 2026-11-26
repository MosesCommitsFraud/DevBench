<script lang="ts">
  import { Tween } from "svelte/motion";
  import { cubicOut } from "svelte/easing";

  let { label, value, detail }: { label: string; value: number; detail: string } = $props();

  // value is 0..1; numbers and bar glide between polls instead of jumping
  const shown = new Tween(0, { duration: 600, easing: cubicOut });
  $effect(() => {
    shown.target = Math.max(0, Math.min(1, value || 0));
  });
</script>

<div class="meter">
  <div class="row">
    <span class="secondary">{label}</span>
    <span class="figure">{Math.round(shown.current * 100)}<small> %</small></span>
  </div>
  <div class="bar"><i style:transform="scaleX({shown.current})" class:high={shown.current > 0.9}></i></div>
  <span class="mono muted detail">{detail}</span>
</div>

<style>
  .meter {
    display: grid;
    gap: 8px;
    min-width: 0;
  }
  .row {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }
  .figure {
    font-family: var(--mono);
    font-size: 20px;
    line-height: 24px;
    font-weight: 500;
    font-variant-numeric: tabular-nums;
  }
  .figure small {
    font-size: 13px;
    color: var(--text-3);
    font-weight: 400;
  }
  .bar {
    height: 4px;
    border-radius: 2px;
    background: var(--raised);
    overflow: hidden;
  }
  .bar i {
    display: block;
    height: 100%;
    background: var(--text-2);
    transform-origin: left;
  }
  .bar i.high {
    background: var(--busy);
  }
  .detail {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
