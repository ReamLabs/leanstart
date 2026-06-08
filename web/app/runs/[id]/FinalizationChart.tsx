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
  lean_finalized_slot: "#11865b",
  lean_justified_slot: "#0069ff",
  lean_head_slot: "#7b3fe4",
  lean_current_slot: "#9aa6bd",
  lean_safe_target_slot: "#d39b1f",
};
const METRIC_LABEL: Record<string, string> = {
  lean_finalized_slot: "finalized",
  lean_justified_slot: "justified",
  lean_head_slot: "head",
  lean_current_slot: "current",
  lean_safe_target_slot: "safe target",
};

// Collapse each metric's per-pod series to a max-over-pods timeline (pods
// converge), then merge metrics on a shared time axis (seconds from start).
export default function FinalizationChart({ series }: { series: Series }) {
  const metrics = Object.keys(METRIC_LABEL).filter((m) => series[m]);
  if (!metrics.length)
    return <p className="faint">No metric series captured for this run.</p>;

  let t0 = Infinity;
  for (const m of metrics)
    for (const pod of Object.values(series[m]))
      for (const [t] of pod) t0 = Math.min(t0, t);

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
    <ResponsiveContainer width="100%" height={340}>
      <LineChart data={data} margin={{ top: 6, right: 18, bottom: 6, left: 0 }}>
        <CartesianGrid stroke="#eceff5" vertical={false} />
        <XAxis
          dataKey="t"
          tickFormatter={fmt}
          stroke="#8b96ad"
          tick={{ fontSize: 11, fill: "#8b96ad" }}
          tickLine={false}
          axisLine={{ stroke: "#e4e8f0" }}
          type="number"
          domain={["dataMin", "dataMax"]}
        />
        <YAxis
          stroke="#8b96ad"
          tick={{ fontSize: 11, fill: "#8b96ad" }}
          tickLine={false}
          axisLine={false}
          width={52}
        />
        <Tooltip
          contentStyle={{
            background: "#fff",
            border: "1px solid #e4e8f0",
            borderRadius: 8,
            boxShadow: "0 6px 18px rgba(16,24,40,.08)",
            fontSize: 12,
          }}
          labelFormatter={(s) => `t + ${fmt(s as number)}`}
        />
        <Legend wrapperStyle={{ fontSize: 12, paddingTop: 6 }} iconType="plainline" />
        {metrics.map((m) => (
          <Line
            key={m}
            type="monotone"
            dataKey={m}
            name={METRIC_LABEL[m]}
            stroke={METRIC_COLORS[m]}
            dot={false}
            strokeWidth={m === "lean_finalized_slot" ? 2.6 : 1.4}
            connectNulls
            isAnimationActive={false}
          />
        ))}
      </LineChart>
    </ResponsiveContainer>
  );
}
