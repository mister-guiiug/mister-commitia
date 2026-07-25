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

  // -- Réglages (Settings) ---------------------------------------------------
  "set.ai.title": {
    fr: "Fournisseur IA (propositions uniquement — jamais d'action automatique)",
    en: "AI provider (proposals only — never any automatic action)",
  },
  "set.ai.type": { fr: "Type", en: "Type" },
  "set.ai.rule": { fr: "Assistant local déterministe (sans LLM)", en: "Deterministic local assistant (no LLM)" },
  "set.ai.ollama": { fr: "Ollama (LLM local)", en: "Ollama (local LLM)" },
  "set.ai.compat": { fr: "Endpoint d'entreprise (compatible OpenAI)", en: "Enterprise endpoint (OpenAI-compatible)" },
  "set.ai.anthropic": { fr: "Anthropic", en: "Anthropic" },
  "set.ai.baseUrl": { fr: "URL de base", en: "Base URL" },
  "set.ai.model": { fr: "Modèle", en: "Model" },
  "set.ai.key": { fr: "Clé d'API (coffre OS)", en: "API key (OS vault)" },
  "set.ai.setDefault": { fr: "Définir comme fournisseur par défaut", en: "Set as default provider" },
  "set.ai.hint": {
    fr: "Sans fournisseur configuré, l'assistant local déterministe est utilisé (100 % hors-ligne). Tout envoi à un fournisseur distant exige un consentement explicite avec aperçu des données.",
    en: "With no provider configured, the deterministic local assistant is used (100% offline). Any send to a remote provider requires explicit consent with a data preview.",
  },
  "set.ai.default": { fr: "défaut", en: "default" },
  "set.ai.secondary": { fr: "secondaire", en: "secondary" },
  "set.ai.keyVault": { fr: "clé au coffre", en: "key in vault" },
  "set.ai.noKey": { fr: "sans clé", en: "no key" },
  "set.ai.remove": { fr: "retirer", en: "remove" },
  "set.gov.title": { fr: "Gouvernance par dépôt", en: "Per-repository governance" },
  "set.gov.declareFirst": { fr: "Déclarer un dépôt d'abord.", en: "Declare a repository first." },
  "set.gov.repo": { fr: "Dépôt", en: "Repository" },
  "set.gov.policy": { fr: "Politique d'attribution IA", en: "AI attribution policy" },
  "set.gov.keepRequired": {
    fr: "keep-required — traçabilité IA exigée : la skill de nettoyage REFUSE (défaut)",
    en: "keep-required — AI attribution required: the cleanup skill REFUSES (default)",
  },
  "set.gov.normAllowed": {
    fr: "normalization-allowed — normalisation des mentions autorisée",
    en: "normalization-allowed — mention normalization permitted",
  },
  "set.gov.protectedBranches": { fr: "Branches protégées (virgules)", en: "Protected branches (comma-separated)" },
  "set.gov.protectedTrailers": {
    fr: "Trailers protégés — jamais supprimés (virgules)",
    en: "Protected trailers — never removed (comma-separated)",
  },
  "set.gov.note": {
    fr: "La normalisation des messages est soumise à ces règles : si la politique du dépôt impose la traçabilité des contributions assistées, l'application refuse la suppression et l'explique. Chaque normalisation appliquée est journalisée avec le contenu retiré.",
    en: "Message normalization is subject to these rules: if the repository policy mandates traceability of assisted contributions, the app refuses removal and explains why. Every applied normalization is logged with the removed content.",
  },
  "set.gov.resign": { fr: "Re-signer les commits réécrits", en: "Re-sign rewritten commits" },
  "set.gov.resignHint": {
    fr: "Si une clé de signature est configurée dans le dépôt (user.signingkey + gpg.format), les commits produits par la réécriture sont re-signés — sinon la réécriture perd les signatures.",
    en: "If a signing key is configured in the repository (user.signingkey + gpg.format), commits produced by the rewrite are re-signed — otherwise rewriting drops signatures.",
  },
  "set.gov.save": { fr: "Enregistrer la gouvernance", en: "Save governance" },

  // -- Dépôts (Repos) --------------------------------------------------------
  "repo.declared.title": { fr: "Dépôts déclarés", en: "Declared repositories" },
  "repo.add": { fr: "Déclarer un dépôt local", en: "Declare a local repository" },
  "repo.empty": {
    fr: "Aucun dépôt déclaré. L'analyse est 100 % locale : rien n'est transmis sans consentement.",
    en: "No repository declared. Analysis is 100% local: nothing is sent without consent.",
  },
  "repo.default": { fr: "défaut", en: "default" },
  "repo.protected": { fr: "protégée(s)", en: "protected" },
  "repo.aiRequired": { fr: "traçabilité IA exigée", en: "AI attribution required" },
  "repo.aiAllowed": { fr: "normalisation autorisée", en: "normalization allowed" },
  "repo.analyze": { fr: "Analyser", en: "Analyze" },
  "repo.remove": { fr: "Retirer", en: "Remove" },
  "repo.neverScanned": { fr: "jamais analysé", en: "never scanned" },
  "repo.lastScan": { fr: "dernière analyse", en: "last scan" },

  // -- Journal (Audit) -------------------------------------------------------
  "au.title": { fr: "Journal d'audit (append-only, local)", en: "Audit log (append-only, local)" },
  "au.refresh": { fr: "Actualiser", en: "Refresh" },
  "au.export": { fr: "Exporter (JSONL)", en: "Export (JSONL)" },
  "au.empty": { fr: "Aucun événement journalisé pour l'instant.", en: "No events logged yet." },
  "au.col.seq": { fr: "#", en: "#" },
  "au.col.ts": { fr: "Horodatage", en: "Timestamp" },
  "au.col.category": { fr: "Catégorie", en: "Category" },
  "au.col.action": { fr: "Action", en: "Action" },
  "au.col.target": { fr: "Cible", en: "Target" },
  "au.col.result": { fr: "Résultat", en: "Result" },
  "au.note": {
    fr: "Journalisation AVANT chaque suppression ; les tokens ne sont jamais écrits en clair.",
    en: "Logging happens BEFORE each deletion; tokens are never written in clear text.",
  },
  "au.sortBy": { fr: "Trier par", en: "Sort by" },
  "au.exported": { fr: "Journal exporté (JSONL chronologique)", en: "Audit log exported (chronological JSONL)" },

  // -- Toasts communs / divers ----------------------------------------------
  "common.cancel": { fr: "Annuler", en: "Cancel" },
  "set.ai.saved": { fr: "Fournisseur IA enregistré comme défaut", en: "AI provider set as default" },
  "set.ai.savedKey": { fr: " — clé envoyée au coffre", en: " — key sent to the vault" },
  "set.ai.removed": { fr: "Fournisseur retiré (clé purgée du coffre)", en: "Provider removed (key purged from vault)" },
  "set.gov.saved": { fr: "Gouvernance de « {n} » enregistrée", en: "Governance for « {n} » saved" },

  // -- Dépôts (Repos), suite -------------------------------------------------
  "repo.selected": { fr: "sélectionné", en: "selected" },
  "repo.noRemote": { fr: "sans remote", en: "no remote" },
  "repo.defaultBranch": { fr: "Branche par défaut", en: "Default branch" },
  "repo.protectedLabel": { fr: "protégées", en: "protected" },
  "repo.guardrails": {
    fr: "Garde-fous actifs : branches protégées bloquées · dry-run obligatoire · backup automatique avant application · aucune action IA automatique · secrets au coffre du système.",
    en: "Active guardrails: protected branches blocked · mandatory dry-run · automatic backup before applying · no automatic AI action · secrets in the OS vault.",
  },
  "repo.removeTitle": { fr: "Retirer « {n} » du workspace ?", en: "Remove « {n} » from the workspace?" },
  "repo.removeBody": {
    fr: "Seules les métadonnées locales (analyses, plans, propositions) sont concernées : le dépôt Git sur disque n'est pas touché.",
    en: "Only local metadata (analyses, plans, proposals) is affected: the on-disk Git repository is left untouched.",
  },
  "repo.declared": { fr: "Dépôt « {n} » déclaré — analyse locale disponible", en: "Repository « {n} » declared — local analysis available" },
  "repo.removed": { fr: "« {n} » retiré du workspace (le dépôt Git est intact)", en: "« {n} » removed from the workspace (the Git repo is intact)" },

  // -- Skills ----------------------------------------------------------------
  "sk.empty": { fr: "Aucune skill chargée.", en: "No skill loaded." },
  "sk.ignored": { fr: "Skills ignorées : {x}", en: "Skills ignored: {x}" },
  "sk.offline": { fr: "exécutable hors-ligne", en: "runnable offline" },
  "sk.edit": { fr: "Éditer", en: "Edit" },
  "sk.editManifest": { fr: "Éditer le manifeste YAML", en: "Edit the YAML manifest" },
  "sk.runTests": { fr: "Lancer les tests", en: "Run tests" },
  "sk.testsViaLlm": { fr: "tests via fournisseur LLM", en: "tests via LLM provider" },
  "sk.rules": { fr: "Règles", en: "Rules" },
  "sk.guardrails": { fr: "Garde-fous (vérifiés par l'application)", en: "Guardrails (verified by the app)" },
  "sk.editorTitle": {
    fr: "Éditer la skill « {n} » (YAML validé à l'enregistrement)",
    en: "Edit skill « {n} » (YAML validated on save)",
  },
  "sk.save": { fr: "Enregistrer", en: "Save" },
  "sk.editorAria": { fr: "Manifeste YAML de la skill", en: "Skill YAML manifest" },
  "sk.editorNote": {
    fr: "Le champ name est immuable (renommer = créer une nouvelle skill) ; chaque édition est journalisée. Relancer les tests après enregistrement.",
    en: "The name field is immutable (renaming = creating a new skill); every edit is logged. Re-run the tests after saving.",
  },
  "sk.saved": { fr: "Skill « {n} » enregistrée (édition journalisée)", en: "Skill « {n} » saved (edit logged)" },
  "sk.testsGreen": { fr: "tests verts", en: "tests passing" },

  // -- CI/CD -----------------------------------------------------------------
  "ci.acct.title": { fr: "Ajouter un accès plateforme", en: "Add a platform access" },
  "ci.platform": { fr: "Plateforme", en: "Platform" },
  "ci.gh": { fr: "GitHub.com", en: "GitHub.com" },
  "ci.ghe": { fr: "GitHub Enterprise Server", en: "GitHub Enterprise Server" },
  "ci.azdo": { fr: "Azure DevOps Services", en: "Azure DevOps Services" },
  "ci.azdoServer": { fr: "Azure DevOps Server", en: "Azure DevOps Server" },
  "ci.apiUrl": { fr: "URL de base de l'API", en: "API base URL" },
  "ci.orgOwner": { fr: "Organisation / owner", en: "Organization / owner" },
  "ci.projectAzdo": { fr: "Projet (AzDO)", en: "Project (AzDO)" },
  "ci.repoGithub": { fr: "Dépôt (GitHub)", en: "Repository (GitHub)" },
  "ci.scopesTitle": {
    fr: "Droits requis (créer un token minimal) — affichés avant l'enregistrement :",
    en: "Required rights (create a minimal token) — shown before saving:",
  },
  "ci.token": {
    fr: "Token (stocké au coffre du système, jamais en clair)",
    en: "Token (stored in the OS vault, never in clear text)",
  },
  "ci.validateSave": { fr: "Valider & enregistrer", en: "Validate & save" },
  "ci.policy.title": { fr: "Politique de rétention", en: "Retention policy" },
  "ci.name": { fr: "Nom", en: "Name" },
  "ci.maxAge": { fr: "Âge max (jours)", en: "Max age (days)" },
  "ci.keepLast": { fr: "Conserver les N derniers / pipeline", en: "Keep the last N / pipeline" },
  "ci.protectedBranches": { fr: "Branches protégées (séparées par des virgules)", en: "Protected branches (comma-separated)" },
  "ci.alwaysProtected": {
    fr: "Toujours protégés, non désactivable : runs en cours, runs sous retention lease (Azure DevOps).",
    en: "Always protected, non-disableable: running runs, runs under a retention lease (Azure DevOps).",
  },
  "ci.savePolicy": { fr: "Enregistrer la politique", en: "Save policy" },
  "ci.invSim.title": { fr: "Inventaire & simulation", en: "Inventory & simulation" },
  "ci.acctAria": { fr: "Compte plateforme", en: "Platform account" },
  "ci.policyAria": { fr: "Politique de rétention", en: "Retention policy" },
  "ci.policyPlaceholder": { fr: "— politique —", en: "— policy —" },
  "ci.inventory": { fr: "Inventorier", en: "Inventory" },
  "ci.simulate": { fr: "Simuler (aucune suppression)", en: "Simulate (no deletion)" },
  "ci.total": { fr: "runs au total", en: "runs total" },
  "ci.kept": { fr: "conservés (âge / N derniers)", en: "kept (age / last N)" },
  "ci.protectedN": { fr: "protégés", en: "protected" },
  "ci.candidatesN": { fr: "candidats à suppression", en: "deletion candidates" },
  "ci.deleteAll": { fr: "Tout supprimer", en: "Delete all" },
  "ci.resume": { fr: "Reprendre", en: "Resume" },
  "ci.protectedHead": { fr: "Protégés (jamais supprimés)", en: "Protected (never deleted)" },
  "ci.candidatesHead": {
    fr: "Candidats (suppression unitaire, confirmation renforcée)",
    en: "Candidates (single deletion, reinforced confirmation)",
  },
  "ci.date": { fr: "date", en: "date" },
  "ci.noCandidates": { fr: "Aucun candidat selon cette politique.", en: "No candidate under this policy." },
  "ci.invSimEmpty": {
    fr: "Inventorier puis simuler : le rapport distingue candidats et protégés avec motifs.",
    en: "Inventory then simulate: the report distinguishes candidates from protected runs with reasons.",
  },
  "ci.reclaim": {
    fr: "Reclaim de stockage : purge des logs et artefacts en conservant les runs ({n} run(s) éligible(s) ; runs en cours ignorés). GitHub uniquement.",
    en: "Storage reclaim: purge logs and artifacts while keeping the runs ({n} eligible run(s); running runs ignored). GitHub only.",
  },
  "ci.purgeBtn": { fr: "Purger logs + artefacts", en: "Purge logs + artifacts" },
  "ci.addAccess": { fr: "Ajouter un accès", en: "Add an access" },
  "ci.noAccount": { fr: "Aucun compte plateforme déclaré.", en: "No platform account declared." },
  "ci.batchTitle": { fr: "Suppression en masse des candidats", en: "Bulk deletion of candidates" },
  "ci.deleteTitle": { fr: "Suppression définitive d'un run", en: "Permanent deletion of a run" },
  "ci.deleteConfirm": { fr: "Supprimer ce run", en: "Delete this run" },
  "ci.purgeTitle": { fr: "Purge des logs et artefacts", en: "Purge of logs and artifacts" },
  "ci.artifacts": { fr: "Artefacts", en: "Artifacts" },
  "ci.logs": { fr: "Logs", en: "Logs" },

  // -- Analyse & plan (Analyze) ---------------------------------------------
  "an.analyzing": { fr: "Analyse en cours…", en: "Analyzing…" },
  "an.branchAria": { fr: "Branche analysée", en: "Analyzed branch" },
  "an.current": { fr: "(courante)", en: "(current)" },
  "an.commits": { fr: "commits", en: "commits" },
  "an.conform": { fr: "conformes", en: "conforming" },
  "an.weak": { fr: "faibles", en: "weak" },
  "an.aiMentions": { fr: "mentions d'outils", en: "tool mentions" },
  "an.segTitle": {
    fr: "Commits du segment réécrivable (du plus ancien au plus récent)",
    en: "Commits of the rewritable segment (oldest to newest)",
  },
  "an.orderChanged": { fr: "ordre modifié → op reorder au plan", en: "order changed → reorder op in the plan" },
  "an.resetOrder": { fr: "rétablir l'ordre", en: "reset order" },
  "an.viewMode": { fr: "Mode d'affichage", en: "Display mode" },
  "an.list": { fr: "Liste", en: "List" },
  "an.graph": { fr: "Graphe", en: "Graph" },
  "an.skillAria": { fr: "Skill à utiliser", en: "Skill to use" },
  "an.skillConv": { fr: "Skill : Conventional Commits (reword)", en: "Skill: Conventional Commits (reword)" },
  "an.skillSynth": { fr: "Skill : Synthèse de groupe (squash)", en: "Skill: Group synthesis (squash)" },
  "an.skillClean": { fr: "Skill : Nettoyage des mentions (gouverné)", en: "Skill: Mention cleanup (governed)" },
  "an.propose": { fr: "Proposer", en: "Propose" },
  "an.suggestMerges": { fr: "Suggérer des fusions", en: "Suggest merges" },
  "an.suggestTitle": { fr: "Groupes suggérés par l'heuristique locale", en: "Groups suggested by the local heuristic" },
  "an.col.subject": { fr: "Sujet", en: "Subject" },
  "an.col.author": { fr: "Auteur · date", en: "Author · date" },
  "an.col.diff": { fr: "Diff", en: "Diff" },
  "an.col.signals": { fr: "Signaux", en: "Signals" },
  "an.shared": { fr: "partagé", en: "shared" },
  "an.signed": { fr: "signé", en: "signed" },
  "an.keep": { fr: "garder", en: "keep" },
  "an.drop": { fr: "abandonner", en: "drop" },
  "an.propTitle": { fr: "Propositions ({n}) — l'IA propose, vous disposez", en: "Proposals ({n}) — the AI proposes, you decide" },
  "an.propEmpty": {
    fr: "Sélectionner des commits puis « Proposer ». Sans fournisseur LLM configuré, l'assistant local déterministe est utilisé (100 % hors-ligne).",
    en: "Select commits then « Propose ». With no LLM provider configured, the deterministic local assistant is used (100% offline).",
  },
  "an.planTitle": { fr: "Plan de réécriture", en: "Rewrite plan" },
  "an.composeFrom": { fr: "Composer depuis les décisions", en: "Compose from decisions" },
  "an.dryRun": { fr: "Dry-run", en: "Dry-run" },
  "an.apply": { fr: "Appliquer", en: "Apply" },
  "an.rollback": { fr: "Rollback", en: "Rollback" },
  "an.push": { fr: "Pousser", en: "Push" },
  "an.planEmpty": {
    fr: "Accepter/éditer des propositions (et éventuellement marquer des abandons), puis composer le plan. Séquence imposée : plan → dry-run → backup automatique → application → rollback possible.",
    en: "Accept/edit proposals (and optionally mark drops), then compose the plan. Enforced sequence: plan → dry-run → automatic backup → apply → rollback possible.",
  },
  "an.st.draft": { fr: "brouillon", en: "draft" },
  "an.st.dryRunOk": { fr: "dry-run OK", en: "dry-run OK" },
  "an.st.applied": { fr: "appliqué", en: "applied" },
  "an.st.rolledBack": { fr: "restauré", en: "rolled back" },
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
