<script lang="ts">
  import { explain } from "../lib/errors";
  import { t } from "../lib/i18n.svelte";

  // An error as a sentence of what happened, what to try, and the raw message folded away
  // underneath (for the curious and for bug reports).
  let { message, center = false }: { message: string; center?: boolean } = $props();

  const e = $derived(explain(message));
</script>

<div class="note" class:center>
  <p class="title">{e.title}</p>
  {#if e.hint}<p class="hint">{e.hint}</p>{/if}
  {#if e.detail}
    <details>
      <summary>{t("err.details")}</summary>
      <p class="raw mono">{e.detail}</p>
    </details>
  {/if}
</div>

<style>
  .note {
    display: grid;
    gap: 2px;
    min-width: 0;
  }
  .center {
    justify-items: center;
    text-align: center;
  }
  p {
    margin: 0;
  }
  .title {
    color: var(--text);
  }
  .hint {
    font-size: 12px;
    color: var(--text-3);
  }
  details {
    margin-top: 4px;
    font-size: 11px;
    color: var(--text-3);
  }
  summary {
    cursor: pointer;
    width: fit-content;
    list-style: none;
  }
  .center summary {
    margin: 0 auto;
  }
  summary::before {
    content: "▸ ";
  }
  details[open] summary::before {
    content: "▾ ";
  }
  summary:hover {
    color: var(--text-2);
  }
  .raw {
    margin-top: 4px;
    max-width: 560px;
    overflow-wrap: anywhere;
    user-select: text;
  }
</style>
