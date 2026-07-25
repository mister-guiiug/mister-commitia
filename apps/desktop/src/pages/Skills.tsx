import { useEffect, useState } from "react";
import { Pencil, PlayCircle } from "lucide-react";
import { asIpcError, call } from "../ipc";
import { t, useLang } from "../i18n";
import type { SkillMeta, SkillTestResult } from "../types";
import { Badge, Button, Card, Empty, ErrorBox, ICON_SM, Modal, inputCls, useToast } from "../ui";

export default function SkillsPage() {
  useLang();
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
      const name = editing.name;
      setEditing(null);
      await refresh();
      toast("success", t("sk.saved").replace("{n}", name));
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
      toast(passed === res.length ? "success" : "error", `${name} : ${passed}/${res.length} ${t("sk.testsGreen")}`);
    } catch (e) {
      setError(asIpcError(e).message);
    }
  };

  return (
    <div className="mx-auto max-w-4xl space-y-4">
      <ErrorBox error={error} />
      {loadErrors.length > 0 && (
        <ErrorBox error={t("sk.ignored").replace("{x}", loadErrors.map(([n, e]) => `${n} (${e})`).join(" ; "))} />
      )}
      {skills.length === 0 && <Empty>{t("sk.empty")}</Empty>}
      {skills.map((s) => (
        <Card
          key={s.name}
          title={
            <span className="flex items-center gap-2">
              {s.name} <Badge tone="slate">v{s.version}</Badge>
              <Badge tone={s.status === "published" ? "teal" : "amber"}>{s.status}</Badge>
              <Badge tone="violet">{s.output}</Badge>
              {s.local_capable && <Badge tone="sky">{t("sk.offline")}</Badge>}
            </span>
          }
          actions={
            <>
              <Button onClick={() => void openEditor(s.name)} title={t("sk.editManifest")}>
                <Pencil size={ICON_SM} /> {t("sk.edit")}
              </Button>
              {s.local_capable ? (
                <Button onClick={() => runTests(s.name)}>
                  <PlayCircle size={ICON_SM} /> {t("sk.runTests")} ({s.tests})
                </Button>
              ) : (
                <span className="text-xs text-slate-500">{t("sk.testsViaLlm")}</span>
              )}
            </>
          }
        >
          <p className="text-sm text-slate-300">{s.description}</p>
          <p className="mt-1 text-xs text-slate-500">owner : {s.owner || "—"}</p>
          <div className="mt-3 grid grid-cols-2 gap-4">
            <div>
              <h3 className="mb-1 text-xs font-semibold uppercase tracking-wide text-slate-400">{t("sk.rules")}</h3>
              <ul className="list-inside list-disc space-y-0.5 text-xs text-slate-400">
                {s.rules.map((r, i) => <li key={i}>{r}</li>)}
              </ul>
            </div>
            <div>
              <h3 className="mb-1 text-xs font-semibold uppercase tracking-wide text-slate-400">
                {t("sk.guardrails")}
              </h3>
              <div className="flex flex-wrap gap-1.5">
                {s.guardrails.map((gg) => <Badge key={gg} tone="amber">{gg}</Badge>)}
              </div>
            </div>
          </div>
          {testResults[s.name] && (
            <div className="mt-3 rounded border border-slate-800 bg-slate-950/60 p-3">
              {testResults[s.name].map((tr) => (
                <div key={tr.case} className="flex items-center gap-2 py-0.5 text-sm">
                  <span className={tr.passed ? "text-teal-400" : "text-rose-400"}>{tr.passed ? "✔" : "✘"}</span>
                  <span className="text-slate-200">{tr.case}</span>
                  <span className="text-xs text-slate-500">{tr.detail}</span>
                </div>
              ))}
            </div>
          )}
        </Card>
      ))}

      {editing && (
        <Modal
          title={t("sk.editorTitle").replace("{n}", editing.name)}
          width={720}
          onClose={() => setEditing(null)}
          footer={
            <>
              <Button onClick={() => setEditing(null)}>{t("common.cancel")}</Button>
              <Button kind="primary" loading={saving} onClick={saveSkill}>
                {t("sk.save")}
              </Button>
            </>
          }
        >
          <textarea
            className={inputCls + " min-h-[50vh] font-mono text-xs leading-relaxed"}
            aria-label={t("sk.editorAria")}
            value={editing.content}
            onChange={(e) => setEditing((s) => s && { ...s, content: e.target.value })}
            autoFocus
            spellCheck={false}
          />
          <ErrorBox error={editError} />
          <p className="mt-2 text-xs text-slate-400">{t("sk.editorNote")}</p>
        </Modal>
      )}
    </div>
  );
}
