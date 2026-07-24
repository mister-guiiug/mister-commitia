import { useEffect, useState } from "react";
import { Download, RefreshCw } from "lucide-react";
import { call } from "../ipc";
import type { AuditEvent } from "../types";
import { Badge, Button, Card, Empty, ErrorBox } from "../ui";

const toneByCategory: Record<string, string> = {
  git_rewrite: "amber",
  ci_cleanup: "rose",
  secret: "violet",
  skill: "sky",
  config: "slate",
};

export default function AuditPage() {
  const [events, setEvents] = useState<AuditEvent[]>([]);
  const [error, setError] = useState<string | null>(null);

  const refresh = async () => {
    setError(null);
    try {
      setEvents(await call<AuditEvent[]>("audit_list", { limit: 200 }));
    } catch (e) {
      setError(String(e));
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
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="mx-auto max-w-5xl">
      <Card
        title="Journal d'audit (append-only, secrets masqués)"
        actions={
          <>
            <Button onClick={refresh}><span className="flex items-center gap-1"><RefreshCw size={14} /> Actualiser</span></Button>
            <Button onClick={exportJsonl}><span className="flex items-center gap-1"><Download size={14} /> Export JSONL</span></Button>
          </>
        }
      >
        <ErrorBox error={error} />
        {events.length === 0 ? (
          <Empty>Aucun événement pour l'instant.</Empty>
        ) : (
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-slate-800 text-left text-xs uppercase tracking-wide text-slate-500">
                <th className="py-2 pr-3">#</th>
                <th className="py-2 pr-3">Horodatage</th>
                <th className="py-2 pr-3">Catégorie</th>
                <th className="py-2 pr-3">Action</th>
                <th className="py-2 pr-3">Cible</th>
                <th className="py-2">Résultat</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-800/70">
              {events.map((e) => (
                <tr key={e.seq} className="align-top">
                  <td className="py-1.5 pr-3 text-slate-500">{e.seq}</td>
                  <td className="py-1.5 pr-3 whitespace-nowrap text-slate-400">{e.ts.replace("T", " ").replace("Z", "")}</td>
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
