<script lang="ts">
  import { Plus, Trash2, FolderOpen, Download, RefreshCw, Search } from "@lucide/svelte";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { api, emptyMachine, emptyVm, type Config, type Machine, type SshHostEntry } from "../lib/api";
  import { langChoice, setLang, t } from "../lib/i18n.svelte";
  import { setConfig, store } from "../lib/store.svelte";
  import { notify, run } from "../lib/ui.svelte";

  let draft = $state<Config>(structuredClone($state.snapshot(store.config)));
  let saved = $state(JSON.stringify(store.config));
  const dirty = $derived(JSON.stringify(draft) !== saved);
  let path = $state("");
  let importList = $state<SshHostEntry[] | null>(null);
  let saving = $state(false);

  api.configPath().then((p) => (path = p));

  async function save() {
    saving = true;
    try {
      const cfg = await run(() => api.saveConfig($state.snapshot(draft)));
      setConfig(cfg);
      draft = structuredClone(cfg);
      saved = JSON.stringify(cfg);
      notify(t("settings.saved"));
    } catch {
      // shown by run()
    } finally {
      saving = false;
    }
  }

  function discard() {
    draft = JSON.parse(saved);
  }

  async function reload() {
    try {
      const cfg = await run(() => api.reloadConfig());
      setConfig(cfg);
      draft = structuredClone(cfg);
      saved = JSON.stringify(cfg);
    } catch {
      // shown by run()
    }
  }

  function addMachine(m: Machine = emptyMachine()) {
    draft.machines.push(m);
  }

  async function openImport() {
    importList = importList ? null : await api.sshConfigHosts();
  }

  function importHost(h: SshHostEntry) {
    const m = emptyMachine();
    m.name = h.alias;
    m.ssh_host = h.alias;
    addMachine(m);
  }

  const imported = (alias: string) => draft.machines.some((m) => m.ssh_host === alias);

  async function detectVms(m: Machine) {
    if (!store.config.machines.some((s) => s.id === m.id)) {
      notify(t("settings.detect.save"), "error");
      return;
    }
    const uri = m.vms[0]?.libvirt_uri ?? "qemu:///system";
    try {
      const names = await run(() => api.listDomains(m.id, uri));
      const fresh = names.filter((n) => !m.vms.some((v) => v.libvirt_name === n));
      if (!fresh.length) notify(t("settings.detect.none"));
      for (const n of fresh) {
        const vm = emptyVm(n);
        vm.libvirt_uri = uri;
        m.vms.push(vm);
      }
    } catch {
      // shown by run()
    }
  }
</script>

<div class="settings">
  <header class="page-head">
    <h1>{t("settings.title")}</h1>
    <div class="head-actions">
      <button class="btn ghost" onclick={openImport}><Download size={16} strokeWidth={1.5} />{t("settings.import")}</button>
      <button class="btn" onclick={() => addMachine()}><Plus size={16} strokeWidth={1.5} />{t("settings.addMachine")}</button>
    </div>
  </header>

  {#if importList}
    <section class="card import">
      {#if importList.length === 0}
        <p class="secondary">{t("settings.import.none")}</p>
      {:else}
        {#each importList as h (h.alias)}
          <div class="import-row">
            <span>{h.alias}</span>
            <span class="mono muted">{h.user ? `${h.user}@` : ""}{h.host_name}{h.port !== 22 ? `:${h.port}` : ""}{h.proxy_jump ? ` via ${h.proxy_jump}` : ""}</span>
            {#if imported(h.alias)}
              <span class="muted">{t("settings.import.added")}</span>
            {:else}
              <button class="btn" onclick={() => importHost(h)}><Plus size={16} strokeWidth={1.5} />{t("settings.addMachine")}</button>
            {/if}
          </div>
        {/each}
      {/if}
    </section>
  {/if}

  {#each draft.machines as m, mi (m.id)}
    <section class="card">
      <div class="card-head">
        <h2>{m.name || m.ssh_host || t("settings.addMachine")}</h2>
        <button class="btn ghost" onclick={() => draft.machines.splice(mi, 1)}><Trash2 size={16} strokeWidth={1.5} />{t("settings.removeMachine")}</button>
      </div>

      <h3>{t("section.connection")}</h3>
      <div class="grid">
        <label class="field"><span>{t("field.name")}</span><input bind:value={m.name} placeholder={m.ssh_host} /></label>
        <label class="field wide">
          <span>{t("field.sshHost")}</span><input bind:value={m.ssh_host} placeholder="server.lan" spellcheck="false" />
          <small>{t("field.sshHost.hint")}</small>
        </label>
        <label class="field"><span>{t("field.sshUser")}</span><input bind:value={m.ssh_user} placeholder={t("field.fromSshConfig")} spellcheck="false" /></label>
        <label class="field"><span>{t("field.sshPort")}</span><input type="number" min="0" max="65535" bind:value={m.ssh_port} /></label>
        <label class="field wide"><span>{t("field.sshKey")}</span><input bind:value={m.ssh_key} placeholder={t("field.fromSshConfig")} spellcheck="false" /></label>
        <label class="field wide">
          <span>{t("field.jump")}</span><input bind:value={m.jump_host} placeholder={t("field.fromSshConfig")} spellcheck="false" />
          <small>{t("field.jump.hint")}</small>
        </label>
      </div>

      <h3>{t("section.wol")}</h3>
      <div class="grid">
        <label class="field">
          <span>{t("field.mac")}</span><input bind:value={m.mac} placeholder="AA:BB:CC:DD:EE:FF" spellcheck="false" />
          <small>{t("field.mac.hint")}</small>
        </label>
        <label class="field"><span>{t("field.broadcast")}</span><input bind:value={m.wol_broadcast} spellcheck="false" /></label>
        <label class="field"><span>{t("field.wolPort")}</span><input type="number" min="1" max="65535" bind:value={m.wol_port} /></label>
      </div>

      <div class="sub-head">
        <h3>{t("section.vms")}</h3>
        <div class="head-actions">
          <button class="btn ghost" onclick={() => detectVms(m)}><Search size={16} strokeWidth={1.5} />{t("settings.detectVms")}</button>
          <button class="btn ghost" onclick={() => m.vms.push(emptyVm())}><Plus size={16} strokeWidth={1.5} />{t("settings.addVm")}</button>
        </div>
      </div>
      {#each m.vms as vm, vi (vm.id)}
        <div class="vm">
          <div class="vm-head">
            <span class="vm-title">{vm.name || vm.libvirt_name || "VM"}</span>
            <button class="btn ghost" onclick={() => m.vms.splice(vi, 1)}><Trash2 size={16} strokeWidth={1.5} />{t("settings.removeVm")}</button>
          </div>
          <div class="grid">
            <label class="field"><span>{t("field.name")}</span><input bind:value={vm.name} placeholder={vm.libvirt_name} /></label>
            <label class="field"><span>{t("field.libvirtName")}</span><input bind:value={vm.libvirt_name} spellcheck="false" /></label>
            <label class="field"><span>{t("field.libvirtUri")}</span><input bind:value={vm.libvirt_uri} spellcheck="false" /></label>
            <label class="field">
              <span>{t("field.vncMode")}</span>
              <select bind:value={vm.vnc_mode}>
                <option value="guest">{t("field.vncMode.guest")}</option>
                <option value="host">{t("field.vncMode.host")}</option>
                <option value="none">{t("field.vncMode.none")}</option>
              </select>
            </label>
            {#if vm.vnc_mode !== "none"}
              <label class="field"><span>{t("field.vncTarget")}</span><input bind:value={vm.vnc_host} spellcheck="false" /></label>
              <label class="field"><span>{t("field.vncPort")}</span><input type="number" min="1" max="65535" bind:value={vm.vnc_port} /></label>
              <label class="field">
                <span>{t("field.vncPassword")}</span><input type="password" bind:value={vm.vnc_password} autocomplete="off" />
                <small>{t("field.vncPassword.hint")}</small>
              </label>
            {/if}
            <label class="field">
              <span>{t("field.guestAddress")}</span><input bind:value={vm.guest_address} placeholder="auto" spellcheck="false" />
              <small>{t("field.guestAddress.hint")}</small>
            </label>
            <label class="field"><span>{t("field.guestUser")}</span><input bind:value={vm.guest_ssh_user} spellcheck="false" /></label>
            <label class="field"><span>{t("field.guestPort")}</span><input type="number" min="1" max="65535" bind:value={vm.guest_ssh_port} /></label>
            <label class="field wide"><span>{t("field.guestKey")}</span><input bind:value={vm.guest_ssh_key} placeholder={t("field.sameAsMachine")} spellcheck="false" /></label>
          </div>
          <div class="checks">
            <label class="check"><input type="checkbox" bind:checked={vm.agent_access} disabled={!m.agent_access} />{t("agent.vm.access")}</label>
            <label class="check"><input type="checkbox" bind:checked={vm.agent_exec} disabled={!m.agent_access} />{t("agent.vm.exec")}</label>
          </div>
        </div>
      {/each}

      <h3>{t("section.agents")}</h3>
      <p class="muted hint">{t("agents.hint")}</p>
      <div class="checks">
        <label class="check"><input type="checkbox" bind:checked={m.agent_access} />{t("agent.access")}</label>
        <label class="check"><input type="checkbox" bind:checked={m.agent_exec} disabled={!m.agent_access} />{t("agent.exec")}</label>
        <label class="check"><input type="checkbox" bind:checked={m.agent_power} disabled={!m.agent_access} />{t("agent.power")}</label>
      </div>
    </section>
  {/each}

  <section class="card">
    <div class="grid">
      <label class="field">
        <span>{t("settings.language")}</span>
        <select value={langChoice()} onchange={(e) => setLang((e.currentTarget as HTMLSelectElement).value as "auto" | "de" | "en")}>
          <option value="auto">{t("settings.language.auto")}</option>
          <option value="de">Deutsch</option>
          <option value="en">English</option>
        </select>
      </label>
      <div class="field wide">
        <span>{t("settings.file")}</span>
        <div class="path">
          <code class="mono secondary">{path}</code>
          <button class="btn ghost" onclick={() => revealItemInDir(path)}><FolderOpen size={16} strokeWidth={1.5} />{t("settings.openFolder")}</button>
          <button class="btn ghost" onclick={reload}><RefreshCw size={16} strokeWidth={1.5} />{t("settings.reload")}</button>
        </div>
      </div>
    </div>
  </section>

  {#if dirty}
    <div class="savebar">
      <span class="secondary">{t("settings.unsaved")}</span>
      <button class="btn ghost" onclick={discard}>{t("settings.discard")}</button>
      <button class="btn primary" disabled={saving} onclick={save}>{t("settings.save")}</button>
    </div>
  {/if}
</div>

<style>
  .settings {
    display: grid;
    gap: 16px;
    padding-bottom: 80px;
  }
  .page-head,
  .card-head,
  .sub-head,
  .vm-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }
  .head-actions {
    display: flex;
    gap: 8px;
  }
  h1 {
    margin: 0;
    font-size: 24px;
    line-height: 32px;
    font-weight: 500;
  }
  h2 {
    margin: 0;
    font-size: 16px;
    line-height: 24px;
    font-weight: 500;
  }
  h3 {
    margin: 12px 0 0;
    font-size: 13px;
    line-height: 20px;
    font-weight: 500;
    color: var(--text-2);
  }
  .sub-head h3 {
    margin: 0;
  }
  .sub-head {
    margin-top: 12px;
  }
  .card {
    background: var(--card);
    border: 1px solid var(--line);
    border-radius: var(--r-panel);
    padding: 20px 24px;
    display: grid;
    gap: 12px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 12px 16px;
  }
  .wide {
    grid-column: span 2;
  }
  .vm {
    border: 1px solid var(--line);
    border-radius: 12px;
    padding: 12px 16px 16px;
    display: grid;
    gap: 12px;
  }
  .vm-title {
    font-weight: 500;
  }
  .checks {
    display: grid;
    gap: 8px;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--text-2);
  }
  .check input {
    width: 16px;
    height: 16px;
    margin: 0;
    accent-color: var(--accent);
  }
  .check:has(input:disabled) {
    opacity: 0.5;
  }
  .hint {
    margin: 0;
  }
  .import {
    gap: 0;
    padding: 8px 24px;
  }
  .import p {
    margin: 8px 0;
  }
  .import-row {
    display: grid;
    grid-template-columns: minmax(120px, auto) 1fr auto;
    align-items: center;
    gap: 16px;
    min-height: 48px;
    border-bottom: 1px solid var(--line);
  }
  .import-row:last-child {
    border-bottom: none;
  }
  .path {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .path code {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
    min-width: 0;
  }
  .savebar {
    position: fixed;
    right: 32px;
    bottom: 24px;
    z-index: 30;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 8px 8px 16px;
    border-radius: 12px;
    border: 1px solid var(--line-strong);
    background: var(--card);
    box-shadow: var(--shadow-float);
  }
  @media (max-width: 760px) {
    .wide {
      grid-column: auto;
    }
  }
</style>
