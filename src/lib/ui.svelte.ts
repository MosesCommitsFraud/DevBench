// App-wide UI state: notices, confirmations and the host key prompt.
import { api, asAppError, type AppError } from "./api";

export interface Notice {
  id: number;
  text: string;
  tone: "info" | "error";
}

export interface ConfirmRequest {
  title: string;
  body: string;
  action: string;
  danger: boolean;
  resolve: (ok: boolean) => void;
}

export interface PromptRequest {
  title: string;
  label: string;
  value: string;
  action: string;
  resolve: (value: string | null) => void;
}

export interface HostKeyRequest {
  error: Extract<AppError, { kind: "host_key_unknown" | "host_key_changed" }>;
  resolve: (trusted: boolean) => void;
}

export const ui = $state({
  notices: [] as Notice[],
  confirm: null as ConfirmRequest | null,
  hostKey: null as HostKeyRequest | null,
  prompt: null as PromptRequest | null,
});

/** Asks for a short text, e.g. a folder name. Resolves null when cancelled. */
export function prompt(title: string, label: string, value: string, action: string): Promise<string | null> {
  return new Promise((resolve) => {
    ui.prompt = {
      title,
      label,
      value,
      action,
      resolve: (v) => {
        ui.prompt = null;
        resolve(v && v.trim() ? v.trim() : null);
      },
    };
  });
}

let nextId = 1;

export function notify(text: string, tone: Notice["tone"] = "info") {
  const id = nextId++;
  ui.notices.push({ id, text, tone });
  setTimeout(() => dismiss(id), tone === "error" ? 9000 : 4000);
}

export function dismiss(id: number) {
  const i = ui.notices.findIndex((n) => n.id === id);
  if (i >= 0) ui.notices.splice(i, 1);
}

export function confirm(title: string, body: string, action: string, danger = true): Promise<boolean> {
  return new Promise((resolve) => {
    ui.confirm = {
      title,
      body,
      action,
      danger,
      resolve: (ok) => {
        ui.confirm = null;
        resolve(ok);
      },
    };
  });
}

let hostKeyQueue: Promise<boolean> | null = null;

/** Asks the user about an unknown or changed host key. Resolves true once trusted. */
export function askHostKey(error: HostKeyRequest["error"]): Promise<boolean> {
  if (hostKeyQueue) return hostKeyQueue;
  hostKeyQueue = new Promise<boolean>((resolve) => {
    ui.hostKey = {
      error,
      resolve: async (trusted) => {
        if (trusted && error.kind === "host_key_unknown") {
          try {
            await api.trustHostKey(error.host, error.port, error.key);
          } catch (e) {
            notify(asAppError(e).message, "error");
            trusted = false;
          }
        }
        ui.hostKey = null;
        hostKeyQueue = null;
        resolve(trusted && error.kind === "host_key_unknown");
      },
    };
  });
  return hostKeyQueue;
}

/**
 * Runs a backend call. On a host key question it asks the user and, once trusted, retries.
 * Other errors are shown as a notice and rethrown.
 */
export async function run<T>(call: () => Promise<T>, quiet = false): Promise<T> {
  for (let attempt = 0; ; attempt++) {
    try {
      return await call();
    } catch (e) {
      const err = asAppError(e);
      if (err.kind !== "error" && attempt === 0 && (await askHostKey(err))) continue;
      if (!quiet) notify(err.message, "error");
      throw err;
    }
  }
}
