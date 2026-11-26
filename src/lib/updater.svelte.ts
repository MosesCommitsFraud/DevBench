// Updates come from the GitHub releases (latest.json, signed). On start DevBench checks once
// in the background; Settings has a button for it. See docs/releasing.md.
import { getVersion } from "@tauri-apps/api/app";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";

export type UpdateStatus = "idle" | "checking" | "current" | "available" | "downloading" | "ready" | "error";

export const updates = $state({
  version: null as string | null,
  status: "idle" as UpdateStatus,
  /** Version, notes and date of the update found. */
  available: null as { version: string; notes: string | null; date: string | null } | null,
  /** Download progress 0..1; null while the size is unknown. */
  progress: null as number | null,
  error: null as string | null,
  checkedAt: null as number | null,
});

let pending: Update | null = null;
let initialized = false;

function message(e: unknown) {
  return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
}

/** Reads the version and checks for updates once, quietly. */
export async function initUpdates() {
  if (initialized) return;
  initialized = true;
  updates.version = await getVersion().catch(() => null);
  await checkUpdates(true);
}

/** Asks GitHub for a newer version. Quiet: errors are not shown. */
export async function checkUpdates(silent = false) {
  if (updates.status === "checking" || updates.status === "downloading") return;
  updates.status = "checking";
  updates.error = null;
  try {
    pending = await check();
    updates.checkedAt = Date.now();
    if (pending) {
      updates.available = { version: pending.version, notes: pending.body ?? null, date: pending.date ?? null };
      updates.status = "available";
    } else {
      updates.available = null;
      updates.status = "current";
    }
  } catch (e) {
    updates.checkedAt = Date.now();
    updates.status = silent ? "idle" : "error";
    updates.error = silent ? null : message(e);
  }
}

/** Downloads the update found, installs it and restarts DevBench. */
export async function installUpdate() {
  if (!pending) return;
  let total = 0;
  let received = 0;
  updates.status = "downloading";
  updates.progress = null;
  updates.error = null;
  try {
    await pending.downloadAndInstall((event) => {
      if (event.event === "Started") total = event.data.contentLength ?? 0;
      else if (event.event === "Progress") {
        received += event.data.chunkLength;
        updates.progress = total > 0 ? Math.min(1, received / total) : null;
      } else if (event.event === "Finished") {
        updates.status = "ready";
        updates.progress = 1;
      }
    });
    updates.status = "ready";
    await relaunch();
  } catch (e) {
    updates.status = "error";
    updates.error = message(e);
  }
}
