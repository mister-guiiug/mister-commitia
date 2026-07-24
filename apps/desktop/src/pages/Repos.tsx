import { useState } from "react";
import { FolderPlus, Trash2, ArrowRight } from "lucide-react";
import { call, pickDirectory } from "../ipc";
import type { RepoRef } from "../types";
import { Badge, Button, Card, Empty, ErrorBox } from "../ui";

export default function ReposPage({
  repos, selected, onSelect, onChanged,
}: {
  repos: RepoRef[];
  selected: RepoRef | null;
  onSelect: (r: RepoRef) => void;
  onChanged: () => Promise<void>;
}) {
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const add = async () => {
    setError(null);
    const path = await pickDirectory();
    if (!path) return;
    setBusy(true);
    try {
      await call<RepoRef>("repo_declare", { path });
      await onChanged();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const remove = async (r: RepoRef) => {
    if (!window.confirm(`Retirer « ${r.name} » du workspace ? (le dépôt Git n'est pas touché)`)) return;
    try {
      await call("repo_remove", { id: r.id });
      await onChanged();
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="mx-auto max-w-4xl space-y-4">
      <Card
        title="Dépôts déclarés"
        actions={
          <Button kind="primary" onClick={add} disabled={busy}>
            <span className="flex items-center gap-1.5"><FolderPlus size={15} /> Déclarer un dépôt local</span>
          </Button>
        }
      >
        <ErrorBox error={error} />
        {repos.length === 0 ? (
          <Empty>Aucun dépôt. Déclarer un dépôt Git local pour commencer — l'analyse est 100 % locale (mode offline).</Empty>
        ) : (
          <ul className="divide-y divide-slate-800">
            {repos.map((r) => (
              <li key={r.id} className="flex items-center gap-3 py-3">
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="font-medium text-slate-100">{r.name}</span>
                    {selected?.id === r.id && <Badge tone="teal">sélectionné</Badge>}
                    {r.governance.ai_attribution_policy === "keep-required" ? (
                      <Badge tone="sky">traçabilité IA exigée</Badge>
                    ) : (
                      <Badge tone="violet">normalisation autorisée</Badge>
                    )}
                  </div>
                  <div className="truncate text-xs text-slate-500">
                    {r.local_path}
                    {r.remote_url ? ` · ${r.remote_url}` : " · sans remote"}
                  </div>
                  <div className="mt-1 text-xs text-slate-500">
                    Branche par défaut : <code>{r.default_branch ?? "?"}</code> · protégées :{" "}
                    {r.protected_branches.map((b) => (
                      <Badge key={b}>{b}</Badge>
                    ))}
                  </div>
                </div>
                <Button onClick={() => onSelect(r)}>
                  <span className="flex items-center gap-1">Analyser <ArrowRight size={14} /></span>
                </Button>
                <Button kind="danger" onClick={() => remove(r)} title="Retirer du workspace">
                  <Trash2 size={14} />
                </Button>
              </li>
            ))}
          </ul>
        )}
      </Card>
      <p className="text-xs text-slate-500">
        Garde-fous actifs : branches protégées bloquées · dry-run obligatoire · backup automatique avant application ·
        aucune action IA automatique · secrets au coffre du système.
      </p>
    </div>
  );
}
