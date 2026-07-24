import { useEffect, useState } from "react";
import { Download, RefreshCw } from "lucide-react";
import { asIpcError, call } from "../ipc";
import type { AuditEvent } from "../types";
import { Badge, Button, Card, Empty, ErrorBox, ICON_SM, shaCls, thCls, trCls, useToast } from "../ui";

const toneByCategory: Record<string, string> = {
  git_rewrite: "amber",
  ci_cleanup: "rose",
  secret: "violet",
  skill: "sky",
  config: "slate",
};

export default function AuditPage() {
  const toast = useToast();
  const [events, setEvents] = useState<AuditEvent[]>([]);
  const [error, setError] = useState<string | null>(null);

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
      toast("success", "Journal exporté (JSONL chronologique)");
    } catch (e) {
      setError(asIpcError(e).message);
    }
  };

  return (
    <div className="mx-auto max-w-5xl">
      <Card
        title="Journal d'audit (append-only, secrets masqués)"
        actions={
          <>
            <Button onClick={refresh}><RefreshCw size={ICON_SM} /> Actualiser</Button>
            <Button onClick={exportJsonl}><Download size={ICON_SM} /> Export JSONL</Button>
          </>
        }
      >
        <ErrorBox error={error} />
        {events.length === 0 ? (
          <Empty>Aucun événement pour l'instant.</Empty>
        ) : (
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-slate-800">
                <th className={thCls}>#</th>
                <th className={thCls}>Horodatage</th>
                <th className={thCls}>Catégorie</th>
                <th className={thCls}>Action</th>
                <th className={thCls}>Cible</th>
                <th className={thCls}>Résultat</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-800/70">
              {events.map((e) => (
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
