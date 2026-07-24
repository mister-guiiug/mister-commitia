// Vue graphe de l'historique (F1) : rendu SVG des lanes calculées par le cœur,
// aligné avec une colonne de libellés (sha, sujet, auteur). Lecture seule —
// reflète la topologie git réelle (indépendante d'un réordonnancement proposé).

import type { CommitGraph, CommitInfo } from "./types";
import { Badge, shaCls } from "./ui";

const LANE_COLORS = ["#2dd4bf", "#38bdf8", "#a78bfa", "#fbbf24", "#fb7185", "#34d399"];
const laneColor = (i: number) => LANE_COLORS[i % LANE_COLORS.length];

const ROW_H = 34;
const LANE_W = 22;
const PAD = 14;
const R = 5;

export default function GitGraph({
  graph, commits, onSelect,
}: {
  graph: CommitGraph; commits: CommitInfo[]; onSelect: (c: CommitInfo) => void;
}) {
  const bySha = new Map(commits.map((c) => [c.sha, c]));
  const nodeBySha = new Map(graph.nodes.map((n) => [n.sha, n]));
  const cx = (lane: number) => PAD + lane * LANE_W;
  const cy = (row: number) => row * ROW_H + ROW_H / 2;
  const gutterW = PAD * 2 + Math.max(0, graph.lanes - 1) * LANE_W;
  const height = graph.nodes.length * ROW_H;

  return (
    <div className="flex overflow-x-auto">
      <svg width={gutterW} height={height} className="shrink-0" role="img" aria-label="Graphe des commits">
        {/* Arêtes : d'abord (sous les nœuds). */}
        {graph.nodes.map((n) =>
          n.parents.map((p, i) => {
            const x1 = cx(n.lane);
            const y1 = cy(n.row);
            const parent = p.in_segment ? nodeBySha.get(p.sha) : undefined;
            if (parent) {
              const x2 = cx(parent.lane);
              const y2 = cy(parent.row);
              const color = laneColor(Math.max(n.lane, parent.lane));
              const d =
                x1 === x2
                  ? `M ${x1} ${y1} L ${x2} ${y2}`
                  : `M ${x1} ${y1} C ${x1} ${y1 + ROW_H / 2}, ${x2} ${y2 - ROW_H / 2}, ${x2} ${y2}`;
              return <path key={`${n.sha}-${i}`} d={d} stroke={color} strokeWidth={1.6} fill="none" />;
            }
            // Parent hors segment : arête-borne (descend vers le bas, tronquée).
            const yEnd = y1 + ROW_H * 0.55;
            return (
              <g key={`${n.sha}-b${i}`}>
                <path
                  d={`M ${x1} ${y1} L ${x1} ${yEnd}`}
                  stroke={laneColor(n.lane)}
                  strokeWidth={1.6}
                  strokeDasharray="2 2"
                  fill="none"
                />
                <circle cx={x1} cy={yEnd} r={2} fill="none" stroke={laneColor(n.lane)} strokeWidth={1.4} />
              </g>
            );
          }),
        )}
        {/* Nœuds par-dessus. */}
        {graph.nodes.map((n) =>
          n.is_merge ? (
            <circle
              key={n.sha}
              cx={cx(n.lane)}
              cy={cy(n.row)}
              r={R + 1}
              fill="#0f172a"
              stroke={laneColor(n.lane)}
              strokeWidth={2}
            />
          ) : (
            <circle key={n.sha} cx={cx(n.lane)} cy={cy(n.row)} r={R} fill={laneColor(n.lane)} />
          ),
        )}
      </svg>

      <div className="min-w-0 flex-1">
        {graph.nodes.map((n) => {
          const c = bySha.get(n.sha);
          return (
            <div
              key={n.sha}
              className="flex items-center gap-2 border-b border-slate-800/50 px-2"
              style={{ height: ROW_H }}
            >
              <button
                type="button"
                className={shaCls + " underline decoration-slate-700 underline-offset-2 hover:text-teal-300"}
                title="Voir le diff de ce commit"
                onClick={() => c && onSelect(c)}
              >
                {c?.short ?? n.sha.slice(0, 8)}
              </button>
              <span className="min-w-0 flex-1 truncate text-sm text-slate-100" title={c?.subject}>
                {c?.subject ?? "(commit)"}
              </span>
              {n.is_merge && <Badge tone="violet">merge</Badge>}
              {c?.on_remote && <Badge tone="rose">partagé</Badge>}
              {c?.signed && <Badge tone="sky">signé</Badge>}
              <span className="hidden shrink-0 text-xs text-slate-500 sm:inline">
                {c?.author_name} · {c?.date.slice(0, 10)}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
