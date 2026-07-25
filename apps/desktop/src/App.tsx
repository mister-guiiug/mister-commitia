import { useEffect, useState } from "react";
import {
  FlaskConical, FolderGit2, GitBranch, GitCommitHorizontal, Keyboard, Languages, Monitor, Moon,
  ScrollText, Server, Settings2, ShieldCheck, Sparkles, Sun,
} from "lucide-react";
import { call, isMock } from "./ipc";
import { t, setLang, useLang } from "./i18n";
import { setTheme, useTheme, type Theme } from "./theme";
import { Badge, Button, ICON_MD, ICON_SM, Modal, ToastProvider } from "./ui";
import type { RepoRef } from "./types";
import ReposPage from "./pages/Repos";
import AnalyzePage from "./pages/Analyze";
import CiPage from "./pages/Ci";
import SkillsPage from "./pages/Skills";
import SettingsPage from "./pages/Settings";
import AuditPage from "./pages/Audit";

type Tab = "repos" | "analyze" | "ci" | "skills" | "settings" | "audit";

const tabs: { id: Tab; key: string; icon: typeof FolderGit2 }[] = [
  { id: "repos", key: "nav.repos", icon: FolderGit2 },
  { id: "analyze", key: "nav.analyze", icon: GitCommitHorizontal },
  { id: "ci", key: "nav.ci", icon: Server },
  { id: "skills", key: "nav.skills", icon: Sparkles },
  { id: "settings", key: "nav.settings", icon: Settings2 },
  { id: "audit", key: "nav.audit", icon: ScrollText },
];

/// Contrôles globaux (U2 + F11 + U12) : thème, langue, aide des raccourcis.
function ShellControls({ onShortcuts }: { onShortcuts: () => void }) {
  const theme = useTheme();
  const lang = useLang();
  const order: Theme[] = ["dark", "light", "system"];
  const icons = { dark: Moon, light: Sun, system: Monitor } as const;
  const Icon = icons[theme];
  const ctl =
    "inline-flex items-center gap-1 rounded border border-slate-700 px-2 py-1 text-[11px] text-slate-300 hover:bg-slate-800";
  return (
    <div className="flex items-center gap-1.5">
      <button
        type="button"
        title={t("controls.theme")}
        aria-label={t("controls.theme")}
        onClick={() => setTheme(order[(order.indexOf(theme) + 1) % order.length])}
        className={ctl}
      >
        <Icon size={ICON_SM} /> {t(`theme.${theme}`)}
      </button>
      <button
        type="button"
        title={t("controls.lang")}
        aria-label={t("controls.lang")}
        onClick={() => setLang(lang === "fr" ? "en" : "fr")}
        className={ctl}
      >
        <Languages size={ICON_SM} /> {lang.toUpperCase()}
      </button>
      <button type="button" title={t("controls.shortcuts")} aria-label={t("controls.shortcuts")} onClick={onShortcuts} className={ctl}>
        <Keyboard size={ICON_SM} /> ?
      </button>
    </div>
  );
}

export default function App() {
  useLang(); // re-render sur changement de langue
  const [tab, setTab] = useState<Tab>("repos");
  const [repos, setRepos] = useState<RepoRef[]>([]);
  const [selected, setSelected] = useState<RepoRef | null>(null);
  const [onboarding, setOnboarding] = useState(false);
  const [shortcuts, setShortcuts] = useState(false);

  // U12 : raccourcis clavier globaux (ignorés dans les champs de saisie).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const el = e.target as HTMLElement | null;
      if (
        el &&
        (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.tagName === "SELECT" || el.isContentEditable)
      )
        return;
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      if (e.key === "?") {
        setShortcuts((s) => !s);
      } else if (e.key >= "1" && e.key <= String(tabs.length)) {
        setTab(tabs[Number(e.key) - 1].id);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

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
              <span>· {t("header.default")}&nbsp;: <code>{selected.default_branch ?? "?"}</code></span>
              {selected.governance.ai_attribution_policy === "keep-required" ? (
                <Badge tone="sky">{t("header.aiRequired")}</Badge>
              ) : (
                <Badge tone="violet">{t("header.aiAllowed")}</Badge>
              )}
            </span>
          ) : (
            <span className="text-xs text-slate-500">{t("header.noRepo")}</span>
          )}
          <div className="ml-auto flex items-center gap-3">
            <ShellControls onShortcuts={() => setShortcuts(true)} />
            <span className="hidden items-center gap-1.5 text-[11px] text-slate-600 lg:flex">
              <ShieldCheck size={ICON_MD} className="text-slate-600" />
              {t("header.guardrails")}
            </span>
          </div>
        </header>

        <div className="flex min-h-0 flex-1">
          <aside className="flex w-56 shrink-0 flex-col border-r border-slate-800 bg-slate-950">
            <nav className="flex-1 space-y-0.5 p-2" aria-label={t("nav.aria")}>
              {tabs.map(({ id, key, icon: Icon }) => (
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
                  <Icon size={ICON_MD} /> {t(key)}
                </button>
              ))}
            </nav>
            {isMock && (
              <div className="m-2 flex items-center gap-2 rounded border border-violet-800 bg-violet-950/40 px-2.5 py-2 text-[11px] text-violet-300">
                <FlaskConical size={ICON_MD} className="shrink-0" />
                {t("mock.banner")}
              </div>
            )}
            <div className="border-t border-slate-800 px-4 py-2 text-[11px] text-slate-600">
              MVP 0.1.0 — lot 5
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
          title={t("onboarding.title")}
          tone="sky"
          width={620}
          onClose={closeOnboarding}
          footer={
            <>
              <Button onClick={closeOnboarding}>{t("onboarding.later")}</Button>
              <Button
                kind="primary"
                onClick={() => {
                  closeOnboarding();
                  setTab("repos");
                }}
              >
                {t("onboarding.declare")}
              </Button>
            </>
          }
        >
          <ol className="list-inside list-decimal space-y-2 text-sm text-slate-300">
            <li>
              <b>{t("onboarding.step1.b")}</b>{t("onboarding.step1.t")}
            </li>
            <li>
              <b>{t("onboarding.step2.b")}</b>{t("onboarding.step2.t")}
            </li>
            <li>
              <b>{t("onboarding.step3.b")}</b>{t("onboarding.step3.t")}
            </li>
          </ol>
          <p className="mt-3 text-xs text-slate-400">{t("onboarding.ci")}</p>
        </Modal>
      )}

      {shortcuts && (
        <Modal title={t("shortcuts.title")} tone="sky" width={460} onClose={() => setShortcuts(false)}>
          <dl className="space-y-2 text-sm text-slate-300">
            {[
              ["1 – 6", t("shortcuts.tabs")],
              ["?", t("shortcuts.help")],
              ["Échap", t("shortcuts.close")],
            ].map(([k, label]) => (
              <div key={k} className="flex items-center gap-3">
                <kbd className="min-w-16 rounded border border-slate-600 bg-slate-800 px-2 py-0.5 text-center font-mono text-xs text-slate-200">
                  {k}
                </kbd>
                <span>{label}</span>
              </div>
            ))}
          </dl>
          <p className="mt-3 text-xs text-slate-500">{t("shortcuts.hint")}</p>
        </Modal>
      )}
    </ToastProvider>
  );
}
