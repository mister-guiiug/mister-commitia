import { useEffect, useState } from "react";
import {
  FolderGit2, GitCommitHorizontal, Server, Sparkles, Settings2, ScrollText, FlaskConical,
} from "lucide-react";
import { call, isMock } from "./ipc";
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

  const refreshRepos = async () => {
    const list = await call<RepoRef[]>("repos_list");
    setRepos(list);
    if (selected && !list.some((r) => r.id === selected.id)) setSelected(null);
    if (!selected && list.length > 0) setSelected(list[0]);
  };

  useEffect(() => {
    void refreshRepos();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <div className="flex h-screen">
      <aside className="flex w-56 shrink-0 flex-col border-r border-slate-800 bg-slate-950">
        <div className="border-b border-slate-800 px-4 py-3">
          <div className="text-base font-bold text-teal-400">mister-commitia</div>
          <div className="text-[11px] text-slate-500">
            réécriture gouvernée · rétention CI/CD
          </div>
        </div>
        <nav className="flex-1 space-y-0.5 p-2">
          {tabs.map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              onClick={() => setTab(id)}
              className={`flex w-full items-center gap-2.5 rounded px-3 py-2 text-sm transition ${
                tab === id
                  ? "bg-teal-900/40 font-semibold text-teal-300"
                  : "text-slate-400 hover:bg-slate-900 hover:text-slate-200"
              }`}
            >
              <Icon size={16} /> {label}
            </button>
          ))}
        </nav>
        {isMock && (
          <div className="m-2 flex items-center gap-2 rounded border border-violet-800 bg-violet-950/40 px-2.5 py-2 text-[11px] text-violet-300">
            <FlaskConical size={14} className="shrink-0" />
            Mode démonstration navigateur — données factices, aucun dépôt réel.
          </div>
        )}
        <div className="border-t border-slate-800 px-4 py-2 text-[11px] text-slate-600">
          MVP 0.1.0 — dry-run & backup obligatoires
        </div>
      </aside>

      <main className="flex-1 overflow-y-auto p-5">
        {tab === "repos" && (
          <ReposPage
            repos={repos}
            selected={selected}
            onSelect={(r) => { setSelected(r); setTab("analyze"); }}
            onChanged={refreshRepos}
          />
        )}
        {tab === "analyze" &&
          (selected ? (
            <AnalyzePage key={selected.id} repo={selected} />
          ) : (
            <p className="text-sm text-slate-500">Déclarer et sélectionner un dépôt d'abord.</p>
          ))}
        {tab === "ci" && <CiPage />}
        {tab === "skills" && <SkillsPage />}
        {tab === "settings" && <SettingsPage repos={repos} onChanged={refreshRepos} />}
        {tab === "audit" && <AuditPage />}
      </main>
    </div>
  );
}
