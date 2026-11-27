// The loaded config and the live status of every machine, polled in the background.
import { api, asAppError, type AppError, type Config, type MachineStatus, type RunningAgent } from "./api";

export interface Live {
  status: MachineStatus | null;
  error: AppError | null;
  checking: boolean;
  /** Pending action label shown on the machine, e.g. while waking up. */
  pending: string | null;
  /**
   * When polls started failing while the machine was online (ms). Its last status stays for a
   * while, marked as not responding, so a busy server (updates, a restarting SSH service) is not
   * shown as asleep after one failed poll.
   */
  lostAt: number | null;
}

/** How long a machine that stops answering keeps its last status before it counts as offline. */
export const GRACE_MS = 60_000;

export const store = $state({
  config: { machines: [] } as Config,
  live: {} as Record<string, Live>,
  /** True while a VNC view covers the dashboard: poll less often. */
  vncOpen: false,
  /** The system the file browser should open on next (e.g. "m:<id>"), set by "Files" buttons. */
  filesSystem: null as string | null,
  /** `devbench mcp` processes connected right now. */
  agents: [] as RunningAgent[],
});

async function pollAgents() {
  store.agents = await api.runningAgents().catch(() => []);
}

const boostUntil: Record<string, number> = {};
const inFlight = new Set<string>();

export async function loadConfig() {
  setConfig(await api.getConfig());
}

/** Replaces the config and makes sure every machine has a live entry. */
export function setConfig(config: Config) {
  for (const m of config.machines) {
    if (!store.live[m.id]) store.live[m.id] = { status: null, error: null, checking: true, pending: null, lostAt: null };
  }
  store.config = config;
  if (started) startPolling();
}

const placeholder: Live = { status: null, error: null, checking: true, pending: null, lostAt: null };

export function live(id: string): Live {
  return store.live[id] ?? placeholder;
}

/** Poll this machine every 2 s for a while, to show the effect of an action quickly. */
export function boost(id: string, seconds = 60) {
  boostUntil[id] = Date.now() + seconds * 1000;
  wakers[id]?.();
}

export async function poll(id: string) {
  if (inFlight.has(id)) return;
  inFlight.add(id);
  const l = live(id);
  try {
    const st = await api.machineStatus(id);
    l.status = st;
    l.error = null;
    l.lostAt = null;
  } catch (e) {
    l.error = asAppError(e);
    if (l.status?.online) {
      l.lostAt ??= Date.now();
      if (Date.now() - l.lostAt > GRACE_MS) l.status = null;
    } else {
      l.status = null;
    }
  } finally {
    l.checking = false;
    inFlight.delete(id);
  }
}

function interval(id: string): number {
  if ((boostUntil[id] ?? 0) > Date.now()) return 1000;
  if (store.vncOpen || document.hidden) return 10000;
  return 2000;
}

const loops = new Set<string>();
const wakers: Record<string, () => void> = {};

/** One steady loop per machine: poll, then wait for the rest of the interval. */
async function loop(id: string) {
  loops.add(id);
  while (store.config.machines.some((m) => m.id === id)) {
    const t0 = Date.now();
    await poll(id);
    const wait = Math.max(0, interval(id) - (Date.now() - t0));
    await new Promise<void>((resolve) => {
      const timer = setTimeout(resolve, wait);
      wakers[id] = () => {
        clearTimeout(timer);
        resolve();
      };
    });
  }
  loops.delete(id);
}

let started = false;

export function startPolling() {
  if (!started) {
    void pollAgents();
    setInterval(pollAgents, 3000);
  }
  started = true;
  for (const m of store.config.machines) if (!loops.has(m.id)) void loop(m.id);
}

/** Poll every machine now (e.g. when the window becomes visible again). */
export function wakeAll() {
  for (const w of Object.values(wakers)) w();
}

document.addEventListener("visibilitychange", () => {
  if (!document.hidden) wakeAll();
});
