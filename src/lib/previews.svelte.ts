// Live pictures of the screens on the dashboard: every running VM with VNC, and the desktop of
// every machine that has one. One loop fetches them in turn while the dashboard is visible.
import { api } from "./api";
import { store } from "./store.svelte";

export interface Preview {
  /** PNG data URL of the last picture. */
  url: string | null;
  /** When the last attempt failed, its message (e.g. the VNC server wants a password). */
  error: string | null;
  at: number;
}

export const previews = $state<Record<string, Preview>>({});

export function previewKey(machine: string, vm: string | null) {
  return `${machine}/${vm ?? ""}`;
}

const EVERY = 5000;
const AFTER_ERROR = 60000;

let enabled = false;
let running = false;

/** The screens worth a picture right now. */
function targets(): [string, string | null][] {
  const out: [string, string | null][] = [];
  for (const m of store.config.machines) {
    const st = store.live[m.id]?.status;
    if (!st?.online) continue;
    if (m.desktop_vnc_port) out.push([m.id, null]);
    for (const v of m.vms) {
      if (v.vnc_mode !== "none" && st.vms.find((x) => x.id === v.id)?.running) out.push([m.id, v.id]);
    }
  }
  return out;
}

async function loop() {
  if (running) return;
  running = true;
  while (enabled) {
    const t0 = Date.now();
    for (const [m, v] of targets()) {
      if (!enabled) break;
      const key = previewKey(m, v);
      const p = previews[key];
      if (p && Date.now() - p.at < (p.error ? AFTER_ERROR : EVERY)) continue;
      try {
        const url = await api.screenPreview(m, v);
        previews[key] = { url, error: null, at: Date.now() };
      } catch (e) {
        previews[key] = { url: p?.url ?? null, error: (e as { message?: string }).message ?? String(e), at: Date.now() };
      }
    }
    await new Promise((r) => setTimeout(r, Math.max(500, 1500 - (Date.now() - t0))));
  }
  running = false;
}

/** Turns the pictures on (dashboard visible) or off (closes the VNC connections). */
export function setPreviews(on: boolean) {
  on = on && !document.hidden;
  if (on === enabled) return;
  enabled = on;
  if (on) void loop();
  else void api.screenPreviewStop().catch(() => {});
}

let wanted = false;
/** What the app asks for; the window being hidden switches it off on top. */
export function wantPreviews(on: boolean) {
  wanted = on;
  setPreviews(on);
}
document.addEventListener("visibilitychange", () => setPreviews(wanted));
