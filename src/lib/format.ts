import { lang } from "./i18n.svelte";

export function bytes(n: number): string {
  if (!n) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let i = 0;
  let v = n;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(v >= 100 || i === 0 ? 0 : 1)} ${units[i]}`;
}

/** Bytes per second, e.g. "1.2 MB/s". */
export function rate(n: number): string {
  if (n < 1024) return `${Math.round(n)} B/s`;
  return `${bytes(n)}/s`;
}

/** Whole gigabytes for sizes like a VM's memory: "16 GB", "0.5 GB". */
export function gb(n: number): string {
  const v = n / 1024 ** 3;
  return `${v >= 1 ? Math.round(v) : v.toFixed(1)} GB`;
}

export function duration(secs: number): string {
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const day = lang() === "de" ? "T" : "d";
  if (d > 0) return `${d} ${day} ${h} h`;
  if (h > 0) return `${h} h ${m} min`;
  return `${m} min`;
}

export function percent(v: number): string {
  return `${Math.round(v)} %`;
}

export function time(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleString(lang() === "de" ? "de-DE" : "en-GB", {
    day: "2-digit",
    month: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}
