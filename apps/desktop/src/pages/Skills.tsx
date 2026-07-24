import { useEffect, useState } from "react";
import { PlayCircle } from "lucide-react";
import { asIpcError, call } from "../ipc";
import type { SkillMeta, SkillTestResult } from "../types";
import { Badge, Button, Card, Empty, ErrorBox, ICON_SM, useToast } from "../ui";

export default function SkillsPage() {
  const toast = useToast();
  const [skills, setSkills] = useState<SkillMeta[]>([]);
  const [loadErrors, setLoadErrors] = useState<[string, string][]>([]);
  const [testResults, setTestResults] = useState<Record<string, SkillTestResult[]>>({});
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    call<[SkillMeta[], [string, string][]]>("skills_list")
      .then(([s, e]) => {
        setSkills(s);
        setLoadErrors(e);
      })
      .catch((e) => setError(String(e)));
  }, []);

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
            s.local_capable ? (
              <Button onClick={() => runTests(s.name)}>
                <PlayCircle size={ICON_SM} /> Lancer les tests ({s.tests})
              </Button>
            ) : (
              <span className="text-xs text-slate-500">tests via fournisseur LLM</span>
            )
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
    </div>
  );
}
