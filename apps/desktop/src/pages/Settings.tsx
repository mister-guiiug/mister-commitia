import { useEffect, useState } from "react";
import { asIpcError, call } from "../ipc";
import type { AiProviderConfig, AiProviderKind, RepoRef } from "../types";
import { Badge, Button, Card, Empty, ErrorBox, Field, inputCls, useToast } from "../ui";

export default function SettingsPage({
  repos, onChanged,
}: {
  repos: RepoRef[];
  onChanged: () => Promise<void>;
}) {
  const toast = useToast();
  const [providers, setProviders] = useState<AiProviderConfig[]>([]);
  const [error, setError] = useState<string | null>(null);

  const [kind, setKind] = useState<AiProviderKind>("ollama");
  const [baseUrl, setBaseUrl] = useState("http://127.0.0.1:11434");
  const [model, setModel] = useState("qwen2.5-coder");
  const [apiKey, setApiKey] = useState("");

  const [repoId, setRepoId] = useState("");
  const [policy, setPolicy] = useState<"keep-required" | "normalization-allowed">("keep-required");
  const [protectedBranches, setProtectedBranches] = useState("");
  const [trailers, setTrailers] = useState("Signed-off-by");

  const refresh = async () => setProviders(await call<AiProviderConfig[]>("ai_provider_list"));
  useEffect(() => { void refresh(); }, []);
  useEffect(() => {
    const r = repos.find((x) => x.id === repoId) ?? repos[0];
    if (r) {
      if (!repoId) setRepoId(r.id);
      setPolicy(r.governance.ai_attribution_policy);
      setProtectedBranches(r.protected_branches.join(", "));
      setTrailers(r.governance.protected_trailers.join(", "));
    }
  }, [repoId, repos]);

  useEffect(() => {
    if (kind === "ollama") { setBaseUrl("http://127.0.0.1:11434"); setModel("qwen2.5-coder"); }
    if (kind === "anthropic") { setBaseUrl("https://api.anthropic.com"); setModel("claude-sonnet-5"); }
    if (kind === "open_ai_compat") { setBaseUrl("https://passerelle.example.com"); setModel(""); }
  }, [kind]);

  const saveProvider = async () => {
    setError(null);
    try {
      await call("ai_provider_save", {
        kind, baseUrl: baseUrl || null, model: model || null,
        apiKey: apiKey || null, isDefault: true,
      });
      setApiKey("");
      await refresh();
      toast("success", "Fournisseur IA enregistré comme défaut" + (apiKey ? " — clé envoyée au coffre" : ""));
    } catch (e) { setError(asIpcError(e).message); }
  };

  const removeProvider = async (id: string) => {
    try {
      await call("ai_provider_remove", { id });
      await refresh();
      toast("info", "Fournisseur retiré (clé purgée du coffre)");
    } catch (e) { setError(asIpcError(e).message); }
  };

  const saveGovernance = async () => {
    setError(null);
    const r = repos.find((x) => x.id === repoId);
    if (!r) return;
    try {
      await call("repo_set_governance", {
        id: r.id,
        governance: {
          ...r.governance,
          ai_attribution_policy: policy,
          protected_trailers: trailers.split(",").map((s) => s.trim()).filter(Boolean),
        },
        protectedBranches: protectedBranches.split(",").map((s) => s.trim()).filter(Boolean),
      });
      await onChanged();
      toast("success", `Gouvernance de « ${r.name} » enregistrée`);
    } catch (e) { setError(asIpcError(e).message); }
  };

  return (
    <div className="mx-auto max-w-4xl space-y-4">
      <ErrorBox error={error} />
      <Card title="Fournisseur IA (propositions uniquement — jamais d'action automatique)">
        <div className="grid grid-cols-4 gap-2">
          <Field label="Type">
            <select className={inputCls} value={kind} onChange={(e) => setKind(e.target.value as AiProviderKind)}>
              <option value="rule_based">Assistant local déterministe (sans LLM)</option>
              <option value="ollama">Ollama (LLM local)</option>
              <option value="open_ai_compat">Endpoint d'entreprise (compatible OpenAI)</option>
              <option value="anthropic">Anthropic</option>
            </select>
          </Field>
          <Field label="URL de base"><input className={inputCls} value={baseUrl} onChange={(e) => setBaseUrl(e.target.value)} disabled={kind === "rule_based"} /></Field>
          <Field label="Modèle"><input className={inputCls} value={model} onChange={(e) => setModel(e.target.value)} disabled={kind === "rule_based"} /></Field>
          <Field label="Clé d'API (coffre OS)">
            <input type="password" className={inputCls} value={apiKey} onChange={(e) => setApiKey(e.target.value)} disabled={kind === "rule_based" || kind === "ollama"} />
          </Field>
        </div>
        <div className="mt-3 flex items-center gap-3">
          <Button kind="primary" onClick={saveProvider}>Définir comme fournisseur par défaut</Button>
          <span className="text-xs text-slate-500">
            Sans fournisseur configuré, l'assistant local déterministe est utilisé (100 % hors-ligne).
            Tout envoi à un fournisseur distant exige un consentement explicite avec aperçu des données.
          </span>
        </div>
        {providers.length > 0 && (
          <ul className="mt-3 space-y-1 text-sm">
            {providers.map((p) => (
              <li key={p.id} className="flex items-center gap-2 rounded border border-slate-800 bg-slate-950/50 px-2.5 py-1.5">
                <Badge tone={p.is_default ? "teal" : "slate"}>{p.is_default ? "défaut" : "secondaire"}</Badge>
                <span className="flex-1 text-slate-200">{p.kind} · {p.model ?? "—"} · {p.base_url ?? "—"}</span>
                <span className="text-xs text-slate-500">{p.key_ref ? "clé au coffre" : "sans clé"}</span>
                <Button kind="danger" onClick={() => removeProvider(p.id)}>retirer</Button>
              </li>
            ))}
          </ul>
        )}
      </Card>

      <Card title="Gouvernance par dépôt">
        {repos.length === 0 ? (
          <Empty>Déclarer un dépôt d'abord.</Empty>
        ) : (
          <div className="space-y-2.5">
            <Field label="Dépôt">
              <select className={inputCls} value={repoId} onChange={(e) => setRepoId(e.target.value)}>
                {repos.map((r) => <option key={r.id} value={r.id}>{r.name}</option>)}
              </select>
            </Field>
            <Field label="Politique d'attribution IA">
              <select className={inputCls} value={policy} onChange={(e) => setPolicy(e.target.value as typeof policy)}>
                <option value="keep-required">keep-required — traçabilité IA exigée&nbsp;: la skill de nettoyage REFUSE (défaut)</option>
                <option value="normalization-allowed">normalization-allowed — normalisation des mentions autorisée</option>
              </select>
            </Field>
            <div className="grid grid-cols-2 gap-2">
              <Field label="Branches protégées (virgules)">
                <input className={inputCls} value={protectedBranches} onChange={(e) => setProtectedBranches(e.target.value)} />
              </Field>
              <Field label="Trailers protégés — jamais supprimés (virgules)">
                <input className={inputCls} value={trailers} onChange={(e) => setTrailers(e.target.value)} />
              </Field>
            </div>
            <p className="text-xs text-slate-500">
              La normalisation des messages est soumise à ces règles&nbsp;: si la politique du dépôt impose la
              traçabilité des contributions assistées, l'application refuse la suppression et l'explique.
              Chaque normalisation appliquée est journalisée avec le contenu retiré.
            </p>
            <Button kind="primary" onClick={saveGovernance}>Enregistrer la gouvernance</Button>
          </div>
        )}
      </Card>
    </div>
  );
}
