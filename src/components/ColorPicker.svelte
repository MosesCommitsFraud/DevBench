<script lang="ts">
  import { Check, Wand } from "@lucide/svelte";
  import { HUES, hue } from "../lib/identity";
  import { t } from "../lib/i18n.svelte";
  import { theme } from "../lib/theme.svelte";

  // Swatches for a machine's or VM's colour; "auto" follows the name.
  // `value` may be missing in configs written before colours existed; that means "auto".
  let { value = $bindable(), name }: { value: string | undefined; name: string } = $props();
  const auto = $derived(hue(name));
</script>

<div class="swatches" role="radiogroup" aria-label={t("field.color")}>
  <button
    type="button"
    class="swatch auto"
    role="radio"
    aria-checked={!value}
    title={t("field.color.auto")}
    style:--c={theme() === "dark" ? auto.dark : auto.light}
    onclick={() => (value = "")}
  >
    {#if !value}<Check size={13} strokeWidth={2.5} />{:else}<Wand size={13} strokeWidth={2} />{/if}
  </button>
  {#each HUES as h (h.key)}
    <button
      type="button"
      class="swatch"
      role="radio"
      aria-checked={value === h.key}
      aria-label={h.key}
      title={h.key}
      style:--c={theme() === "dark" ? h.dark : h.light}
      onclick={() => (value = h.key)}
    >
      {#if value === h.key}<Check size={13} strokeWidth={2.5} />{/if}
    </button>
  {/each}
</div>

<style>
  .swatches {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding: 3px 0;
  }
  .swatch {
    width: 26px;
    height: 26px;
    border-radius: 8px;
    border: none;
    display: grid;
    place-items: center;
    color: #fff;
    background: var(--c);
    cursor: pointer;
    transition: transform var(--fast) var(--ease);
  }
  .swatch:hover {
    transform: scale(1.08);
  }
  .swatch[aria-checked="true"] {
    box-shadow:
      0 0 0 2px var(--card),
      0 0 0 4px var(--c);
  }
  .swatch.auto {
    color: var(--c);
    background: color-mix(in srgb, var(--c) 16%, var(--card));
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--c) 35%, transparent);
  }
  .swatch.auto[aria-checked="true"] {
    box-shadow:
      inset 0 0 0 1px color-mix(in srgb, var(--c) 35%, transparent),
      0 0 0 2px var(--card),
      0 0 0 4px var(--c);
  }
</style>
