// What running guests report about themselves (system, RAM, disk, GPU), fetched over the
// guest's SSH while the dashboard shows them. PowerShell takes a few seconds to start, so
// guests are asked one after another, every 15 seconds.
import { api, asAppError, type GuestStats } from "./api";
import { store } from "./store.svelte";

export const guests = $state<Record<string, { stats: GuestStats | null; error: string | null }>>({});

const EVERY = 15000;
let enabled = false;
let running = false;

function targets(): [string, string][] {
  const out: [string, string][] = [];
  for (const m of store.config.machines) {
    const st = store.live[m.id]?.status;
    if (!st?.online) continue;
    for (const v of m.vms) {
      if (v.guest_ssh_user && st.vms.find((x) => x.id === v.id)?.running) out.push([m.id, v.id]);
    }
  }
  return out;
}

async function loop() {
  if (running) return;
  running = true;
  try {
    while (enabled) {
      const list = targets();
      for (const [m, v] of list) {
        if (!enabled) break;
        const key = `${m}/${v}`;
        try {
          guests[key] = { stats: await api.guestStats(m, v), error: null };
        } catch (e) {
          guests[key] = { stats: guests[key]?.stats ?? null, error: asAppError(e).message };
        }
      }
      // Nothing to ask yet (statuses still loading): look again soon
      await new Promise((r) => setTimeout(r, list.length ? EVERY : 2000));
    }
  } finally {
    running = false;
  }
}

export function wantGuests(on: boolean) {
  enabled = on;
  if (on) void loop();
}
