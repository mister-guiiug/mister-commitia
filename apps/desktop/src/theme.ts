// Thème clair/sombre/système persisté (U2). Le CSS (index.css) bascule sur
// l'attribut `data-theme` de <html> ; ici on résout « système » et on persiste.

import { useEffect, useState } from "react";

export type Theme = "dark" | "light" | "system";
const KEY = "mc:theme";
const listeners = new Set<() => void>();

function systemDark(): boolean {
  return window.matchMedia?.("(prefers-color-scheme: dark)").matches ?? true;
}

function resolve(t: Theme): "dark" | "light" {
  return t === "system" ? (systemDark() ? "dark" : "light") : t;
}

let current: Theme = (localStorage.getItem(KEY) as Theme) || "dark";

/// Applique le thème résolu à l'élément racine (appelé au démarrage, sans flash).
export function applyTheme(): void {
  document.documentElement.dataset.theme = resolve(current);
}

export function getTheme(): Theme {
  return current;
}

export function setTheme(t: Theme): void {
  current = t;
  localStorage.setItem(KEY, t);
  applyTheme();
  listeners.forEach((f) => f());
}

// Suivre les changements système quand le thème est « système ».
if (typeof window !== "undefined" && window.matchMedia) {
  window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
    if (current === "system") {
      applyTheme();
      listeners.forEach((f) => f());
    }
  });
}

export function useTheme(): Theme {
  const [, force] = useState(0);
  useEffect(() => {
    const f = () => force((x) => x + 1);
    listeners.add(f);
    return () => {
      listeners.delete(f);
    };
  }, []);
  return current;
}
