// Every machine and VM gets a colour and a monogram, so it is recognisable at a glance in the
// sidebar, on the dashboard and in the file browser. The colour can be chosen in Settings;
// left empty, it follows from the name.

export interface Hue {
  /** Stored in the config. */
  key: string;
  /** The tile's colour in the light and in the dark theme. */
  light: string;
  dark: string;
}

export const HUES: Hue[] = [
  { key: "blue", light: "#2457e6", dark: "#5b86f5" },
  { key: "teal", light: "#0f9d8a", dark: "#2dd4bf" },
  { key: "violet", light: "#7c4ddb", dark: "#a78bfa" },
  { key: "pink", light: "#d6457f", dark: "#f472b6" },
  { key: "orange", light: "#e0701a", dark: "#fb923c" },
  { key: "amber", light: "#c48a06", dark: "#fbbf24" },
  { key: "green", light: "#2f9e44", dark: "#4ade80" },
  { key: "sky", light: "#0b84c6", dark: "#38bdf8" },
  { key: "red", light: "#d6453d", dark: "#f87171" },
  { key: "slate", light: "#5b6478", dark: "#94a3b8" },
];

export function hue(name: string, chosen = ""): Hue {
  const picked = HUES.find((h) => h.key === chosen);
  if (picked) return picked;
  const seed = name.trim().toLowerCase() || "devbench";
  let i = 0;
  for (const ch of seed) i = (i * 31 + (ch.codePointAt(0) ?? 0)) % HUES.length;
  return HUES[i];
}

/** "Windows-Test-VM" -> "WT", "Devbench" -> "D", "nas" -> "N". */
export function monogram(name: string): string {
  const words = name
    .replace(/([a-z])([A-Z])/g, "$1 $2")
    .split(/[^\p{L}\p{N}]+/u)
    .filter(Boolean);
  if (!words.length) return "?";
  if (words.length === 1) return words[0][0].toUpperCase();
  return (words[0][0] + words[1][0]).toUpperCase();
}
