import { useState } from "react";
import { ArrowRight, FolderPlus, Trash2 } from "lucide-react";
import { asIpcError, call, pickDirectory } from "../ipc";
import type { RepoRef } from "../types";
import { Badge, Button, Card, Empty, ErrorBox, ICON_SM, Modal, useToast } from "../ui";

export default function ReposPage({
  repos, selected, onSelect, onChanged,
}: {
  repos: RepoRef[];
  selected: RepoRef | null;
  onSelect: (r: RepoRef) => void;
  onChanged: () => Promise<void>;
}) {
  const toast = useToast();
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [removing, setRemoving] = useState<RepoRef | null>(null);

  const add = async () => {
    setError(null);
    const path = await pickDirectory();
    if (!path) return;
    setBusy(true);
    try {
      const r = await call<RepoRef>("repo_declare", { path });
      await onChanged();
      toast("success", `Dépôt « ${r.name} » déclaré — analyse locale disponible`);
    } catch (e) {
      setError(asIpcError(e).message);
    } finally {
      setBusy(false);
    }
  };

  const remove = async (r: RepoRef) => {
    setError(null);
    try {
      await call("repo_remove", { id: r.id });
      setRemoving(null);
      await onChanged();
      toast("info", `« ${r.name} » retiré du workspace (le dépôt Git est intact)`);
    } catch (e) {
      setError(asIpcError(e).message);
    }
  };

  return (
    <div className="mx-auto max-w-4xl space-y-4">
      <Card
        title="Dépôts déclarés"
        actions={
          <Button kind="primary" onClick={add} loading={busy}>
            <FolderPlus size={ICON_SM} /> Déclarer un dépôt local
          </Button>
        }
      >
        <ErrorBox error={error} />
        {repos.length === 0 ? (
          <Empty actionLabel="Déclarer un dépôt local" onAction={add}>
            Aucun dépôt. L'analyse est 100&nbsp;% locale (mode offline).
          </Empty>
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
                  <div className="truncate text-xs text-slate-500" title={r.local_path}>
                    {r.local_path}
                    {r.remote_url ? ` · ${r.remote_url}` : " · sans remote"}
                  </div>
                  <div className="mt-1 flex flex-wrap items-center gap-1 text-xs text-slate-500">
                    Branche par défaut&nbsp;: <code>{r.default_branch ?? "?"}</code> · protégées&nbsp;:
                    {r.protected_branches.map((b) => (
                      <Badge key={b}>{b}</Badge>
                    ))}
                  </div>
                </div>
                <Button onClick={() => onSelect(r)}>
                  Analyser <ArrowRight size={ICON_SM} />
                </Button>
                <Button kind="danger" onClick={() => setRemoving(r)} title="Retirer du workspace">
                  <Trash2 size={ICON_SM} />
                </Button>
              </li>
            ))}
          </ul>
        )}
      </Card>
      <p className="text-xs text-slate-500">
        Garde-fous actifs&nbsp;: branches protégées bloquées · dry-run obligatoire · backup automatique avant
        application · aucune action IA automatique · secrets au coffre du système.
      </p>

      {removing && (
        <Modal
          title={`Retirer « ${removing.name} » du workspace ?`}
          onClose={() => setRemoving(null)}
          footer={
            <>
              <Button onClick={() => setRemoving(null)} autoFocus>
                Annuler
              </Button>
              <Button kind="danger" onClick={() => void remove(removing)}>
                Retirer
              </Button>
            </>
          }
        >
          <p className="text-sm text-slate-300">
            Seules les métadonnées locales (analyses, plans, propositions) sont concernées&nbsp;: le dépôt Git
            sur disque n'est pas touché.
          </p>
        </Modal>
      )}
    </div>
  );
}
