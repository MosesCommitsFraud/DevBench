// Ports of machines and guests forwarded to this PC. The backend holds the live listeners; the
// list of what the user forwarded is kept here (localStorage) and opened again at every start.
// A listener costs nothing until something connects, so a sleeping machine does no harm.
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, type ForwardInfo } from "./api";

interface Saved {
  machine: string;
  vm: string | null;
  port: number;
  host: string | null;
  local: number;
}

const KEY = "devbench.forwards";

export const ports = $state({
  list: [] as ForwardInfo[],
  /** The system whose ports dialog is open. */
  dialog: null as { machine: string; vm: string | null } | null,
});

function saved(): Saved[] {
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? "[]") as Saved[];
  } catch {
    return [];
  }
}

function save(list: Saved[]) {
  localStorage.setItem(KEY, JSON.stringify(list));
}

export async function refreshForwards() {
  ports.list = await api.portForwards().catch(() => ports.list);
}

export function forwardsOf(machine: string, vm: string | null): ForwardInfo[] {
  return ports.list.filter((f) => f.machine === machine && (f.vm ?? null) === vm);
}

export async function openForward(machine: string, vm: string | null, port: number, host: string | null = null, local: number | null = null) {
  const f = await api.portForwardOpen(machine, vm, port, host, local);
  save([...saved().filter((s) => !(s.machine === machine && s.vm === vm && s.port === port)), { machine, vm, port, host, local: f.local_port }]);
  await refreshForwards();
  return f;
}

export async function closeForward(f: ForwardInfo) {
  await api.portForwardClose(f.id);
  save(saved().filter((s) => !(s.machine === f.machine && s.vm === (f.vm ?? null) && s.port === f.remote_port)));
  await refreshForwards();
}

export function browse(f: ForwardInfo) {
  void openUrl(`http://localhost:${f.local_port}`);
}

export function openPorts(machine: string, vm: string | null) {
  ports.dialog = { machine, vm };
}

/** Opens the saved forwards again (same local port when it is free) and keeps the list fresh. */
export async function startForwards(knownMachines: string[]) {
  for (const s of saved()) {
    if (!knownMachines.includes(s.machine)) continue;
    await api.portForwardOpen(s.machine, s.vm, s.port, s.host, null).catch(() => {});
  }
  await refreshForwards();
  setInterval(() => {
    if (ports.list.length || ports.dialog) void refreshForwards();
  }, 3000);
}
