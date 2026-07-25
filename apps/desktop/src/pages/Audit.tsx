import { useEffect, useMemo, useState, type ReactNode } from "react";
import { ArrowUpDown, Download, RefreshCw } from "lucide-react";
import { asIpcError, call } from "../ipc";
import { t, useLang } from "../i18n";
import type { AuditEvent } from "../types";
import { Badge, Button, Card, Empty, ErrorBox, ICON_SM, shaCls, thCls, trCls, useToast } from "../ui";

type SortKey = "seq" | "category" | "action" | "result";

const toneByCategory: Record<string, string> = {
  git_rewrite: "amber",
  ci_cleanup: "rose",
  secret: "violet",
  skill: "sky",
  config: "slate",
};

/// En-tête de colonne triable (U7) : clic pour trier, indicateur de sens.
function SortTh({
  k, sort, onSort, children,
}: {
  k: SortKey; sort: { key: SortKey; asc: boolean }; onSort: (k: SortKey) => void; children: ReactNode;
}) {
  const active = sort.key === k;
  return (
    <th className={thCls}>
      <button
        type="button"
        className="inline-flex items-center gap-1 hover:text-slate-200"
        onClick={() => onSort(k)}
        aria-label={`${t("au.sortBy")} ${typeof children === "string" ? children : k}`}
      >
        {children}
        {active ? (sort.asc ? "↑" : "↓") : <ArrowUpDown size={12} className="opacity-40" />}
      </button>
    </th>
  );
}

export default function AuditPage() {
  useLang();
  const toast = useToast();
  const [events, setEvents] = useState<AuditEvent[]>([]);
  const [sort, setSort] = useState<{ key: SortKey; asc: boolean }>({ key: "seq", asc: false });
  const [error, setError] = useState<string | null>(null);

  const toggleSort = (key: SortKey) =>
    setSort((s) => (s.key === key ? { key, asc: !s.asc } : { key, asc: true }));

  const sorted = useMemo(() => {
    const dir = sort.asc ? 1 : -1;
    return [...events].sort((a, b) => {
      if (sort.key === "seq") return (a.seq - b.seq) * dir;
      return String(a[sort.key]).localeCompare(String(b[sort.key])) * dir;
    });
  }, [events, sort]);

  const refresh = async () => {
    setError(null);
    try {
      setEvents(await call<AuditEvent[]>("audit_list", { limit: 200 }));
    } catch (e) {
      setError(asIpcError(e).message);
    }
  };

  useEffect(() => {
    void refresh();
  }, []);

  const exportJsonl = async () => {
    try {
      const content = await call<string>("audit_export");
      const blob = new Blob([content], { type: "application/jsonl" });
      const a = document.createElement("a");
      a.href = URL.createObjectURL(blob);
      a.download = "mister-commitia-audit.jsonl";
      a.click();
      URL.revokeObjectURL(a.href);
      toast("success", t("au.exported"));
    } catch (e) {
      setError(asIpcError(e).message);
    }
  };

  return (
    <div className="mx-auto max-w-5xl">
      <Card
        title={t("au.title")}
        actions={
          <>
            <Button onClick={refresh}><RefreshCw size={ICON_SM} /> {t("au.refresh")}</Button>
            <Button onClick={exportJsonl}><Download size={ICON_SM} /> {t("au.export")}</Button>
          </>
        }
      >
        <ErrorBox error={error} />
        {events.length === 0 ? (
          <Empty>{t("au.empty")}</Empty>
        ) : (
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-slate-800">
                <SortTh k="seq" sort={sort} onSort={toggleSort}>{t("au.col.seq")}</SortTh>
                <th className={thCls}>{t("au.col.ts")}</th>
                <SortTh k="category" sort={sort} onSort={toggleSort}>{t("au.col.category")}</SortTh>
                <SortTh k="action" sort={sort} onSort={toggleSort}>{t("au.col.action")}</SortTh>
                <th className={thCls}>{t("au.col.target")}</th>
                <SortTh k="result" sort={sort} onSort={toggleSort}>{t("au.col.result")}</SortTh>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-800/70">
              {sorted.map((e) => (
                <tr key={e.seq} className={`align-top ${trCls}`}>
                  <td className="py-1.5 pr-3 text-slate-500">{e.seq}</td>
                  <td className={"py-1.5 pr-3 whitespace-nowrap " + shaCls}>{e.ts.replace("T", " ").replace("Z", "")}</td>
                  <td className="py-1.5 pr-3"><Badge tone={toneByCategory[e.category] ?? "slate"}>{e.category}</Badge></td>
                  <td className="py-1.5 pr-3 text-slate-200">{e.action}</td>
                  <td className="py-1.5 pr-3 text-slate-300">{e.target}</td>
                  <td className={`py-1.5 ${e.result === "ok" || e.result === "tentative" ? "text-teal-400" : "text-rose-400"}`}>{e.result}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Card>
    </div>
  );
}
