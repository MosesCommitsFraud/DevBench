<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { ChevronRight, ChevronLeft, X } from "@lucide/svelte";
  import { files, newId, type Location, type Progress } from "../lib/api";
  import { bytes } from "../lib/format";
  import { t } from "../lib/i18n.svelte";
  import { store } from "../lib/store.svelte";
  import { run } from "../lib/ui.svelte";
  import FilePane from "./FilePane.svelte";
  import { drag, setDropHandler } from "../lib/drag.svelte";

  interface Job extends Progress {
    label: string;
  }

  let left: FilePane;
  let right: FilePane;
  let jobs = $state<Job[]>([]);
  let dropSide = $state<"left" | "right" | null>(null);

  const firstMachine = store.config.machines[0];
  const rightInitial = firstMachine ? `m:${firstMachine.id}` : "local";

  function locLabel(l: Location) {
    if (l.kind === "local") return t("files.thisPc");
    const m = store.config.machines.find((x) => x.id === l.machine);
    const v = m?.vms.find((x) => x.id === l.vm);
    return [m?.name, v?.name].filter(Boolean).join(" / ");
  }

  async function transfer(from: FilePane, to: FilePane, moveFiles: boolean, sources = from.selection()) {
    if (!sources.length || !to.path()) return;
    await start(from.location(), sources, to.location(), to.path(), moveFiles);
  }

  async function start(from: Location, sources: string[], to: Location, dest: string, moveFiles: boolean) {
    const id = newId();
    const first = sources[0].split(/[\\/]/).pop() ?? "";
    const label = `${sources.length === 1 ? first : t("files.items", sources.length)} → ${locLabel(to)}`;
    jobs.unshift({ id, label, current: "", files_done: 0, bytes_done: 0, bytes_total: 0, state: "running", error: null });
    await run(() => files.copy(id, from, sources, to, dest, moveFiles)).catch(() => {
      const j = jobs.find((x) => x.id === id);
      if (j) j.state = "failed";
    });
  }

  let unlisten: (() => void)[] = [];

  setDropHandler((from, to, mv) => {
    const panes = { left, right };
    void transfer(panes[from], panes[to], mv);
  });

  onMount(async () => {
    unlisten.push(
      await listen<Progress>("transfer", (e) => {
        const j = jobs.find((x) => x.id === e.payload.id);
        if (!j) return;
        if (e.payload.state === "running") {
          Object.assign(j, e.payload, { label: j.label });
        } else {
          j.state = e.payload.state;
          j.error = e.payload.error;
          void left?.refresh();
          void right?.refresh();
        }
      }),
    );
    // Files dragged in from Explorer/Finder: copy them into the pane under the pointer
    unlisten.push(
      await getCurrentWebview().onDragDropEvent((e) => {
        const p = e.payload;
        if (p.type === "leave") {
          dropSide = null;
          return;
        }
        const x = p.position.x / devicePixelRatio;
        const y = p.position.y / devicePixelRatio;
        const pane = document.elementFromPoint(x, y)?.closest("[data-pane]")?.getAttribute("data-pane") as "left" | "right" | null;
        if (p.type === "over" || p.type === "enter") {
          dropSide = pane;
        } else if (p.type === "drop") {
          dropSide = null;
          const target = pane === "left" ? left : pane === "right" ? right : null;
          if (target && p.paths.length && target.path()) void start({ kind: "local" }, p.paths, target.location(), target.path(), false);
        }
      }),
    );
  });

  onDestroy(() => unlisten.forEach((f) => f()));

  function percent(j: Job) {
    return j.bytes_total ? j.bytes_done / j.bytes_total : j.state === "done" ? 1 : 0;
  }
</script>

<div class="files">
  <div class="panes">
    <div class="slot" class:drop={dropSide === "left"}>
      <FilePane bind:this={left} side="left" initial="local" />
    </div>
    <div class="middle">
      <button class="btn icon" title={t("files.copyRight")} aria-label={t("files.copyRight")} onclick={(e) => transfer(left, right, e.shiftKey)}>
        <ChevronRight size={16} strokeWidth={1.5} />
      </button>
      <button class="btn icon" title={t("files.copyLeft")} aria-label={t("files.copyLeft")} onclick={(e) => transfer(right, left, e.shiftKey)}>
        <ChevronLeft size={16} strokeWidth={1.5} />
      </button>
    </div>
    <div class="slot" class:drop={dropSide === "right"}>
      <FilePane bind:this={right} side="right" initial={rightInitial} />
    </div>
  </div>

  {#if drag.active}
  <div class="drag-label" style:left="{drag.x + 14}px" style:top="{drag.y + 10}px">{t("files.items", drag.count)}</div>
{/if}

{#if jobs.length}
    <section class="jobs">
      <div class="jobs-head">
        <span class="secondary">{t("files.transfers")}</span>
        <button class="btn ghost" onclick={() => (jobs = jobs.filter((j) => j.state === "running"))}>{t("files.clear")}</button>
      </div>
      {#each jobs as j (j.id)}
        <div class="job">
          <span class="led" class:on={j.state === "done"} class:busy={j.state === "running"} class:error={j.state === "failed"}></span>
          <span class="job-label">{j.label}</span>
          <div class="bar"><i style:transform="scaleX({percent(j)})"></i></div>
          <span class="mono muted job-meta">
            {#if j.state === "running"}{bytes(j.bytes_done)} / {bytes(j.bytes_total)}
            {:else if j.state === "failed"}{j.error ?? t("files.failed")}
            {:else}{t(j.state === "done" ? "files.done" : "files.cancelled")}{/if}
          </span>
          {#if j.state === "running"}
            <button class="btn ghost icon" title={t("files.cancel")} aria-label={t("files.cancel")} onclick={() => files.cancel(j.id)}>
              <X size={16} strokeWidth={1.5} />
            </button>
          {:else}
            <span></span>
          {/if}
        </div>
      {/each}
    </section>
  {/if}
</div>

<style>
  .files {
    display: flex;
    flex-direction: column;
    gap: 16px;
    height: 100%;
    max-width: none !important;
  }
  .panes {
    flex: 1;
    min-height: 320px;
    display: grid;
    grid-template-columns: minmax(0, 1fr) 32px minmax(0, 1fr);
    gap: 12px;
  }
  .slot {
    display: flex;
    min-height: 0;
    border-radius: var(--r-panel);
    transition: box-shadow var(--fast) var(--ease);
  }
  .slot > :global(*) {
    flex: 1;
  }
  .slot.drop {
    box-shadow: 0 0 0 2px var(--accent);
  }
  .middle {
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 8px;
  }
  .drag-label {
    position: fixed;
    z-index: 70;
    pointer-events: none;
    padding: 4px 10px;
    border-radius: 6px;
    border: 1px solid var(--line-strong);
    background: var(--card);
    box-shadow: var(--shadow-float);
    font-size: 12px;
  }
  .jobs {
    flex: none;
    max-height: 200px;
    overflow-y: auto;
    border: 1px solid var(--line);
    border-radius: var(--r-panel);
    background: var(--card);
    padding: 4px 16px 8px;
  }
  .jobs-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    height: 40px;
  }
  .job {
    display: grid;
    grid-template-columns: 8px minmax(0, 1fr) 160px minmax(0, 220px) 32px;
    align-items: center;
    gap: 12px;
    min-height: 36px;
  }
  .job-label,
  .job-meta {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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
    background: var(--accent);
    transform-origin: left;
    transition: transform var(--menu) var(--ease);
  }
</style>
