import { useSyncExternalStore } from "react";

/**
 * Tema da UI. Mesmo padrão de store do `i18n.ts` (useSyncExternalStore) pra
 * poder ler/trocar fora de componente. Módulo dedicado (e não o `App.tsx`) pra
 * evitar ciclo de import: `App.tsx` já importa `lib/*`.
 *
 * `system` = segue o SO: REMOVE o `data-theme` do <html> e deixa o
 * `@media (prefers-color-scheme: dark)` do App.css decidir. Qualquer outro
 * valor vira `data-theme="<nome>"` e o bloco correspondente do CSS assume.
 *
 * Os temas nomeados são paletas FIXAS (não seguem claro/escuro do SO) e
 * SOBREPÕEM o accent do chrome. As cores por app/categoria do `catalog.ts`
 * são dados do catálogo e continuam intactas.
 */

export type Theme =
  | "system"
  | "light"
  | "dark"
  | "nature"
  | "darkblue"
  | "calmgreen"
  | "pastelpink"
  | "punkprincess";

/** Ordem de exibição no seletor. */
export const THEMES: readonly Theme[] = [
  "system",
  "light",
  "dark",
  "nature",
  "darkblue",
  "calmgreen",
  "pastelpink",
  "punkprincess",
] as const;

const THEME_KEY = "taylorhub.theme";

function isTheme(v: unknown): v is Theme {
  return typeof v === "string" && (THEMES as readonly string[]).includes(v);
}

function loadTheme(): Theme {
  const v = typeof localStorage !== "undefined" ? localStorage.getItem(THEME_KEY) : null;
  return isTheme(v) ? v : "system";
}

/** Reflete o tema no <html> (`system` cai no media query do CSS). */
export function applyTheme(theme: Theme) {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  if (theme === "system") root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", theme);
}

let current: Theme = loadTheme();
const listeners = new Set<() => void>();

export function getTheme(): Theme {
  return current;
}

export function setTheme(theme: Theme) {
  if (theme === current) return;
  current = theme;
  applyTheme(theme);
  try {
    localStorage.setItem(THEME_KEY, theme);
  } catch {
    /* localStorage indisponível */
  }
  for (const l of listeners) l();
}

function subscribe(l: () => void) {
  listeners.add(l);
  return () => listeners.delete(l);
}

/** Inscreve o componente nas trocas de tema. */
export function useTheme(): Theme {
  return useSyncExternalStore(subscribe, getTheme);
}
