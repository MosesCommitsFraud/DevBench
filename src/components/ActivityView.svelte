<script lang="ts">
  import { onDestroy } from "svelte";
  import { api, type ActivityEntry } from "../lib/api";
  import { time } from "../lib/format";
  import { t } from "../lib/i18n.svelte";

  let entries = $state<ActivityEntry[]>([]);

  async function load() {
    entries = await api.activity(200).catch(() => []);
  }
  void load();
  const timer = setInterval(load, 3000);
  onDestroy(() => clearInterval(timer));
</script>

<div class="activity">
  <header>
    <h1>{t("activity.title")}</h1>
    <p class="secondary">{t("activity.hint")}</p>
  </header>
  {#if entries.length === 0}
    <p class="muted">{t("activity.empty")}</p>
  {:else}
    <ol>
      {#each entries as e, i (i)}
        <li>
          <span class="led" class:on={e.ok} class:error={!e.ok}></span>
          <span class="mono muted">{time(e.time)}</span>
          <span class="mono tool">{e.tool}</span>
          <span class="secondary where">{[e.machine, e.vm].filter(Boolean).join(" / ")}</span>
          <span class="summary">{e.ok ? e.summary : e.error || e.summary}</span>
        </li>
      {/each}
    </ol>
  {/if}
</div>

<style>
  .activity {
    display: grid;
    gap: 16px;
  }
  header {
    display: grid;
    gap: 4px;
  }
  h1 {
    margin: 0;
    font-size: 24px;
    line-height: 32px;
    font-weight: 500;
  }
  p {
    margin: 0;
  }
  ol {
    list-style: none;
    margin: 0;
    padding: 0 20px;
    border: 1px solid var(--line);
    border-radius: var(--r-panel);
    background: var(--card);
  }
  li {
    display: grid;
    grid-template-columns: 8px 128px 128px minmax(80px, 200px) 1fr;
    align-items: center;
    gap: 12px;
    min-height: 40px;
    border-bottom: 1px solid var(--line);
  }
  li:last-child {
    border-bottom: none;
  }
  .summary,
  .where {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
