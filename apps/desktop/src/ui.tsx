import type { ReactNode } from "react";

export function Button({
  children, onClick, kind = "default", disabled, title,
}: {
  children: ReactNode; onClick?: () => void;
  kind?: "default" | "primary" | "danger" | "ghost"; disabled?: boolean; title?: string;
}) {
  const styles: Record<string, string> = {
    default: "bg-slate-800 hover:bg-slate-700 border-slate-600 text-slate-100",
    primary: "bg-teal-700 hover:bg-teal-600 border-teal-500 text-white",
    danger: "bg-rose-900/60 hover:bg-rose-800 border-rose-600 text-rose-100",
    ghost: "bg-transparent hover:bg-slate-800 border-transparent text-slate-300",
  };
  return (
    <button
      type="button"
      title={title}
      disabled={disabled}
      onClick={onClick}
      className={`rounded border px-3 py-1.5 text-sm font-medium transition disabled:cursor-not-allowed disabled:opacity-40 ${styles[kind]}`}
    >
      {children}
    </button>
  );
}

export function Badge({ children, tone = "slate" }: { children: ReactNode; tone?: string }) {
  const tones: Record<string, string> = {
    slate: "bg-slate-800 text-slate-300 border-slate-600",
    teal: "bg-teal-900/50 text-teal-300 border-teal-700",
    amber: "bg-amber-900/40 text-amber-300 border-amber-700",
    rose: "bg-rose-900/40 text-rose-300 border-rose-700",
    violet: "bg-violet-900/40 text-violet-300 border-violet-700",
    sky: "bg-sky-900/40 text-sky-300 border-sky-700",
  };
  return (
    <span className={`inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-xs ${tones[tone] ?? tones.slate}`}>
      {children}
    </span>
  );
}

export function Card({ title, actions, children }: { title: ReactNode; actions?: ReactNode; children: ReactNode }) {
  return (
    <section className="rounded-lg border border-slate-800 bg-slate-900/60">
      <header className="flex items-center justify-between gap-2 border-b border-slate-800 px-4 py-2.5">
        <h2 className="text-sm font-semibold text-slate-200">{title}</h2>
        <div className="flex items-center gap-2">{actions}</div>
      </header>
      <div className="p-4">{children}</div>
    </section>
  );
}

export function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="block text-sm">
      <span className="mb-1 block text-xs font-medium uppercase tracking-wide text-slate-400">{label}</span>
      {children}
    </label>
  );
}

export const inputCls =
  "w-full rounded border border-slate-700 bg-slate-950 px-2.5 py-1.5 text-sm text-slate-100 placeholder-slate-500 focus:border-teal-600 focus:outline-none";

export function ErrorBox({ error }: { error: string | null }) {
  if (!error) return null;
  return (
    <div className="mt-2 rounded border border-rose-800 bg-rose-950/50 px-3 py-2 text-sm text-rose-200" role="alert">
      {error}
    </div>
  );
}

export function Empty({ children }: { children: ReactNode }) {
  return <p className="py-6 text-center text-sm text-slate-500">{children}</p>;
}

export function riskTone(risk: string): string {
  return risk === "high" ? "rose" : risk === "medium" ? "amber" : "teal";
}

export function verdictTone(v: string): string {
  return v === "bloquant" ? "rose" : v === "attention" ? "amber" : "teal";
}
