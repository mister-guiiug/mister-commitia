// Internationalisation FR/EN (F11). Scaffold : `t(clé)` + dictionnaire, langue
// persistée, bascule réactive. Couvre pour l'instant le shell (en-tête,
// navigation, onboarding, contrôles) ; les corps de page restent à externaliser
// (suivi). La langue par défaut est le français.

import { useEffect, useState } from "react";

export type Lang = "fr" | "en";
const KEY = "mc:lang";
const listeners = new Set<() => void>();

type Entry = { fr: string; en: string };

const STRINGS: Record<string, Entry> = {
  "app.tagline": { fr: "réécriture Git gouvernée", en: "governed Git rewriting" },
  "nav.repos": { fr: "Dépôts", en: "Repositories" },
  "nav.analyze": { fr: "Analyse & plan", en: "Analyze & plan" },
  "nav.ci": { fr: "CI/CD", en: "CI/CD" },
  "nav.skills": { fr: "Skills", en: "Skills" },
  "nav.settings": { fr: "Réglages", en: "Settings" },
  "nav.audit": { fr: "Journal", en: "Audit log" },
  "nav.aria": { fr: "Navigation principale", en: "Main navigation" },
  "header.default": { fr: "défaut", en: "default" },
  "header.noRepo": { fr: "aucun dépôt sélectionné", en: "no repository selected" },
  "header.aiRequired": { fr: "traçabilité IA exigée", en: "AI attribution required" },
  "header.aiAllowed": { fr: "normalisation autorisée", en: "normalization allowed" },
  "header.guardrails": { fr: "dry-run & backup obligatoires", en: "dry-run & backup mandatory" },
  "mock.banner": {
    fr: "Mode démonstration navigateur — données factices, aucun dépôt réel.",
    en: "Browser demo mode — fake data, no real repository.",
  },
  "controls.theme": { fr: "Thème", en: "Theme" },
  "controls.lang": { fr: "Langue", en: "Language" },
  "controls.shortcuts": { fr: "Raccourcis (?)", en: "Shortcuts (?)" },
  "shortcuts.title": { fr: "Raccourcis clavier", en: "Keyboard shortcuts" },
  "shortcuts.tabs": { fr: "Changer d'onglet", en: "Switch tab" },
  "shortcuts.help": { fr: "Afficher / masquer cette aide", en: "Toggle this help" },
  "shortcuts.close": { fr: "Fermer les fenêtres", en: "Close dialogs" },
  "shortcuts.hint": {
    fr: "Astuce : appuyez sur ? à tout moment (hors champ de saisie).",
    en: "Tip: press ? anytime (outside input fields).",
  },
  "theme.dark": { fr: "Sombre", en: "Dark" },
  "theme.light": { fr: "Clair", en: "Light" },
  "theme.system": { fr: "Système", en: "System" },
  "onboarding.title": { fr: "Bienvenue dans mister-commitia", en: "Welcome to mister-commitia" },
  "onboarding.later": { fr: "Plus tard", en: "Later" },
  "onboarding.declare": { fr: "Déclarer un dépôt", en: "Declare a repository" },
  "onboarding.step1.b": { fr: "Déclarer un dépôt Git local", en: "Declare a local Git repository" },
  "onboarding.step1.t": {
    fr: " — l'analyse est 100 % locale (mode offline), rien ne sort de votre poste sans consentement explicite.",
    en: " — analysis is 100% local (offline), nothing leaves your machine without explicit consent.",
  },
  "onboarding.step2.b": { fr: "Choisir l'assistance IA", en: "Choose AI assistance" },
  "onboarding.step2.t": {
    fr: " (Réglages) : assistant local déterministe par défaut, Ollama en local, ou endpoint d'entreprise — l'IA propose, vous disposez.",
    en: " (Settings): deterministic local assistant by default, local Ollama, or an enterprise endpoint — the AI proposes, you decide.",
  },
  "onboarding.step3.b": { fr: "Comprendre les garde-fous", en: "Understand the guardrails" },
  "onboarding.step3.t": {
    fr: " : branches protégées bloquées, dry-run obligatoire, backup automatique avant toute écriture, rollback en un clic, journal d'audit local.",
    en: ": protected branches blocked, dry-run mandatory, automatic backup before any write, one-click rollback, local audit log.",
  },
  "onboarding.ci": {
    fr: "Côté CI/CD : inventaire, politique de rétention, simulation obligatoire avant toute suppression — les runs sous retention lease ne sont jamais touchés.",
    en: "For CI/CD: inventory, retention policy, mandatory simulation before any deletion — runs under a retention lease are never touched.",
  },
};

let current: Lang = (localStorage.getItem(KEY) as Lang) || "fr";

export function getLang(): Lang {
  return current;
}

export function setLang(l: Lang): void {
  current = l;
  localStorage.setItem(KEY, l);
  document.documentElement.lang = l;
  listeners.forEach((f) => f());
}

export function t(key: string): string {
  const e = STRINGS[key];
  return e ? e[current] : key;
}

/// Abonne un composant aux changements de langue et retourne la langue courante.
export function useLang(): Lang {
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
