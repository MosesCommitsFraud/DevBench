<script lang="ts">
  import { untrack } from "svelte";
  import { ArrowUp, RefreshCw, FolderPlus, Pencil, Trash2, Folder, File as FileIcon } from "@lucide/svelte";
  import { asAppError, files, type FileEntry, type Listing, type Location } from "../lib/api";
  import { bytes } from "../lib/format";
  import { lang, t } from "../lib/i18n.svelte";
  import { store } from "../lib/store.svelte";
  import { confirm, prompt, run } from "../lib/ui.svelte";
  import { beginDrag, drag } from "../lib/drag.svelte";

  let {
    side,
    initial,
  }: {
    side: "left" | "right";
    initial: string;
  } = $props();

  // Location choices: this PC, every machine, and every VM that can be reached over SSH
  const choices = $derived([
    { key: "local", label: t("files.thisPc") },
    ...store.config.machines.flatMap((m) => [
      { key: `m:${m.id}`, label: m.name },
      ...m.vms.filter((v) => v.guest_ssh_user).map((v) => ({ key: `v:${m.id}:${v.id}`, label: `${m.name} / ${v.name}` })),
    ]),
  ]);

  let key = $state(untrack(() => initial));
  let listing = $state<Listing | null>(null);
  let pathInput = $state("");
  let selected = $state<Set<string>>(new Set());
  let anchor = $state<number | null>(null);
  let loading = $state(false);
  let problem = $state("");

  export function location(): Location {
    if (key === "local") return { kind: "local" };
    const [kind, machine, vm] = key.split(":");
    return { kind: "remote", machine, vm: kind === "v" ? vm : null };
  }

  export function path(): string {
    return listing?.path ?? "";
  }

  export function selection(): string[] {
    return listing?.entries.filter((e) => selected.has(e.path)).map((e) => e.path) ?? [];
  }

  export async function refresh() {
    await open(listing?.path ?? null, true);
  }

  async function open(path: string | null, keepSelection = false) {
    loading = true;
    try {
      const l = await run(() => files.list(location(), path), true);
      listing = l;
      pathInput = l.path;
      problem = "";
      if (!keepSelection) selected = new Set();
      else selected = new Set([...selected].filter((p) => l.entries.some((e) => e.path === p)));
    } catch (e) {
      problem = asAppError(e).message;
      if (!listing) pathInput = path ?? "";
    } finally {
      loading = false;
    }
  }

  // Open the home folder whenever the location changes
  $effect(() => {
    void key;
    untrack(() => {
      listing = null;
      selected = new Set();
      problem = "";
      void open(null);
    });
  });

  function click(e: MouseEvent, entry: FileEntry, index: number) {
    const next = new Set(e.ctrlKey || e.metaKey ? selected : []);
    if (e.shiftKey && anchor !== null && listing) {
      const [a, b] = [Math.min(anchor, index), Math.max(anchor, index)];
      for (const x of listing.entries.slice(a, b + 1)) next.add(x.path);
    } else if ((e.ctrlKey || e.metaKey) && next.has(entry.path)) {
      next.delete(entry.path);
    } else {
      next.add(entry.path);
      anchor = index;
    }
    selected = next;
  }

  function activate(entry: FileEntry) {
    if (entry.is_dir) void open(entry.path);
  }

  async function newFolder() {
    const name = await prompt(t("files.newFolder"), t("files.folderName"), "", t("files.create"));
    if (!name || !listing) return;
    await run(() => files.mkdir(location(), listing!.path, name)).catch(() => {});
    await refresh();
  }

  async function rename() {
    const [p] = selection();
    const entry = listing?.entries.find((e) => e.path === p);
    if (!entry || !listing) return;
    const name = await prompt(t("files.rename"), t("files.newName"), entry.name, t("files.rename"));
    if (!name || name === entry.name) return;
    await run(() => files.rename(location(), listing!.path, entry.path, name)).catch(() => {});
    await refresh();
  }

  async function remove() {
    const paths = selection();
    if (!paths.length) return;
    const what = paths.length === 1 ? (listing?.entries.find((e) => e.path === paths[0])?.name ?? "") : t("files.items", paths.length);
    if (!(await confirm(t("files.delete.confirm", what), t("files.delete.body"), t("files.delete")))) return;
    await run(() => files.remove(location(), paths)).catch(() => {});
    await refresh();
  }

  function onKey(e: KeyboardEvent) {
    if ((e.target as HTMLElement).tagName === "INPUT") return;
    if (e.key === "Delete") void remove();
    else if (e.key === "F2") void rename();
    else if (e.key === "Backspace" && listing?.parent) void open(listing.parent);
    else if (e.key === "Enter") {
      const entry = listing?.entries.find((x) => selected.has(x.path));
      if (entry) activate(entry);
    } else if (e.key === "a" && (e.ctrlKey || e.metaKey) && listing) {
      e.preventDefault();
      selected = new Set(listing.entries.map((x) => x.path));
    }
  }

  function pointerDown(e: PointerEvent, entry: FileEntry, index: number) {
    if (e.button !== 0) return;
    const modifier = e.ctrlKey || e.metaKey || e.shiftKey;
    if (!modifier && !selected.has(entry.path)) {
      selected = new Set([entry.path]);
      anchor = index;
    }
    if (!modifier) beginDrag(e, side, selected.size);
  }

  function modified(secs: number) {
    if (!secs) return "";
    return new Date(secs * 1000).toLocaleString(lang() === "de" ? "de-DE" : "en-GB", {
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    });
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<section
  class="pane"
  class:over={drag.active && drag.over === side && drag.from !== side}
  data-pane={side}
  tabindex="0"
  onkeydown={onKey}
  aria-label={side}
>
  <div class="bar">
    <select class="input loc" bind:value={key}>
      {#each choices as c (c.key)}
        <option value={c.key}>{c.label}</option>
      {/each}
    </select>
    {#if listing && listing.places.length > 1}
      <select class="input places" value="" onchange={(e) => open((e.currentTarget as HTMLSelectElement).value)}>
        <option value="" disabled>{t("files.places")}</option>
        {#each listing.places as p (p)}<option value={p}>{p}</option>{/each}
      </select>
    {/if}
  </div>
  <div class="bar">
    <button class="btn ghost icon" title={t("files.up")} aria-label={t("files.up")} disabled={!listing?.parent} onclick={() => listing?.parent && open(listing.parent)}>
      <ArrowUp size={16} strokeWidth={1.5} />
    </button>
    <form class="pathform" onsubmit={(e) => { e.preventDefault(); void open(pathInput); }}>
      <input class="input mono" bind:value={pathInput} spellcheck="false" aria-label="Path" />
    </form>
    <button class="btn ghost icon" title={t("files.refresh")} aria-label={t("files.refresh")} onclick={refresh}>
      <RefreshCw size={16} strokeWidth={1.5} />
    </button>
    <button class="btn ghost icon" title={t("files.newFolder")} aria-label={t("files.newFolder")} disabled={!listing} onclick={newFolder}>
      <FolderPlus size={16} strokeWidth={1.5} />
    </button>
    <button class="btn ghost icon" title={t("files.rename")} aria-label={t("files.rename")} disabled={selected.size !== 1} onclick={rename}>
      <Pencil size={16} strokeWidth={1.5} />
    </button>
    <button class="btn ghost icon" title={t("files.delete")} aria-label={t("files.delete")} disabled={selected.size === 0} onclick={remove}>
      <Trash2 size={16} strokeWidth={1.5} />
    </button>
  </div>

  <div class="head">
    <span>{t("files.name")}</span>
    <span class="num">{t("files.size")}</span>
    <span class="date">{t("files.modified")}</span>
  </div>
  <div class="list" class:loading>
    {#if problem}
      <p class="note mono muted">{problem}</p>
    {:else if listing && listing.entries.length === 0}
      <p class="note muted">{t("files.empty")}</p>
    {:else if listing}
      {#each listing.entries as e, i (e.path)}
        <div
          class="row"
          class:sel={selected.has(e.path)}
          role="option"
          aria-selected={selected.has(e.path)}
          tabindex="-1"
          onpointerdown={(ev) => pointerDown(ev, e, i)}
          onclick={(ev) => click(ev, e, i)}
          ondblclick={() => activate(e)}
          onkeydown={() => {}}
        >
          <span class="name">
            {#if e.is_dir}<Folder size={16} strokeWidth={1.5} />{:else}<FileIcon size={16} strokeWidth={1.5} />{/if}
            <span class="label">{e.name}</span>
          </span>
          <span class="num mono">{e.is_dir ? "" : bytes(e.size)}</span>
          <span class="date mono muted">{modified(e.modified)}</span>
        </div>
      {/each}
    {/if}
  </div>
  <div class="foot muted">
    {#if selected.size}{t("files.selected", selected.size)}{:else if listing}{t("files.items", listing.entries.length)}{/if}
  </div>
</section>

<style>
  .pane {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    border: 1px solid var(--line);
    border-radius: var(--r-panel);
    background: var(--card);
    overflow: hidden;
    outline: none;
    transition: border-color var(--fast) var(--ease);
  }
  .pane:focus-within {
    border-color: var(--line-strong);
  }
  .pane.over {
    border-color: var(--accent);
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 8px 8px 0;
  }
  .bar:first-child {
    padding-top: 12px;
    gap: 8px;
  }
  .input {
    height: 32px;
    padding: 0 10px;
    border-radius: var(--r-control);
    border: 1px solid var(--line-strong);
    background: var(--bg);
    min-width: 0;
  }
  .input:focus {
    outline: none;
    border-color: var(--focus);
  }
  .loc {
    flex: 1;
    font-weight: 500;
  }
  .places {
    width: 112px;
    color: var(--text-2);
  }
  .pathform {
    flex: 1;
    min-width: 0;
    display: flex;
  }
  .pathform input {
    flex: 1;
    width: 100%;
  }
  .head,
  .row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 88px 136px;
    gap: 12px;
    align-items: center;
    padding: 0 16px;
  }
  .head {
    height: 32px;
    margin-top: 8px;
    border-bottom: 1px solid var(--line);
    color: var(--text-3);
    font-size: 12px;
  }
  .list {
    flex: 1;
    overflow-y: auto;
    padding: 4px;
    transition: opacity var(--fast) var(--ease);
  }
  .list.loading {
    opacity: 0.6;
  }
  .row {
    height: 32px;
    padding: 0 12px;
    border-radius: var(--r-control);
    cursor: default;
    user-select: none;
  }
  .row:hover {
    background: var(--raised);
  }
  .row.sel {
    background: color-mix(in srgb, var(--accent) 22%, transparent);
  }
  .name {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .name :global(svg) {
    flex: none;
    color: var(--text-3);
  }
  .label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .num {
    text-align: right;
  }
  .date {
    text-align: right;
    white-space: nowrap;
  }
  .note {
    margin: 16px 12px;
    overflow-wrap: anywhere;
  }
  .foot {
    height: 32px;
    display: flex;
    align-items: center;
    padding: 0 16px;
    border-top: 1px solid var(--line);
    font-size: 12px;
  }
  @media (max-width: 1100px) {
    .head,
    .row {
      grid-template-columns: minmax(0, 1fr) 80px;
    }
    .date {
      display: none;
    }
  }
</style>
