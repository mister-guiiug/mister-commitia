import { useState } from "react";
import { ArrowRight, FolderPlus, Trash2 } from "lucide-react";
import { asIpcError, call, pickDirectory } from "../ipc";
import { t, useLang } from "../i18n";
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
  useLang();
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
      toast("success", t("repo.declared").replace("{n}", r.name));
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
      toast("info", t("repo.removed").replace("{n}", r.name));
    } catch (e) {
      setError(asIpcError(e).message);
    }
  };

  return (
    <div className="mx-auto max-w-4xl space-y-4">
      <Card
        title={t("repo.declared.title")}
        actions={
          <Button kind="primary" onClick={add} loading={busy}>
            <FolderPlus size={ICON_SM} /> {t("repo.add")}
          </Button>
        }
      >
        <ErrorBox error={error} />
        {repos.length === 0 ? (
          <Empty actionLabel={t("repo.add")} onAction={add}>
            {t("repo.empty")}
          </Empty>
        ) : (
          <ul className="divide-y divide-slate-800">
            {repos.map((r) => (
              <li key={r.id} className="flex items-center gap-3 py-3">
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="font-medium text-slate-100">{r.name}</span>
                    {selected?.id === r.id && <Badge tone="teal">{t("repo.selected")}</Badge>}
                    {r.governance.ai_attribution_policy === "keep-required" ? (
                      <Badge tone="sky">{t("repo.aiRequired")}</Badge>
                    ) : (
                      <Badge tone="violet">{t("repo.aiAllowed")}</Badge>
                    )}
                  </div>
                  <div className="truncate text-xs text-slate-500" title={r.local_path}>
                    {r.local_path}
                    {r.remote_url ? ` · ${r.remote_url}` : ` · ${t("repo.noRemote")}`}
                  </div>
                  <div className="mt-1 flex flex-wrap items-center gap-1 text-xs text-slate-500">
                    {t("repo.defaultBranch")}&nbsp;: <code>{r.default_branch ?? "?"}</code> · {t("repo.protectedLabel")}&nbsp;:
                    {r.protected_branches.map((b) => (
                      <Badge key={b}>{b}</Badge>
                    ))}
                  </div>
                </div>
                <Button onClick={() => onSelect(r)}>
                  {t("repo.analyze")} <ArrowRight size={ICON_SM} />
                </Button>
                <Button kind="danger" onClick={() => setRemoving(r)} title={t("repo.remove")}>
                  <Trash2 size={ICON_SM} />
                </Button>
              </li>
            ))}
          </ul>
        )}
      </Card>
      <p className="text-xs text-slate-500">{t("repo.guardrails")}</p>

      {removing && (
        <Modal
          title={t("repo.removeTitle").replace("{n}", removing.name)}
          onClose={() => setRemoving(null)}
          footer={
            <>
              <Button onClick={() => setRemoving(null)} autoFocus>
                {t("common.cancel")}
              </Button>
              <Button kind="danger" onClick={() => void remove(removing)}>
                {t("repo.remove")}
              </Button>
            </>
          }
        >
          <p className="text-sm text-slate-300">{t("repo.removeBody")}</p>
        </Modal>
      )}
    </div>
  );
}
