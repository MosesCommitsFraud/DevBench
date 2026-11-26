<script lang="ts">
  // Little drawings by hand for quiet moments: a sleeping server, a screen that is off, an
  // empty folder. Written underneath in a hand, like the hints on a Kladde board.
  let {
    kind,
    label = "",
    size = 72,
  }: { kind: "sleeping" | "off" | "empty" | "lost" | "rack"; label?: string; size?: number } = $props();
</script>

<div class="sketch" role="img" aria-label={label}>
  <svg viewBox="0 0 80 56" style:height="{size}px" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    {#if kind === "sleeping"}
      <!-- a server box, eyes closed, and z z z floating off -->
      <path d="M12 22.5l35-1.2 1 25-35.5 1.4z" />
      <path d="M13 31l34.5-1" />
      <path d="M13.3 39l34.6-.8" />
      <path d="M19 26.5q2 1.6 4 0M27 26.2q2 1.6 4 0" />
      <circle cx="42" cy="35" r=".9" fill="currentColor" />
      <circle cx="42" cy="43" r=".9" fill="currentColor" />
      <path d="M55 21h6l-6 7h6" />
      <path d="M64 10h4.5L64 15h4.5" stroke-width="1.4" />
      <path d="M70 3h3l-3 3.5h3" stroke-width="1.2" />
    {:else if kind === "off"}
      <!-- a screen that is off, its stand, and the power sign -->
      <path d="M10 8.5l58-1.5 1.2 34-58.5 1.3z" />
      <path d="M36 42.5l-2 7M44 42.3l1.6 7.2M29 50.2l22-.6" />
      <path d="M45.5 20.5a8.5 8.5 0 1 1-11.5.3" />
      <path d="M39.8 16.2v9" />
    {:else if kind === "empty"}
      <!-- an open folder with nothing in it but a few specks -->
      <path d="M10 15.5l17-.6 4 5 30-.8.8 26.5-51.3 1.2z" />
      <path d="M10.5 26l51.5-1.2" />
      <circle cx="30" cy="36" r=".8" fill="currentColor" />
      <circle cx="41" cy="34" r=".8" fill="currentColor" />
      <circle cx="49" cy="38" r=".8" fill="currentColor" />
      <path d="M66 12l5-4M68 18l6-1" stroke-width="1.3" />
    {:else if kind === "lost"}
      <!-- a plug and a socket that do not meet -->
      <path d="M8 30h14M22 23.5l8-.5.5 14-8.5.4z" />
      <path d="M30.5 26.5h5M30.6 33.5h5" />
      <path d="M50 22.5l9 .2-.3 15-9-.4z" />
      <path d="M59 30h13" />
      <path d="M41 25l3 4-3 4" stroke-dasharray="2 2.5" />
    {:else}
      <!-- a small rack: two servers stacked, one waving -->
      <path d="M16 12l40-1 .8 14-40.6 1z" />
      <path d="M16.3 29.5l40.5-.8.7 14.3-41 .8z" />
      <circle cx="49" cy="18.5" r="1" fill="currentColor" />
      <circle cx="49.5" cy="36.5" r="1" fill="currentColor" />
      <path d="M22 18h14M22.5 36h14" />
      <path d="M62 8q4-3 7 1M64 4q5-3 9 2" stroke-width="1.3" />
    {/if}
  </svg>
  {#if label}<span class="hand">{label}</span>{/if}
</div>

<style>
  .sketch {
    display: inline-flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    color: var(--sketch);
  }
  svg {
    width: auto;
    transform: rotate(-3deg);
    overflow: visible;
  }
  .hand {
    font-family: var(--hand);
    font-size: 20px;
    line-height: 22px;
    transform: rotate(-2deg);
  }
</style>
