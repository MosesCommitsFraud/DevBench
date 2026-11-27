// Turns the raw error text of the backend ("connecting to devbench:22: Eine vorhandene Verbindung
// wurde vom Remotehost geschlossen. (os error 10054)") into a short sentence of what happened and
// what to do, with the raw text kept as a detail. Windows reports socket errors in the system's
// language, so they are recognised by their number as well.
import { t, type Key } from "./i18n.svelte";

export interface Explained {
  /** What happened, in plain words. */
  title: string;
  /** What to try, if there is something; may be empty. */
  hint: string;
  /** The raw message, for the curious and for bug reports. */
  detail: string;
  /** Whether the machine is evidently switched on (it answered somehow); null = cannot tell. */
  up: boolean | null;
}

const rules: { test: RegExp; key: string; up: boolean | null }[] = [
  { test: /host key/i, key: "hostkey", up: true },
  { test: /handshake .*timed out/i, key: "handshake", up: true },
  { test: /command timed out/i, key: "slow", up: true },
  { test: /10054|connection reset|reset by peer|vom remotehost geschlossen|forcibly closed/i, key: "reset", up: true },
  { test: /10053|connection abort|softwaregesteuert|software caused/i, key: "reset", up: true },
  { test: /10061|connection refused|verweigert/i, key: "refused", up: true },
  { test: /ssh login as .* failed|authentication failed|permission denied/i, key: "auth", up: true },
  { test: /sudo asks for a password/i, key: "sudo", up: true },
  { test: /11001|11004|failed to lookup|no such host|name or service not known|host ist unbekannt|nodename nor servname/i, key: "dns", up: null },
  { test: /10065|10051|no route to host|network is unreachable|host is unreachable|nicht erreichbar/i, key: "unreachable", up: false },
  { test: /did not answer \(timeout\)|10060|timed out|zeitspanne/i, key: "timeout", up: false },
  { test: /disconnected|channel .*closed|connection closed|eof|broken pipe|10058/i, key: "closed", up: true },
];

export function explain(message: string): Explained {
  const detail = message.trim();
  for (const r of rules) {
    if (r.test.test(detail)) {
      return { title: t(`err.${r.key}` as Key), hint: t(`err.${r.key}.hint` as Key), detail, up: r.up };
    }
  }
  // Unknown: the message itself is the best title; no separate detail.
  return { title: detail, hint: "", detail: "", up: null };
}
