"use client";

import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  Legend,
  ResponsiveContainer,
} from "recharts";

type Series = Record<string, Record<string, [number, number][]>>;

const METRIC_COLORS: Record<string, string> = {
  lean_finalized_slot: "#4ade80",
  lean_justified_slot: "#818cf8",
  lean_head_slot: "#5eead4",
  lean_current_slot: "#64748b",
  lean_safe_target_slot: "#fbbf24",
};
const METRIC_LABEL: Record<string, string> = {
  lean_finalized_slot: "finalized",
  lean_justified_slot: "justified",
  lean_head_slot: "head",
  lean_current_slot: "current",
  lean_safe_target_slot: "safe_target",
};

// Collapse a metric's per-pod series to a single max-over-pods timeline (pods
// converge), then merge metrics on a shared time axis (seconds-from-start).
export default function FinalizationChart({ series }: { series: Series }) {
  const metrics = Object.keys(METRIC_LABEL).filter((m) => series[m]);
  if (!metrics.length) return <p className="muted">No metric series for this run.</p>;

  // find global start
  let t0 = Infinity;
  for (const m of metrics)
    for (const pod of Object.values(series[m]))
      for (const [t] of pod) t0 = Math.min(t0, t);

  // build map t -> {metric: maxv}
  const byT = new Map<number, any>();
  for (const m of metrics) {
    const merged = new Map<number, number>();
    for (const pod of Object.values(series[m]))
      for (const [t, v] of pod) merged.set(t, Math.max(merged.get(t) ?? -Infinity, v));
    for (const [t, v] of merged) {
      const sec = Math.round(t - t0);
      const row = byT.get(sec) ?? { t: sec };
      row[m] = v;
      byT.set(sec, row);
    }
  }
  const data = [...byT.values()].sort((a, b) => a.t - b.t);

  const fmt = (s: number) => {
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    return h ? `${h}h${m}m` : `${m}m`;
  };

  return (
    <ResponsiveContainer width="100%" height={360}>
      <LineChart data={data} margin={{ top: 8, right: 16, bottom: 8, left: 0 }}>
        <CartesianGrid stroke="#283041" strokeDasharray="3 3" />
        <XAxis
          dataKey="t"
          tickFormatter={fmt}
          stroke="#8b94a7"
          fontSize={11}
          type="number"
          domain={["dataMin", "dataMax"]}
        />
        <YAxis stroke="#8b94a7" fontSize={11} width={56} />
        <Tooltip
          contentStyle={{ background: "#141925", border: "1px solid #283041" }}
          labelFormatter={(s) => `t+${fmt(s as number)}`}
        />
        <Legend />
        {metrics.map((m) => (
          <Line
            key={m}
            type="monotone"
            dataKey={m}
            name={METRIC_LABEL[m]}
            stroke={METRIC_COLORS[m]}
            dot={false}
            strokeWidth={m === "lean_finalized_slot" ? 2.5 : 1.3}
            connectNulls
            isAnimationActive={false}
          />
        ))}
      </LineChart>
    </ResponsiveContainer>
  );
}
