import { useEffect, useState } from "react";
import {
  FlaskConical, FolderGit2, GitBranch, GitCommitHorizontal, ScrollText, Server, Settings2, ShieldCheck, Sparkles,
} from "lucide-react";
import { call, isMock } from "./ipc";
import { Badge, Button, ICON_MD, Modal, ToastProvider } from "./ui";
import type { RepoRef } from "./types";
import ReposPage from "./pages/Repos";
import AnalyzePage from "./pages/Analyze";
import CiPage from "./pages/Ci";
import SkillsPage from "./pages/Skills";
import SettingsPage from "./pages/Settings";
import AuditPage from "./pages/Audit";

type Tab = "repos" | "analyze" | "ci" | "skills" | "settings" | "audit";

const tabs: { id: Tab; label: string; icon: typeof FolderGit2 }[] = [
  { id: "repos", label: "Dépôts", icon: FolderGit2 },
  { id: "analyze", label: "Analyse & plan", icon: GitCommitHorizontal },
  { id: "ci", label: "CI/CD", icon: Server },
  { id: "skills", label: "Skills", icon: Sparkles },
  { id: "settings", label: "Réglages", icon: Settings2 },
  { id: "audit", label: "Journal", icon: ScrollText },
];

export default function App() {
  const [tab, setTab] = useState<Tab>("repos");
  const [repos, setRepos] = useState<RepoRef[]>([]);
  const [selected, setSelected] = useState<RepoRef | null>(null);
  const [onboarding, setOnboarding] = useState(false);

  const refreshRepos = async () => {
    const list = await call<RepoRef[]>("repos_list");
    setRepos(list);
    if (selected && !list.some((r) => r.id === selected.id)) setSelected(null);
    if (!selected && list.length > 0) setSelected(list[0]);
  };

  useEffect(() => {
    void refreshRepos().then(() => {
      const forced = new URLSearchParams(window.location.search).has("onboarding");
      if (forced || !localStorage.getItem("mc:onboarded")) setOnboarding(true);
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const closeOnboarding = () => {
    localStorage.setItem("mc:onboarded", "1");
    setOnboarding(false);
  };

  return (
    <ToastProvider>
      <div className="flex h-screen flex-col">
        {/* En-tête contextuel global : le dépôt et sa gouvernance, visibles partout. */}
        <header className="flex items-center gap-3 border-b border-slate-800 bg-slate-950 px-4 py-2">
          <span className="text-sm font-bold text-teal-400">mister-commitia</span>
          {selected ? (
            <span className="flex items-center gap-2 text-xs text-slate-400">
              <GitBranch size={ICON_MD} className="text-slate-500" />
              <span className="font-medium text-slate-200">{selected.name}</span>
              <span>· défaut&nbsp;: <code>{selected.default_branch ?? "?"}</code></span>
              {selected.governance.ai_attribution_policy === "keep-required" ? (
                <Badge tone="sky">traçabilité IA exigée</Badge>
              ) : (
                <Badge tone="violet">normalisation autorisée</Badge>
              )}
            </span>
          ) : (
            <span className="text-xs text-slate-500">aucun dépôt sélectionné</span>
          )}
          <span className="ml-auto flex items-center gap-1.5 text-[11px] text-slate-600">
            <ShieldCheck size={ICON_MD} className="text-slate-600" />
            dry-run &amp; backup obligatoires
          </span>
        </header>

        <div className="flex min-h-0 flex-1">
          <aside className="flex w-56 shrink-0 flex-col border-r border-slate-800 bg-slate-950">
            <nav className="flex-1 space-y-0.5 p-2" aria-label="Navigation principale">
              {tabs.map(({ id, label, icon: Icon }) => (
                <button
                  key={id}
                  onClick={() => setTab(id)}
                  aria-current={tab === id ? "page" : undefined}
                  className={`flex w-full items-center gap-2.5 rounded px-3 py-2 text-sm transition ${
                    tab === id
                      ? "bg-teal-900/40 font-semibold text-teal-300"
                      : "text-slate-400 hover:bg-slate-900 hover:text-slate-200"
                  }`}
                >
                  <Icon size={ICON_MD} /> {label}
                </button>
              ))}
            </nav>
            {isMock && (
              <div className="m-2 flex items-center gap-2 rounded border border-violet-800 bg-violet-950/40 px-2.5 py-2 text-[11px] text-violet-300">
                <FlaskConical size={ICON_MD} className="shrink-0" />
                Mode démonstration navigateur — données factices, aucun dépôt réel.
              </div>
            )}
            <div className="border-t border-slate-800 px-4 py-2 text-[11px] text-slate-600">
              MVP 0.1.0 — lot 2
            </div>
          </aside>

          {/* Les pages restent montées (état conservé entre onglets, U6). */}
          <main className="flex-1 overflow-y-auto p-5">
            <div className={tab === "repos" ? "" : "hidden"}>
              <ReposPage
                repos={repos}
                selected={selected}
                onSelect={(r) => {
                  setSelected(r);
                  setTab("analyze");
                }}
                onChanged={refreshRepos}
              />
            </div>
            <div className={tab === "analyze" ? "" : "hidden"}>
              {selected ? (
                <AnalyzePage key={selected.id} repo={selected} />
              ) : (
                <p className="text-sm text-slate-400">Déclarer et sélectionner un dépôt d'abord.</p>
              )}
            </div>
            <div className={tab === "ci" ? "" : "hidden"}>
              <CiPage />
            </div>
            <div className={tab === "skills" ? "" : "hidden"}>
              <SkillsPage />
            </div>
            <div className={tab === "settings" ? "" : "hidden"}>
              <SettingsPage repos={repos} onChanged={refreshRepos} />
            </div>
            <div className={tab === "audit" ? "" : "hidden"}>
              <AuditPage />
            </div>
          </main>
        </div>
      </div>

      {onboarding && (
        <Modal
          title="Bienvenue dans mister-commitia"
          tone="sky"
          width={620}
          onClose={closeOnboarding}
          footer={
            <>
              <Button onClick={closeOnboarding}>Plus tard</Button>
              <Button
                kind="primary"
                onClick={() => {
                  closeOnboarding();
                  setTab("repos");
                }}
              >
                Déclarer un dépôt
              </Button>
            </>
          }
        >
          <ol className="list-inside list-decimal space-y-2 text-sm text-slate-300">
            <li>
              <b>Déclarer un dépôt Git local</b> — l'analyse est 100&nbsp;% locale (mode offline),
              rien ne sort de votre poste sans consentement explicite.
            </li>
            <li>
              <b>Choisir l'assistance IA</b> (Réglages)&nbsp;: assistant local déterministe par défaut,
              Ollama en local, ou endpoint d'entreprise — l'IA <i>propose</i>, vous <i>disposez</i>.
            </li>
            <li>
              <b>Comprendre les garde-fous</b>&nbsp;: branches protégées bloquées, dry-run obligatoire,
              backup automatique avant toute écriture, rollback en un clic, journal d'audit local.
            </li>
          </ol>
          <p className="mt-3 text-xs text-slate-400">
            Côté CI/CD&nbsp;: inventaire, politique de rétention, <b>simulation obligatoire</b> avant
            toute suppression — les runs sous retention lease ne sont jamais touchés.
          </p>
        </Modal>
      )}
    </ToastProvider>
  );
}
