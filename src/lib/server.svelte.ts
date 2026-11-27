// What each server has of what DevBench needs, and how many updates wait there. Checked when
// a machine comes online, every 30 minutes after that, and after a setup step ran.
import { untrack } from "svelte";
import { api, asAppError, type ServerCheck } from "./api";
import { store } from "./store.svelte";

export const servers = $state<Record<string, { check: ServerCheck | null; error: string | null; loading: boolean; at: number }>>({});

const EVERY = 30 * 60 * 1000;

// One check per server at a time: a second call while one runs gets the running one.
const inFlight: Record<string, Promise<void>> = {};

/** Checks a server. Safe to call from an $effect: reading and writing `servers` is untracked
 *  here, so the caller's effect does not re-run (and check again) each time the result lands. */
export function checkServer(machine: string): Promise<void> {
  return (inFlight[machine] ??= untrack(() => runCheck(machine)).finally(() => delete inFlight[machine]));
}

async function runCheck(machine: string) {
  const prev = servers[machine];
  servers[machine] = { check: prev?.check ?? null, error: null, loading: true, at: prev?.at ?? 0 };
  try {
    servers[machine] = { check: await api.serverCheck(machine), error: null, loading: false, at: Date.now() };
  } catch (e) {
    servers[machine] = { check: prev?.check ?? null, error: asAppError(e).message, loading: false, at: Date.now() };
  }
}

export function startServerChecks() {
  const tick = () => {
    for (const m of store.config.machines) {
      if (!store.live[m.id]?.status?.online) continue;
      const s = servers[m.id];
      if (!s || (!s.loading && Date.now() - s.at > EVERY)) void checkServer(m.id);
    }
  };
  setInterval(tick, 10000);
  setTimeout(tick, 3000);
}

/** Updates waiting on a machine (packages and snaps), 0 if unknown. */
export function pendingUpdates(machine: string): number {
  const c = servers[machine]?.check;
  return c ? c.updates + c.snap_updates : 0;
}
