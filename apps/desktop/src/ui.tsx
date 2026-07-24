// Primitives UI partagées — design tokens centralisés.
// Palette sémantique : teal = action · rose = destructif · amber = attention ·
// sky = information · violet = IA · slate = neutre.
// Icônes : ICON_SM (14) inline, ICON_MD (16) navigation/titres.

import {
  createContext, useCallback, useContext, useEffect, useMemo, useRef, useState,
  type ReactNode,
} from "react";
import { Loader2 } from "lucide-react";

export const ICON_SM = 14;
export const ICON_MD = 16;

export const inputCls =
  "w-full rounded border border-slate-700 bg-slate-950 px-2.5 py-1.5 text-sm text-slate-100 placeholder-slate-500 focus:border-teal-600 focus:outline-none";

export const trCls = "transition-colors hover:bg-slate-900/40";
export const shaCls = "font-mono text-xs text-slate-400";
export const thCls =
  "py-2 pr-3 text-left text-xs font-semibold uppercase tracking-wide text-slate-500";

// ---------------------------------------------------------------------------
// Boutons / badges
// ---------------------------------------------------------------------------

export function Button({
  children, onClick, kind = "default", disabled, title, loading, autoFocus,
}: {
  children: ReactNode; onClick?: () => void;
  kind?: "default" | "primary" | "danger" | "ghost";
  disabled?: boolean; title?: string; loading?: boolean; autoFocus?: boolean;
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
      autoFocus={autoFocus}
      disabled={disabled || loading}
      onClick={onClick}
      className={`inline-flex items-center gap-1.5 rounded border px-3 py-1.5 text-sm font-medium transition disabled:cursor-not-allowed disabled:opacity-40 ${styles[kind]}`}
    >
      {loading && <Loader2 size={ICON_SM} className="animate-spin" />}
      {children}
    </button>
  );
}

const badgeTones: Record<string, string> = {
  slate: "bg-slate-800 text-slate-300 border-slate-600",
  teal: "bg-teal-900/50 text-teal-300 border-teal-700",
  amber: "bg-amber-900/40 text-amber-300 border-amber-700",
  rose: "bg-rose-900/40 text-rose-300 border-rose-700",
  violet: "bg-violet-900/40 text-violet-300 border-violet-700",
  sky: "bg-sky-900/40 text-sky-300 border-sky-700",
};

export function Badge({ children, tone = "slate", title }: { children: ReactNode; tone?: string; title?: string }) {
  return (
    <span
      title={title}
      className={`inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-xs ${badgeTones[tone] ?? badgeTones.slate}`}
    >
      {children}
    </span>
  );
}

// ---------------------------------------------------------------------------
// Verdicts — échelle UNIQUE ok / attention / bloquant (risques ET CI)
// ---------------------------------------------------------------------------

export function verdictTone(v: string): string {
  return v === "bloquant" ? "rose" : v === "attention" ? "amber" : "teal";
}

export function riskTone(risk: string): string {
  return risk === "high" ? "rose" : risk === "medium" ? "amber" : "teal";
}

export function VerdictBadge({ verdict, label }: { verdict: string; label?: string }) {
  return <Badge tone={verdictTone(verdict)}>{label ?? verdict}</Badge>;
}

export function VerdictLegend() {
  return (
    <span className="inline-flex items-center gap-1.5 text-xs text-slate-500">
      Légende&nbsp;: <VerdictBadge verdict="ok" /> <VerdictBadge verdict="attention" />{" "}
      <VerdictBadge verdict="bloquant" />
    </span>
  );
}

// ---------------------------------------------------------------------------
// Conteneurs
// ---------------------------------------------------------------------------

export function Card({ title, actions, children }: { title: ReactNode; actions?: ReactNode; children: ReactNode }) {
  return (
    <section className="rounded-lg border border-slate-800 bg-slate-900/60">
      <header className="flex flex-wrap items-center justify-between gap-2 border-b border-slate-800 px-4 py-2.5">
        <h2 className="text-sm font-semibold text-slate-200">{title}</h2>
        <div className="flex flex-wrap items-center gap-2">{actions}</div>
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

export function ErrorBox({ error }: { error: string | null }) {
  if (!error) return null;
  return (
    <div className="mt-2 rounded border border-rose-800 bg-rose-950/50 px-3 py-2 text-sm text-rose-200" role="alert">
      {error}
    </div>
  );
}

/// Barre de progression d'une opération longue (T2) : phase courante, compteur
/// quand le total est connu (sinon indéterminée), annulation coopérative.
export function ProgressPanel({
  label, phase, current, total, onCancel,
}: {
  label: string; phase: string; current: number; total: number | null;
  onCancel?: () => void;
}) {
  const pct = total ? Math.min(100, Math.round((current / total) * 100)) : null;
  return (
    <div className="flex items-center gap-3 rounded border border-slate-800 bg-slate-950/70 px-3 py-2">
      <Loader2 size={ICON_MD} className="shrink-0 animate-spin text-teal-400" />
      <div className="min-w-0 flex-1">
        <div className="flex items-baseline justify-between gap-2 text-xs">
          <span className="truncate text-slate-300">
            {label}&nbsp;— {phase}
          </span>
          <span className="shrink-0 tabular-nums text-slate-500">
            {total ? `${current}/${total}` : "…"}
          </span>
        </div>
        <div
          role="progressbar"
          aria-label={label}
          aria-valuemin={0}
          aria-valuemax={total ?? undefined}
          aria-valuenow={total ? current : undefined}
          aria-valuetext={phase}
          className="mt-1.5 h-1.5 overflow-hidden rounded bg-slate-800"
        >
          <div
            className={`h-full rounded bg-teal-500 transition-[width] duration-200 ${pct === null ? "w-1/3 animate-pulse" : ""}`}
            style={pct === null ? undefined : { width: `${pct}%` }}
          />
        </div>
      </div>
      {onCancel && (
        <Button kind="ghost" onClick={onCancel} title="Annuler l'opération en cours (arrêt au prochain point sûr)">
          Annuler
        </Button>
      )}
    </div>
  );
}

/// État vide ACTIONNABLE : toujours proposer le geste suivant quand il existe.
export function Empty({
  children, actionLabel, onAction,
}: {
  children: ReactNode; actionLabel?: string; onAction?: () => void;
}) {
  return (
    <div className="py-6 text-center">
      <p className="text-sm text-slate-500">{children}</p>
      {actionLabel && onAction && (
        <div className="mt-3">
          <Button kind="primary" onClick={onAction}>{actionLabel}</Button>
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Modal accessible unique (overlay, rôle dialog, Échap, piège de focus)
// ---------------------------------------------------------------------------

const FOCUSABLE =
  'button:not([disabled]), input:not([disabled]), textarea:not([disabled]), select:not([disabled]), a[href], [tabindex]:not([tabindex="-1"])';

export function Modal({
  title, tone = "slate", onClose, children, footer, width = 560,
}: {
  title: ReactNode; tone?: "slate" | "rose" | "sky";
  onClose: () => void; children: ReactNode; footer?: ReactNode; width?: number;
}) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = ref.current;
    const first = el?.querySelector<HTMLElement>("[autofocus]") ?? el?.querySelector<HTMLElement>(FOCUSABLE);
    first?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
      if (e.key === "Tab" && el) {
        const items = Array.from(el.querySelectorAll<HTMLElement>(FOCUSABLE));
        if (items.length === 0) return;
        const firstEl = items[0];
        const lastEl = items[items.length - 1];
        if (e.shiftKey && document.activeElement === firstEl) {
          e.preventDefault();
          lastEl.focus();
        } else if (!e.shiftKey && document.activeElement === lastEl) {
          e.preventDefault();
          firstEl.focus();
        }
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [onClose]);

  const border = tone === "rose" ? "border-rose-800" : tone === "sky" ? "border-sky-800" : "border-slate-700";
  const heading = tone === "rose" ? "text-rose-300" : tone === "sky" ? "text-sky-300" : "text-slate-200";

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        ref={ref}
        role="dialog"
        aria-modal="true"
        aria-label={typeof title === "string" ? title : undefined}
        className={`w-full rounded-lg border ${border} bg-slate-900 p-5 shadow-xl`}
        style={{ maxWidth: width }}
      >
        <h3 className={`text-sm font-semibold ${heading}`}>{title}</h3>
        <div className="mt-2">{children}</div>
        {footer && <div className="mt-4 flex justify-end gap-2">{footer}</div>}
      </div>
    </div>
  );
}

/// Confirmation renforcée UNIFIÉE : saisie exacte du nom de la cible.
/// Utilisée pour toute action destructive (application sur branche partagée,
/// suppression d'un run CI…). `expected` vient du cœur (code confirm_required).
export function ConfirmTyped({
  title, description, expected, confirmLabel, busy, onConfirm, onClose,
}: {
  title: string; description: ReactNode; expected: string;
  confirmLabel: string; busy?: boolean;
  onConfirm: (typed: string) => void; onClose: () => void;
}) {
  const [typed, setTyped] = useState("");
  const ok = typed === expected;
  return (
    <Modal
      title={title}
      tone="rose"
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>Annuler</Button>
          <Button kind="danger" disabled={!ok} loading={busy} onClick={() => onConfirm(typed)}>
            {confirmLabel}
          </Button>
        </>
      }
    >
      <div className="text-sm text-slate-300">{description}</div>
      <p className="mt-2 text-xs text-slate-400">
        Pour confirmer, saisir exactement&nbsp;: <code className="text-rose-300">{expected}</code>
      </p>
      <input
        autoFocus
        className={inputCls + " mt-2"}
        value={typed}
        onChange={(e) => setTyped(e.target.value)}
        placeholder={expected}
        aria-label={`Saisir ${expected} pour confirmer`}
      />
    </Modal>
  );
}

// ---------------------------------------------------------------------------
// Toasts non bloquants (aria-live)
// ---------------------------------------------------------------------------

type ToastKind = "success" | "error" | "info";
interface Toast { id: number; kind: ToastKind; message: string }

const ToastCtx = createContext<(kind: ToastKind, message: string) => void>(() => {});

export function useToast() {
  return useContext(ToastCtx);
}

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const idRef = useRef(0);

  const push = useCallback((kind: ToastKind, message: string) => {
    const id = ++idRef.current;
    setToasts((t) => [...t, { id, kind, message }]);
    window.setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), 4500);
  }, []);

  const tones: Record<ToastKind, string> = useMemo(
    () => ({
      success: "border-teal-700 bg-teal-950/90 text-teal-200",
      error: "border-rose-700 bg-rose-950/90 text-rose-200",
      info: "border-sky-700 bg-sky-950/90 text-sky-200",
    }),
    [],
  );

  return (
    <ToastCtx.Provider value={push}>
      {children}
      <div aria-live="polite" className="pointer-events-none fixed bottom-4 right-4 z-[60] flex w-80 flex-col gap-2">
        {toasts.map((t) => (
          <div
            key={t.id}
            role={t.kind === "error" ? "alert" : "status"}
            className={`pointer-events-auto rounded border px-3 py-2 text-sm shadow-lg ${tones[t.kind]}`}
          >
            {t.message}
          </div>
        ))}
      </div>
    </ToastCtx.Provider>
  );
}
