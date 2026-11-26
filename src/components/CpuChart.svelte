<script lang="ts">
  import { HISTORY } from "../lib/store.svelte";

  // CPU use over the last polls as a smooth line with a soft fill. Values are 0..100.
  let { values }: { values: number[] } = $props();

  const W = 600;
  const H = 120;
  const PAD = 4;

  function y(v: number) {
    return PAD + (H - 2 * PAD) * (1 - Math.max(0, Math.min(100, v)) / 100);
  }

  // Newest sample at the right edge. The first minute is stretched over the whole width, so the
  // chart is readable right away; after that it keeps a fixed scale and scrolls.
  const span = $derived(Math.min(HISTORY, Math.max(30, values.length)) - 1);
  const points = $derived(values.map((v, i) => [W - (values.length - 1 - i) * (W / span), y(v)] as const));

  // Monotone cubic curve through the points (no overshoot below 0 or above 100)
  const line = $derived.by(() => {
    const p = points;
    if (p.length < 2) return "";
    let d = `M${p[0][0].toFixed(1)},${p[0][1].toFixed(1)}`;
    for (let i = 1; i < p.length; i++) {
      const [x0, y0] = p[i - 1];
      const [x1, y1] = p[i];
      const dx = (x1 - x0) / 3;
      d += ` C${(x0 + dx).toFixed(1)},${y0.toFixed(1)} ${(x1 - dx).toFixed(1)},${y1.toFixed(1)} ${x1.toFixed(1)},${y1.toFixed(1)}`;
    }
    return d;
  });
  const area = $derived(points.length < 2 ? "" : `${line} L${W},${H} L${points[0][0].toFixed(1)},${H} Z`);
  const last = $derived(points.at(-1));
</script>

<svg viewBox="0 0 {W} {H}" preserveAspectRatio="none" aria-hidden="true">
  {#each [25, 50, 75] as g (g)}
    <line class="grid" x1="0" x2={W} y1={y(g)} y2={y(g)} />
  {/each}
  {#if area}
    <path class="area" d={area} />
    <path class="line" d={line} />
  {/if}
</svg>
{#if last}
  <span class="dot" style:top="{(last[1] / H) * 100}%"></span>
{/if}

<style>
  svg {
    display: block;
    width: 100%;
    height: 100%;
    overflow: visible;
  }
  .grid {
    stroke: var(--line);
    stroke-width: 1;
    stroke-dasharray: 2 4;
    vector-effect: non-scaling-stroke;
  }
  .area {
    fill: var(--chart-fill);
    opacity: 0.8;
  }
  .line {
    fill: none;
    stroke: var(--chart);
    stroke-width: 1.75;
    stroke-linejoin: round;
    vector-effect: non-scaling-stroke;
  }
  .dot {
    position: absolute;
    right: -4px;
    width: 8px;
    height: 8px;
    margin-top: -4px;
    border-radius: 50%;
    background: var(--chart);
    box-shadow: 0 0 0 3px var(--card);
    transition: top 600ms var(--ease);
  }
</style>
