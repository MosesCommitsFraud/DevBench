<script lang="ts">
  import { onDestroy, onMount, untrack } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import {
    ArrowLeft,
    ArrowUp,
    RefreshCw,
    FolderPlus,
    Pencil,
    Trash2,
    Folder,
    File as FileIcon,
    Laptop,
    Upload,
    Download,
    Copy,
    Scissors,
    ClipboardPaste,
    Search,
    X,
    House,
    HardDrive,
    ChevronRight,
    ChevronDown,
    ChevronUp,
  } from "@lucide/svelte";
  import { api, asAppError, files, newId, type Drive, type FileEntry, type Listing, type Location, type Progress } from "../lib/api";
  import { bytes } from "../lib/format";
  import { lang, t } from "../lib/i18n.svelte";
  import { live, store } from "../lib/store.svelte";
  import { confirm, notify, prompt, run } from "../lib/ui.svelte";
  import { beginDrag, drag, dropTargetAt } from "../lib/drag.svelte";
  import Tile from "./Tile.svelte";
  import Sketch from "./Sketch.svelte";

  // ---- Systems: this PC, every machine, every VM ------------------------------------------

  interface System {
    key: string;
    label: string;
    color: string;
    kind: "pc" | "server" | "vm";
    /** Why it cannot be browsed right now, or "" if it can. */
    unavailable: string;
    led: "on" | "off" | "busy";
  }

  const systems = $derived.by<System[]>(() => {
    const list: System[] = [{ key: "local", label: t("files.thisPc"), color: "", kind: "pc", unavailable: "", led: "on" }];
    for (const m of store.config.machines) {
      const l = live(m.id);
      const online = !!l.status?.online;
      list.push({
        key: `m:${m.id}`,
        label: m.name,
        color: m.color,
        kind: "server",
        unavailable: online || l.checking ? "" : t("files.machineOff", m.name),
        led: online ? "on" : l.checking ? "busy" : "off",
      });
      for (const v of m.vms) {
        const vs = l.status?.vms.find((x) => x.id === v.id);
        const running = !!vs?.running;
        list.push({
          key: `v:${m.id}:${v.id}`,
          label: v.name,
          color: v.color,
          kind: "vm",
          unavailable: !v.guest_ssh_user ? t("files.noGuestUser") : !running && !l.checking ? t("files.vmOff", v.name) : "",
          led: running ? "on" : l.checking ? "busy" : "off",
        });
      }
    }
    return list;
  });

  function locationOf(key: string): Location {
    if (key === "local") return { kind: "local" };
    const [kind, machine, vm] = key.split(":");
    return { kind: "remote", machine, vm: kind === "v" ? vm : null };
  }
  const sysOf = (key: string) => systems.find((s) => s.key === key);

  // ---- Where we are -----------------------------------------------------------------------

  const saved = JSON.parse(localStorage.getItem("devbench.files") ?? "{}") as { sys?: string; paths?: Record<string, string> };
  const requested = store.filesSystem;
  store.filesSystem = null;
  let current = $state(
    requested ?? (saved.sys && (saved.sys === "local" || store.config.machines.some((m) => saved.sys!.includes(m.id))) ? saved.sys : "local"),
  );
  /** The last folder per system, so switching back lands where you were. */
  let paths = $state<Record<string, string>>(saved.paths ?? {});
  /** Listings per system, shown at once while the fresh one loads. */
  const cache = new Map<string, Listing>();

  let listing = $state<Listing | null>(null);
  let loading = $state(false);
  let problem = $state("");
  let history = $state<string[]>([]);

  let selected = $state<Set<string>>(new Set());
  let anchor = $state<number | null>(null);
  let filter = $state("");
  let sortBy = $state<"name" | "size" | "modified">("name");
  let sortAsc = $state(true);
  let editingPath = $state(false);
  let pathInput = $state("");
  let localPlaces = $state<[string, string][]>([]);
  /** Disks per system, fetched once per visit of a system. */
  let drives = $state<Record<string, Drive[]>>({});

  let listEl: HTMLDivElement;
  let filterEl: HTMLInputElement;

  function persist() {
    localStorage.setItem("devbench.files", JSON.stringify({ sys: current, paths }));
  }

  async function open(path: string | null, opts: { keepSelection?: boolean; push?: boolean } = {}) {
    const key = current;
    loading = true;
    try {
      const l = await run(() => files.list(locationOf(key), path), true);
      if (key !== current) return;
      if (opts.push !== false && listing && listing.path !== l.path) history = [...history.slice(-49), listing.path];
      if (!listing || listing.path !== l.path) filter = "";
      listing = l;
      cache.set(key, l);
      paths[key] = l.path;
      persist();
      problem = "";
      editingPath = false;
      if (!opts.keepSelection) selected = new Set();
      else selected = new Set([...selected].filter((p) => l.entries.some((e) => e.path === p)));
    } catch (e) {
      if (key !== current) return;
      problem = asAppError(e).message;
    } finally {
      if (key === current) loading = false;
    }
  }

  async function refresh() {
    await open(listing?.path ?? paths[current] ?? null, { keepSelection: true, push: false });
  }

  function choose(key: string) {
    if (key === current) return;
    current = key;
    persist();
  }

  // Switching systems: show the cached folder at once, then load it fresh
  $effect(() => {
    const key = current;
    untrack(() => {
      listing = cache.get(key) ?? null;
      selected = new Set();
      history = [];
      problem = "";
      filter = "";
      editingPath = false;
      if (!sysOf(key)?.unavailable) {
        void open(paths[key] ?? null, { push: false });
        void loadDrives(key);
      }
    });
  });

  async function loadDrives(key: string) {
    try {
      drives[key] = await files.drives(locationOf(key));
    } catch {
      // the places still work; a failed listing shows its own error
    }
  }

  // A machine or VM that comes online while selected gets loaded
  $effect(() => {
    const s = sysOf(current);
    if (s && !s.unavailable && !listing && !loading) untrack(() => void open(paths[current] ?? null, { push: false }));
  });

  function back() {
    const prev = history.at(-1);
    if (!prev) return;
    history = history.slice(0, -1);
    void open(prev, { push: false });
  }

  // ---- Entries ------------------------------------------------------------------------------

  const entries = $derived.by(() => {
    if (!listing) return [];
    const q = filter.trim().toLowerCase();
    const list = q ? listing.entries.filter((e) => e.name.toLowerCase().includes(q)) : [...listing.entries];
    const dir = sortAsc ? 1 : -1;
    list.sort((a, b) => {
      if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
      if (sortBy === "size") return (a.size - b.size) * dir;
      if (sortBy === "modified") return (a.modified - b.modified) * dir;
      return a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: "base" }) * dir;
    });
    return list;
  });

  function sort(by: typeof sortBy) {
    if (sortBy === by) sortAsc = !sortAsc;
    else {
      sortBy = by;
      sortAsc = by === "name";
    }
  }

  const selection = $derived(entries.filter((e) => selected.has(e.path)));
  const selectionSize = $derived(selection.reduce((n, e) => n + (e.is_dir ? 0 : e.size), 0));

  function click(e: MouseEvent, entry: FileEntry, index: number) {
    const next = new Set(e.ctrlKey || e.metaKey ? selected : []);
    if (e.shiftKey && anchor !== null) {
      const [a, b] = [Math.min(anchor, index), Math.max(anchor, index)];
      for (const x of entries.slice(a, b + 1)) next.add(x.path);
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

  // ---- Breadcrumb ---------------------------------------------------------------------------

  const crumbs = $derived.by(() => {
    const p = listing?.path ?? "";
    if (!p) return [];
    if (p.includes("\\")) {
      // Windows on this PC: C:\Users\name
      const parts = p.split("\\").filter(Boolean);
      return parts.map((name, i) => ({ name: i === 0 ? name : name, path: i === 0 ? `${name}\\` : parts.slice(0, i + 1).join("\\") }));
    }
    const parts = p.split("/").filter(Boolean);
    return [{ name: "/", path: "/" }, ...parts.map((name, i) => ({ name, path: "/" + parts.slice(0, i + 1).join("/") }))];
  });

  function editPath() {
    pathInput = listing?.path ?? "";
    editingPath = true;
  }

  // ---- Places of the current system ---------------------------------------------------------

  const places = $derived.by(() => {
    const out: { label: string; path: string; home: boolean }[] = [];
    if (current === "local") {
      const names: Record<string, string> = { home: t("files.home"), desktop: t("files.desktop"), downloads: t("files.downloads"), documents: t("files.documents") };
      for (const [k, p] of localPlaces) out.push({ label: names[k] ?? k, path: p, home: k === "home" });
    } else if (listing) {
      out.push({ label: t("files.home"), path: listing.places[0], home: true });
    }
    return out;
  });

  /** Disks of the current system; without an answer yet, the roots the listing knows. */
  const driveList = $derived.by(() => {
    const known = drives[current];
    if (known?.length) return known;
    return (listing?.places.slice(1) ?? []).map((path) => ({ path, label: "", total: 0, free: 0 }));
  });

  function driveName(d: Drive): string {
    if (d.label) return d.label;
    if (d.path === "/") return t("files.systemDisk");
    const win = /^\/?([A-Za-z]:)[\\/]?$/.exec(d.path);
    if (win) return win[1];
    return d.path.split(/[\\/]/).filter(Boolean).pop() ?? d.path;
  }

  // ---- Actions ------------------------------------------------------------------------------

  async function newFolder() {
    if (!listing) return;
    const name = await prompt(t("files.newFolder"), t("files.folderName"), "", t("files.create"));
    if (!name) return;
    await run(() => files.mkdir(locationOf(current), listing!.path, name)).catch(() => {});
    await refresh();
  }

  async function rename() {
    const entry = selection[0];
    if (!entry || selection.length !== 1 || !listing) return;
    const name = await prompt(t("files.rename"), t("files.newName"), entry.name, t("files.rename"));
    if (!name || name === entry.name) return;
    await run(() => files.rename(locationOf(current), listing!.path, entry.path, name)).catch(() => {});
    await refresh();
  }

  async function remove() {
    const paths = selection.map((e) => e.path);
    if (!paths.length) return;
    const what = paths.length === 1 ? selection[0].name : t("files.items", paths.length);
    if (!(await confirm(t("files.delete.confirm", what), t("files.delete.body"), t("files.delete")))) return;
    await run(() => files.remove(locationOf(current), paths)).catch(() => {});
    await refresh();
  }

  // Clipboard across systems: copy or cut here, paste anywhere
  let clip = $state<{ key: string; paths: string[]; move: boolean } | null>(null);

  function copy(move: boolean) {
    if (!selection.length) return;
    clip = { key: current, paths: selection.map((e) => e.path), move };
    notify(t(move ? "files.cutDone" : "files.copyDone", selection.length === 1 ? selection[0].name : t("files.items", selection.length)));
  }

  async function paste(dest = listing?.path) {
    if (!clip || !dest) return;
    const c = clip;
    if (c.move) clip = null;
    await transfer(c.key, c.paths, current, dest, c.move);
  }

  async function upload() {
    if (!listing) return;
    const picked = await openDialog({ multiple: true, directory: false, title: t("files.upload") });
    const list = Array.isArray(picked) ? picked : picked ? [picked] : [];
    if (list.length) await transfer("local", list, current, listing.path, false);
  }

  async function download() {
    if (!selection.length) return;
    const dest = localPlaces.find(([k]) => k === "downloads")?.[1] ?? localPlaces[0]?.[1];
    if (!dest) return;
    await transfer(current, selection.map((e) => e.path), "local", dest, false, t("files.savedTo", t("files.downloads")));
  }

  /** The folder a drop onto a system goes to: where you last were there, else its home. */
  async function folderOf(key: string): Promise<string | null> {
    if (paths[key]) return paths[key];
    try {
      const l = await run(() => files.list(locationOf(key), null));
      paths[key] = l.path;
      cache.set(key, l);
      return l.path;
    } catch {
      return null;
    }
  }

  // ---- Transfers ----------------------------------------------------------------------------

  interface Job extends Progress {
    label: string;
    done?: string;
  }
  let jobs = $state<Job[]>([]);
  let jobsOpen = $state(true);
  const runningJobs = $derived(jobs.filter((j) => j.state === "running").length);

  async function transfer(fromKey: string, sources: string[], toKey: string, dest: string, move: boolean, done?: string) {
    if (!sources.length) return;
    const id = newId();
    const first = sources[0].split(/[\\/]/).pop() ?? "";
    const where = sysOf(toKey)?.label ?? "";
    const label = `${sources.length === 1 ? first : t("files.items", sources.length)} → ${where}`;
    jobs.unshift({ id, label, done, current: "", files_done: 0, bytes_done: 0, bytes_total: 0, state: "running", error: null });
    jobsOpen = true;
    await run(() => files.copy(id, locationOf(fromKey), sources, locationOf(toKey), dest, move)).catch(() => {
      const j = jobs.find((x) => x.id === id);
      if (j) j.state = "failed";
    });
  }

  function percent(j: Job) {
    return j.bytes_total ? j.bytes_done / j.bytes_total : j.state === "done" ? 1 : 0;
  }

  // ---- Dragging -----------------------------------------------------------------------------

  /** Drop targets: "sys:<key>" a system in the list, "dir:<path>" a folder row, "here" the list. */
  function acceptsInternal(target: string) {
    if (target.startsWith("sys:")) {
      const s = sysOf(target.slice(4));
      return !!s && !s.unavailable && s.key !== current;
    }
    if (target.startsWith("dir:")) return !selected.has(target.slice(4));
    return false;
  }

  async function dropInternal(target: string, shift: boolean) {
    const sources = selection.map((e) => e.path);
    if (target.startsWith("sys:")) {
      const key = target.slice(4);
      const dest = await folderOf(key);
      if (dest) await transfer(current, sources, key, dest, shift);
    } else if (target.startsWith("dir:")) {
      // Inside one system dragging moves, like a file manager; Shift copies instead
      await transfer(current, sources, current, target.slice(4), !shift);
    }
  }

  function pointerDown(e: PointerEvent, entry: FileEntry, index: number) {
    if (e.button !== 0) return;
    const modifier = e.ctrlKey || e.metaKey || e.shiftKey;
    if (!modifier && !selected.has(entry.path)) {
      selected = new Set([entry.path]);
      anchor = index;
    }
    if (!modifier) beginDrag(e, selected.size, acceptsInternal, (target, shift) => void dropInternal(target, shift));
  }

  /** Files dragged in from Explorer/Finder, and where they would land. */
  let osDrop = $state<string | null>(null);

  function osTarget(x: number, y: number): string | null {
    const target = dropTargetAt(x, y);
    if (!target) return null;
    if (target.startsWith("sys:")) {
      const s = sysOf(target.slice(4));
      return s && !s.unavailable && s.key !== "local" ? target : null;
    }
    return current === "local" || !listing ? null : target;
  }

  async function dropFromOs(target: string, sources: string[]) {
    if (target.startsWith("sys:")) {
      const key = target.slice(4);
      const dest = await folderOf(key);
      if (dest) await transfer("local", sources, key, dest, false);
    } else if (target.startsWith("dir:")) {
      await transfer("local", sources, current, target.slice(4), false);
    } else if (listing) {
      await transfer("local", sources, current, listing.path, false);
    }
  }

  // ---- Keyboard -----------------------------------------------------------------------------

  function onKey(e: KeyboardEvent) {
    const typing = (e.target as HTMLElement).tagName === "INPUT";
    const mod = e.ctrlKey || e.metaKey;
    if (mod && e.key.toLowerCase() === "f") {
      e.preventDefault();
      filterEl?.focus();
      return;
    }
    if (mod && e.key.toLowerCase() === "l") {
      e.preventDefault();
      editPath();
      return;
    }
    if (e.key === "Escape" && filter) {
      filter = "";
      listEl?.focus();
      return;
    }
    if (typing) return;
    if (e.key === "Delete") void remove();
    else if (e.key === "F2") void rename();
    else if (e.key === "F5") void refresh();
    else if (e.key === "Backspace" || (e.altKey && e.key === "ArrowLeft")) back();
    else if (e.altKey && e.key === "ArrowUp" && listing?.parent) void open(listing.parent);
    else if (e.key === "Enter" && selection[0]) activate(selection[0]);
    else if (mod && e.key.toLowerCase() === "a") {
      e.preventDefault();
      selected = new Set(entries.map((x) => x.path));
    } else if (mod && e.key.toLowerCase() === "c") copy(false);
    else if (mod && e.key.toLowerCase() === "x") copy(true);
    else if (mod && e.key.toLowerCase() === "v") void paste();
    else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const i = entries.findIndex((x) => x.path === [...selected].at(-1));
      const next = Math.max(0, Math.min(entries.length - 1, i + (e.key === "ArrowDown" ? 1 : -1)));
      if (entries[next]) {
        selected = new Set([entries[next].path]);
        anchor = next;
        listEl?.querySelectorAll(".row")[next]?.scrollIntoView({ block: "nearest" });
      }
    }
  }

  // ---- Setup --------------------------------------------------------------------------------

  let unlisten: (() => void)[] = [];

  onMount(async () => {
    localPlaces = await api.localPlaces().catch(() => []);
    unlisten.push(
      await listen<Progress>("transfer", (e) => {
        const j = jobs.find((x) => x.id === e.payload.id);
        if (!j) return;
        if (e.payload.state === "running") {
          Object.assign(j, e.payload, { label: j.label, done: j.done });
        } else {
          j.state = e.payload.state;
          j.error = e.payload.error;
          if (j.state === "done" && j.done) notify(j.done);
          void refresh();
        }
      }),
    );
    unlisten.push(
      await getCurrentWebview().onDragDropEvent((e) => {
        const p = e.payload;
        if (p.type === "leave") {
          osDrop = null;
          return;
        }
        const target = osTarget(p.position.x / devicePixelRatio, p.position.y / devicePixelRatio);
        if (p.type === "over" || p.type === "enter") osDrop = target;
        else if (p.type === "drop") {
          osDrop = null;
          if (target && p.paths.length) void dropFromOs(target, p.paths);
        }
      }),
    );
  });

  onDestroy(() => unlisten.forEach((f) => f()));

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

  const sys = $derived(sysOf(current));
  const isRemote = $derived(current !== "local");
  const dropOver = (target: string) => drag.over === target || osDrop === target;
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="files" onkeydown={onKey}>
  <aside class="rail">
    <div class="group">
      <span class="caption">{t("files.systems")}</span>
      {#each systems as s (s.key)}
        <button
          class="sys"
          class:vm={s.kind === "vm"}
          class:active={s.key === current}
          class:dim={!!s.unavailable}
          class:drop={dropOver(`sys:${s.key}`)}
          data-drop="sys:{s.key}"
          title={s.unavailable}
          onclick={() => choose(s.key)}
        >
          {#if s.kind === "pc"}
            <span class="pc"><Laptop size={14} strokeWidth={1.75} /></span>
          {:else}
            <Tile name={s.label} color={s.color} size={s.kind === "vm" ? 18 : 22} status={s.led === "on" ? "on" : s.led === "busy" ? "busy" : "off"} />
          {/if}
          <span class="label">{s.label}</span>
        </button>
      {/each}
    </div>

    {#if places.length}
      <div class="group">
        <span class="caption">{t("files.places")}</span>
        {#each places as p (p.path)}
          <button class="sys" class:active={listing?.path === p.path} onclick={() => open(p.path)} title={p.path}>
            {#if p.home}<House size={16} strokeWidth={1.5} />{:else if /^[A-Z]:\\$|^\/$/.test(p.path)}<HardDrive size={16} strokeWidth={1.5} />{:else}<Folder size={16} strokeWidth={1.5} />{/if}
            <span class="label">{p.label}</span>
          </button>
        {/each}
      </div>
    {/if}

    {#if driveList.length}
      <div class="group">
        <span class="caption">{t("files.drives")}</span>
        {#each driveList as d (d.path)}
          {@const used = d.total ? 1 - d.free / d.total : 0}
          <button class="sys drive" class:active={listing?.path === d.path} onclick={() => open(d.path)} title={d.path}>
            <HardDrive size={16} strokeWidth={1.5} />
            <span class="drive-text">
              <span class="label">{driveName(d)}{#if d.label || d.path !== "/"}<span class="where mono">{d.path}</span>{/if}</span>
              {#if d.total}
                <span class="meter" class:hot={used > 0.9}><i style:transform="scaleX({used})"></i></span>
                <span class="free">{t("files.freeOf", bytes(d.free), bytes(d.total))}</span>
              {/if}
            </span>
          </button>
        {/each}
      </div>
    {/if}
  </aside>

  <section class="browser">
    <div class="toolbar">
      <div class="nav-buttons">
        <button class="btn ghost icon" title={t("files.back")} aria-label={t("files.back")} disabled={!history.length} onclick={back}>
          <ArrowLeft size={16} strokeWidth={1.5} />
        </button>
        <button class="btn ghost icon" title={t("files.up")} aria-label={t("files.up")} disabled={!listing?.parent} onclick={() => listing?.parent && open(listing.parent)}>
          <ArrowUp size={16} strokeWidth={1.5} />
        </button>
      </div>

      {#if editingPath}
        <form class="path-edit" onsubmit={(e) => { e.preventDefault(); void open(pathInput); }}>
          <!-- svelte-ignore a11y_autofocus -->
          <input class="mono" bind:value={pathInput} spellcheck="false" autofocus aria-label={t("files.path")} onblur={() => (editingPath = false)} onkeydown={(e) => e.key === "Escape" && (editingPath = false)} />
        </form>
      {:else}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div class="crumbs" onclick={(e) => e.target === e.currentTarget && editPath()} title={t("files.editPath")}>
          {#if sys}<span class="crumb-sys">{sys.label}</span>{/if}
          {#each crumbs as c, i (c.path)}
            {#if i > 0 || sys}<ChevronRight size={14} strokeWidth={1.5} />{/if}
            <button class="crumb" class:last={i === crumbs.length - 1} data-drop={i < crumbs.length - 1 ? `dir:${c.path}` : undefined} class:drop={dropOver(`dir:${c.path}`)} onclick={() => open(c.path)}>{c.name}</button>
          {/each}
        </div>
      {/if}

      <label class="filter">
        <Search size={14} strokeWidth={1.5} />
        <input bind:this={filterEl} bind:value={filter} placeholder={t("files.filter")} spellcheck="false" aria-label={t("files.filter")} />
        {#if filter}<button class="clear" aria-label={t("files.clearFilter")} onclick={() => (filter = "")}><X size={14} strokeWidth={1.5} /></button>{/if}
      </label>
      <button class="btn ghost icon" title={t("files.refresh")} aria-label={t("files.refresh")} onclick={refresh}>
        <RefreshCw size={16} strokeWidth={1.5} />
      </button>
    </div>

    <div class="actions">
      {#if isRemote}
        <button class="btn" disabled={!listing} onclick={upload}><Upload size={16} strokeWidth={1.5} />{t("files.upload")}</button>
        <button class="btn" disabled={!selection.length} onclick={download}><Download size={16} strokeWidth={1.5} />{t("files.download")}</button>
        <span class="sep"></span>
      {/if}
      <button class="btn ghost" disabled={!selection.length} onclick={() => copy(false)} title="Ctrl+C"><Copy size={16} strokeWidth={1.5} />{t("files.copy")}</button>
      <button class="btn ghost" disabled={!selection.length} onclick={() => copy(true)} title="Ctrl+X"><Scissors size={16} strokeWidth={1.5} />{t("files.cut")}</button>
      <button class="btn ghost" class:ready={!!clip} disabled={!clip || !listing} onclick={() => paste()} title="Ctrl+V">
        <ClipboardPaste size={16} strokeWidth={1.5} />{clip ? t("files.pasteN", clip.paths.length) : t("files.paste")}
      </button>
      <span class="sep"></span>
      <button class="btn ghost icon" title={t("files.newFolder")} aria-label={t("files.newFolder")} disabled={!listing} onclick={newFolder}><FolderPlus size={16} strokeWidth={1.5} /></button>
      <button class="btn ghost icon" title={`${t("files.rename")} (F2)`} aria-label={t("files.rename")} disabled={selection.length !== 1} onclick={rename}><Pencil size={16} strokeWidth={1.5} /></button>
      <button class="btn ghost icon quiet-danger" title={`${t("files.delete")} (Entf)`} aria-label={t("files.delete")} disabled={!selection.length} onclick={remove}><Trash2 size={16} strokeWidth={1.5} /></button>
    </div>

    <div class="table">
      <div class="head">
        {#each [["name", t("files.name")], ["size", t("files.size")], ["modified", t("files.modified")]] as [key, label] (key)}
          <button class="col {key}" class:sorted={sortBy === key} onclick={() => sort(key as typeof sortBy)}>
            {label}
            {#if sortBy === key}{#if sortAsc}<ChevronUp size={12} strokeWidth={1.5} />{:else}<ChevronDown size={12} strokeWidth={1.5} />{/if}{/if}
          </button>
        {/each}
      </div>
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <div class="list" class:loading class:drop={osDrop === "here"} bind:this={listEl} tabindex="0" role="listbox" aria-multiselectable="true" data-drop="here">
        {#if sys?.unavailable}
          <div class="note">
            <Sketch kind={sys.kind === "vm" ? "off" : "sleeping"} size={64} />
            <p>{sys.unavailable}</p>
          </div>
        {:else if problem}
          <div class="note"><p class="mono muted">{problem}</p></div>
        {:else if listing && entries.length === 0}
          <div class="note">
            <Sketch kind="empty" size={56} label={filter ? "" : t("files.emptyHand")} />
            {#if filter}<p class="muted">{t("files.noMatch", filter)}</p>{/if}
          </div>
        {:else if listing}
          {#each entries as e, i (e.path)}
            <div
              class="row"
              class:sel={selected.has(e.path)}
              class:drop={e.is_dir && dropOver(`dir:${e.path}`)}
              data-drop={e.is_dir ? `dir:${e.path}` : undefined}
              role="option"
              aria-selected={selected.has(e.path)}
              tabindex="-1"
              onpointerdown={(ev) => pointerDown(ev, e, i)}
              onclick={(ev) => click(ev, e, i)}
              ondblclick={() => activate(e)}
              onkeydown={() => {}}
            >
              <span class="name" class:dir={e.is_dir}>
                {#if e.is_dir}<Folder size={16} strokeWidth={1.5} />{:else}<FileIcon size={16} strokeWidth={1.5} />{/if}
                <span class="label">{e.name}</span>
              </span>
              <span class="size mono">{e.is_dir ? "" : bytes(e.size)}</span>
              <span class="modified mono muted">{modified(e.modified)}</span>
            </div>
          {/each}
        {:else if loading}
          <div class="note"><p class="muted">{t("files.loading")}</p></div>
        {/if}
      </div>
    </div>

    <footer class="status mono">
      <span>
        {#if selection.length}{t("files.selected", selection.length)}{#if selectionSize} · {bytes(selectionSize)}{/if}
        {:else if listing}{t("files.items", entries.length)}{/if}
      </span>
      {#if clip}<span class="muted">{t(clip.move ? "files.clipCut" : "files.clipCopy", clip.paths.length, sysOf(clip.key)?.label ?? "")}</span>{/if}
      <span class="muted hint">{isRemote ? t("files.hintRemote") : t("files.hintLocal")}</span>
    </footer>

    {#if jobs.length}
      <section class="jobs" class:closed={!jobsOpen}>
        <button class="jobs-head" onclick={() => (jobsOpen = !jobsOpen)}>
          {#if jobsOpen}<ChevronDown size={14} strokeWidth={1.5} />{:else}<ChevronUp size={14} strokeWidth={1.5} />{/if}
          <span>{t("files.transfers")}</span>
          {#if runningJobs}<span class="mono muted">{t("files.running", runningJobs)}</span>{/if}
          <span class="grow"></span>
          {#if jobs.some((j) => j.state !== "running")}
            <span
              class="link"
              role="button"
              tabindex="0"
              onclick={(e) => { e.stopPropagation(); jobs = jobs.filter((j) => j.state === "running"); }}
              onkeydown={() => {}}
            >{t("files.clear")}</span>
          {/if}
        </button>
        {#if jobsOpen}
          <div class="job-list">
            {#each jobs as j (j.id)}
              <div class="job">
                <span class="led" class:on={j.state === "done"} class:busy={j.state === "running"} class:error={j.state === "failed"}></span>
                <span class="job-label" title={j.current}>{j.label}</span>
                <div class="bar"><i style:transform="scaleX({percent(j)})" class:done={j.state === "done"}></i></div>
                <span class="mono muted job-meta" title={j.error ?? ""}>
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
          </div>
        {/if}
      </section>
    {/if}
  </section>

  {#if drag.active}
    <div class="drag-label" style:left="{drag.x + 14}px" style:top="{drag.y + 10}px">
      {t("files.items", drag.count)}{#if drag.over}<span class="muted"> · {drag.over.startsWith("sys:") ? (drag.move ? t("files.move") : t("files.copy")) : drag.move ? t("files.copy") : t("files.move")}</span>{/if}
    </div>
  {/if}
</div>

<style>
  .files {
    display: grid;
    grid-template-columns: 224px minmax(0, 1fr);
    height: 100%;
    animation: enter var(--enter) var(--ease);
  }

  /* Systems and places */
  .rail {
    display: flex;
    flex-direction: column;
    gap: 20px;
    padding: 20px 12px;
    border-right: 1px solid var(--line);
    background: var(--bg);
    overflow-y: auto;
  }
  .group {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .caption {
    padding: 0 8px 6px;
    font-size: 12px;
    color: var(--text-3);
  }
  .sys {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 32px;
    padding: 0 8px;
    border: none;
    border-radius: var(--r-control);
    background: transparent;
    color: var(--text-2);
    text-align: left;
    cursor: pointer;
    transition:
      background-color var(--fast) var(--ease),
      color var(--fast) var(--ease),
      box-shadow var(--fast) var(--ease);
  }
  .sys {
    --dot-ring: var(--bg);
  }
  .sys:hover,
  .sys.active {
    --dot-ring: var(--card);
  }
  .sys.vm {
    margin-left: 14px;
    height: 30px;
  }
  .sys.drive {
    height: auto;
    min-height: 32px;
    padding: 7px 8px;
    align-items: flex-start;
  }
  .drive-text {
    flex: 1;
    min-width: 0;
    display: grid;
    gap: 4px;
  }
  .drive-text .label {
    display: flex;
    align-items: baseline;
    gap: 6px;
    min-width: 0;
    line-height: 16px;
  }
  .where {
    font-size: 10.5px;
    color: var(--text-3);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .drive-text .meter {
    display: block;
    height: 3px;
    border-radius: 2px;
    background: var(--line);
    overflow: hidden;
  }
  .drive-text .meter i {
    display: block;
    height: 100%;
    background: var(--text-3);
    transform-origin: left;
    transition: transform 600ms var(--ease);
  }
  .drive-text .meter.hot i {
    background: var(--danger);
  }
  .free {
    font-size: 11px;
    color: var(--text-3);
  }
  .pc {
    width: 22px;
    height: 22px;
    border-radius: 7px;
    display: grid;
    place-items: center;
    background: var(--raised);
  }
  .sys :global(svg) {
    flex: none;
    color: var(--text-3);
  }
  .sys:hover {
    background: var(--raised);
    color: var(--text);
  }
  .sys.active {
    background: var(--card);
    color: var(--text);
    box-shadow:
      0 0 0 1px var(--line),
      var(--highlight);
  }
  .sys.active :global(svg) {
    color: var(--accent-text);
  }
  .sys.dim {
    color: var(--text-3);
  }
  .sys.drop {
    background: var(--accent-soft);
    box-shadow: 0 0 0 1px var(--accent);
    color: var(--text);
  }
  .sys .label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Browser */
  .browser {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    position: relative;
  }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 16px 20px 8px;
  }
  .nav-buttons {
    display: flex;
    gap: 2px;
  }
  .crumbs,
  .path-edit {
    flex: 1;
    min-width: 0;
    height: 36px;
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 0 6px 0 12px;
    border-radius: var(--r-control);
    border: 1px solid var(--line);
    background: var(--card);
    overflow: hidden;
    cursor: text;
  }
  .crumbs :global(svg) {
    flex: none;
    color: var(--text-3);
  }
  .crumb-sys {
    font-weight: 500;
    white-space: nowrap;
    margin-right: 2px;
  }
  .crumb {
    height: 26px;
    padding: 0 6px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--text-2);
    white-space: nowrap;
    cursor: pointer;
    flex: none;
    transition: background-color var(--fast) var(--ease);
  }
  .crumb:hover {
    background: var(--raised);
    color: var(--text);
  }
  .crumb.last {
    color: var(--text);
    font-weight: 500;
  }
  .crumb.drop {
    background: var(--accent-soft);
    box-shadow: 0 0 0 1px var(--accent);
  }
  .path-edit {
    padding: 0;
    border-color: var(--focus);
  }
  .path-edit input {
    flex: 1;
    height: 100%;
    padding: 0 12px;
    border: none;
    background: transparent;
    outline: none;
  }
  .filter {
    position: relative;
    display: flex;
    align-items: center;
    width: 200px;
    height: 36px;
    border-radius: var(--r-control);
    border: 1px solid var(--line);
    background: var(--card);
    color: var(--text-3);
    padding-left: 10px;
    gap: 6px;
    transition: border-color var(--fast) var(--ease);
  }
  .filter:focus-within {
    border-color: var(--focus);
  }
  .filter input {
    flex: 1;
    min-width: 0;
    height: 100%;
    border: none;
    background: transparent;
    color: var(--text);
    outline: none;
  }
  .filter input::placeholder {
    color: var(--text-3);
  }
  .clear {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border: none;
    background: transparent;
    color: var(--text-3);
    cursor: pointer;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px 20px 12px;
    flex-wrap: wrap;
  }
  .actions .sep {
    width: 1px;
    height: 20px;
    margin: 0 6px;
    background: var(--line-strong);
  }
  .actions .ready {
    color: var(--accent-text);
  }

  .table {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    margin: 0 20px;
    border: 1px solid var(--line);
    border-radius: var(--r-panel);
    background: var(--card);
    box-shadow: var(--shadow-card);
    overflow: hidden;
  }
  .head,
  .row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 96px 148px;
    gap: 16px;
    align-items: center;
  }
  .head {
    height: 36px;
    padding: 0 16px;
    border-bottom: 1px solid var(--line);
  }
  .col {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 24px;
    padding: 0;
    border: none;
    background: transparent;
    color: var(--text-3);
    font-size: 12px;
    cursor: pointer;
  }
  .col.sorted,
  .col:hover {
    color: var(--text-2);
  }
  .col.size,
  .col.modified {
    justify-content: flex-end;
  }
  .list {
    flex: 1;
    overflow-y: auto;
    padding: 4px;
    outline: none;
    transition: opacity var(--fast) var(--ease);
  }
  .list.loading {
    opacity: 0.7;
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
    background: var(--accent-soft);
  }
  .list:focus .row.sel {
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 45%, transparent);
  }
  .row.drop {
    background: var(--accent-soft);
    box-shadow: inset 0 0 0 1px var(--accent);
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
  .name.dir :global(svg) {
    color: var(--accent-text);
  }
  .label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .size,
  .modified {
    text-align: right;
    white-space: nowrap;
  }
  .note {
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 12px;
    min-height: 200px;
    padding: 24px;
    text-align: center;
  }
  .note p {
    margin: 0;
    max-width: 480px;
    overflow-wrap: anywhere;
    color: var(--text-2);
  }
  .status {
    display: flex;
    align-items: center;
    gap: 16px;
    height: 36px;
    padding: 0 24px;
    font-size: 12px;
    color: var(--text-2);
  }
  .status .hint {
    margin-left: auto;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* Transfers */
  .jobs {
    margin: 0 20px 16px;
    border: 1px solid var(--line);
    border-radius: var(--r-panel);
    background: var(--card);
    box-shadow: var(--shadow-card);
    overflow: hidden;
  }
  .jobs-head {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    height: 40px;
    padding: 0 16px;
    border: none;
    background: transparent;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .jobs-head :global(svg) {
    color: var(--text-3);
  }
  .grow {
    flex: 1;
  }
  .link {
    color: var(--text-2);
    font-size: 13px;
  }
  .link:hover {
    color: var(--text);
  }
  .job-list {
    max-height: 168px;
    overflow-y: auto;
    padding: 0 16px 8px;
    border-top: 1px solid var(--line);
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
  .bar i.done {
    background: var(--on);
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

  @media (max-width: 1100px) {
    .head,
    .row {
      grid-template-columns: minmax(0, 1fr) 88px;
    }
    .modified {
      display: none;
    }
    .filter {
      width: 160px;
    }
    .status .hint {
      display: none;
    }
  }
</style>
