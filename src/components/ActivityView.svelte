<script lang="ts">
  import { onDestroy } from "svelte";
  import { api, type ActivityEntry } from "../lib/api";
  import { time } from "../lib/format";
  import { t } from "../lib/i18n.svelte";
  import { store } from "../lib/store.svelte";

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
  {#if store.agents.length}
    <section class="running">
      <h2>{t("agents.running")}</h2>
      {#each store.agents as a (a.pid)}
        <div class="agent">
          <span class="led on"></span>
          <span class="name">{a.client}</span>
          <span class="mono muted">PID {a.pid}</span>
          <span class="secondary">{a.last_tool ? `${a.last_tool}${a.last_target ? ` · ${a.last_target}` : ""}` : t("agents.idle")}</span>
          <span class="mono muted calls">{t("agents.calls", a.calls)}</span>
        </div>
      {/each}
    </section>
  {/if}
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
  .running {
    border: 1px solid var(--line);
    border-radius: var(--r-panel);
    background: var(--card);
    padding: 12px 20px;
  }
  h2 {
    margin: 0 0 4px;
    font-size: 13px;
    line-height: 20px;
    font-weight: 500;
    color: var(--text-2);
  }
  .agent {
    display: grid;
    grid-template-columns: 8px minmax(80px, auto) auto 1fr auto;
    align-items: center;
    gap: 12px;
    min-height: 36px;
  }
  .agent .name {
    font-weight: 500;
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
    grid-template-columns: 8px 152px 128px minmax(80px, 200px) 1fr;
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
