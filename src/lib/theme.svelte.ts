// Light and dark theme, both built from the logo's colours. Follows the system unless chosen.

export type ThemeChoice = "system" | "light" | "dark";

const media = window.matchMedia("(prefers-color-scheme: dark)");

let choice = $state<ThemeChoice>((localStorage.getItem("devbench.theme") as ThemeChoice | null) ?? "system");
let systemDark = $state(media.matches);

export function themeChoice(): ThemeChoice {
  return choice;
}

/** The theme in effect right now. */
export function theme(): "light" | "dark" {
  return choice === "system" ? (systemDark ? "dark" : "light") : choice;
}

function apply() {
  const root = document.documentElement;
  root.dataset.theme = theme();
  root.style.colorScheme = theme();
}

export function setTheme(v: ThemeChoice) {
  choice = v;
  localStorage.setItem("devbench.theme", v);
  apply();
}

/** Flips between light and dark (leaving "system"). */
export function toggleTheme() {
  setTheme(theme() === "dark" ? "light" : "dark");
}

media.addEventListener("change", (e) => {
  systemDark = e.matches;
  apply();
});

apply();

/** Reads a design token (CSS custom property) of the current theme. */
export function token(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}
