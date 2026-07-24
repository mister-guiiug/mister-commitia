import { useEffect, useState } from "react";
import { Pencil, PlayCircle } from "lucide-react";
import { asIpcError, call } from "../ipc";
import type { SkillMeta, SkillTestResult } from "../types";
import { Badge, Button, Card, Empty, ErrorBox, ICON_SM, Modal, inputCls, useToast } from "../ui";

export default function SkillsPage() {
  const toast = useToast();
  const [skills, setSkills] = useState<SkillMeta[]>([]);
  const [loadErrors, setLoadErrors] = useState<[string, string][]>([]);
  const [testResults, setTestResults] = useState<Record<string, SkillTestResult[]>>({});
  const [editing, setEditing] = useState<{ name: string; content: string } | null>(null);
  const [editError, setEditError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = () =>
    call<[SkillMeta[], [string, string][]]>("skills_list")
      .then(([s, e]) => {
        setSkills(s);
        setLoadErrors(e);
      })
      .catch((e) => setError(asIpcError(e).message));

  useEffect(() => {
    void refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const openEditor = async (name: string) => {
    setError(null);
    setEditError(null);
    try {
      const content = await call<string>("skill_read", { name });
      setEditing({ name, content });
    } catch (e) {
      setError(asIpcError(e).message);
    }
  };

  const saveSkill = async () => {
    if (!editing) return;
    setEditError(null);
    setSaving(true);
    try {
      await call("skill_write", { name: editing.name, content: editing.content });
      setEditing(null);
      await refresh();
      toast("success", `Skill « ${editing.name} » enregistrée (édition journalisée)`);
    } catch (e) {
      setEditError(asIpcError(e).message);
    } finally {
      setSaving(false);
    }
  };

  const runTests = async (name: string) => {
    setError(null);
    try {
      const res = await call<SkillTestResult[]>("skill_run_tests", { name });
      setTestResults((prev) => ({ ...prev, [name]: res }));
      const passed = res.filter((r) => r.passed).length;
      toast(passed === res.length ? "success" : "error", `${name} : ${passed}/${res.length} tests verts`);
    } catch (e) {
      setError(asIpcError(e).message);
    }
  };

  return (
    <div className="mx-auto max-w-4xl space-y-4">
      <ErrorBox error={error} />
      {loadErrors.length > 0 && (
        <ErrorBox error={`Skills ignorées : ${loadErrors.map(([n, e]) => `${n} (${e})`).join(" ; ")}`} />
      )}
      {skills.length === 0 && <Empty>Aucune skill chargée.</Empty>}
      {skills.map((s) => (
        <Card
          key={s.name}
          title={
            <span className="flex items-center gap-2">
              {s.name} <Badge tone="slate">v{s.version}</Badge>
              <Badge tone={s.status === "published" ? "teal" : "amber"}>{s.status}</Badge>
              <Badge tone="violet">{s.output}</Badge>
              {s.local_capable && <Badge tone="sky">exécutable hors-ligne</Badge>}
            </span>
          }
          actions={
            <>
              <Button onClick={() => void openEditor(s.name)} title="Éditer le manifeste YAML">
                <Pencil size={ICON_SM} /> Éditer
              </Button>
              {s.local_capable ? (
                <Button onClick={() => runTests(s.name)}>
                  <PlayCircle size={ICON_SM} /> Lancer les tests ({s.tests})
                </Button>
              ) : (
                <span className="text-xs text-slate-500">tests via fournisseur LLM</span>
              )}
            </>
          }
        >
          <p className="text-sm text-slate-300">{s.description}</p>
          <p className="mt-1 text-xs text-slate-500">owner : {s.owner || "—"}</p>
          <div className="mt-3 grid grid-cols-2 gap-4">
            <div>
              <h3 className="mb-1 text-xs font-semibold uppercase tracking-wide text-slate-400">Règles</h3>
              <ul className="list-inside list-disc space-y-0.5 text-xs text-slate-400">
                {s.rules.map((r, i) => <li key={i}>{r}</li>)}
              </ul>
            </div>
            <div>
              <h3 className="mb-1 text-xs font-semibold uppercase tracking-wide text-slate-400">
                Garde-fous (vérifiés par l'application)
              </h3>
              <div className="flex flex-wrap gap-1.5">
                {s.guardrails.map((gg) => <Badge key={gg} tone="amber">{gg}</Badge>)}
              </div>
            </div>
          </div>
          {testResults[s.name] && (
            <div className="mt-3 rounded border border-slate-800 bg-slate-950/60 p-3">
              {testResults[s.name].map((t) => (
                <div key={t.case} className="flex items-center gap-2 py-0.5 text-sm">
                  <span className={t.passed ? "text-teal-400" : "text-rose-400"}>{t.passed ? "✔" : "✘"}</span>
                  <span className="text-slate-200">{t.case}</span>
                  <span className="text-xs text-slate-500">{t.detail}</span>
                </div>
              ))}
            </div>
          )}
        </Card>
      ))}

      {editing && (
        <Modal
          title={`Éditer la skill « ${editing.name} » (YAML validé à l'enregistrement)`}
          width={720}
          onClose={() => setEditing(null)}
          footer={
            <>
              <Button onClick={() => setEditing(null)}>Annuler</Button>
              <Button kind="primary" loading={saving} onClick={saveSkill}>
                Enregistrer
              </Button>
            </>
          }
        >
          <textarea
            className={inputCls + " min-h-[50vh] font-mono text-xs leading-relaxed"}
            aria-label="Manifeste YAML de la skill"
            value={editing.content}
            onChange={(e) => setEditing((s) => s && { ...s, content: e.target.value })}
            autoFocus
            spellCheck={false}
          />
          <ErrorBox error={editError} />
          <p className="mt-2 text-xs text-slate-400">
            Le champ <code>name</code> est immuable (renommer = créer une nouvelle skill)&nbsp;;
            chaque édition est journalisée. Relancer les tests après enregistrement.
          </p>
        </Modal>
      )}
    </div>
  );
}
