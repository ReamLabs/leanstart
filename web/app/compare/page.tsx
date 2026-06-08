"use client";

import { useEffect, useState } from "react";
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
import { FIX_KEYS, FIX_LABELS } from "@/lib/fixes";

const COLORS = ["#5eead4", "#818cf8", "#fbbf24", "#f87171", "#4ade80", "#f472b6"];

// Overlay the finalized-slot curve of N runs (max-over-pods, t from start).
export default function Compare() {
  const [ids, setIds] = useState<string[]>([]);
  const [runs, setRuns] = useState<any[]>([]);
  const [allRuns, setAllRuns] = useState<any[]>([]);

  useEffect(() => {
    const p = new URLSearchParams(window.location.search);
    const idList = (p.get("ids") || "").split(",").filter(Boolean);
    setIds(idList);
    fetch("/api/runs")
      .then((r) => r.json())
      .then((d) => setAllRuns(d.runs || []));
    Promise.all(
      idList.map((id) => fetch(`/api/runs/${id}`).then((r) => (r.ok ? r.json() : null)))
    ).then((rs) => setRuns(rs.filter(Boolean)));
  }, []);

  const setSelection = (idList: string[]) => {
    const s = idList.join(",");
    window.history.replaceState(null, "", s ? `?ids=${s}` : window.location.pathname);
    setIds(idList);
    Promise.all(
      idList.map((id) => fetch(`/api/runs/${id}`).then((r) => (r.ok ? r.json() : null)))
    ).then((rs) => setRuns(rs.filter(Boolean)));
  };

  // build merged finalized series keyed by t (seconds), one column per run.
  const byT = new Map<number, any>();
  for (const r of runs) {
    const pods = r.series?.lean_finalized_slot || {};
    let t0 = Infinity;
    for (const p of Object.values<any>(pods)) for (const [t] of p) t0 = Math.min(t0, t);
    const merged = new Map<number, number>();
    for (const p of Object.values<any>(pods))
      for (const [t, v] of p) merged.set(t, Math.max(merged.get(t) ?? -Infinity, v));
    for (const [t, v] of merged) {
      const sec = Math.round(t - t0);
      const row = byT.get(sec) ?? { t: sec };
      row[r.run.run_id] = v;
      byT.set(sec, row);
    }
  }
  const data = [...byT.values()].sort((a, b) => a.t - b.t);
  const fmt = (s: number) => `${Math.floor(s / 3600)}h${Math.floor((s % 3600) / 60)}m`;

  return (
    <div>
      <h1 style={{ fontSize: 20 }}>Compare runs</h1>
      <p className="muted">
        Pick runs to overlay their finalization curves. Shareable via URL.
      </p>

      <div className="panel">
        <h2>Select runs</h2>
        <div className="btnrow">
          {allRuns.slice(0, 60).map((r) => (
            <button
              key={r.run_id}
              className={`btn ${ids.includes(r.run_id) ? "active" : ""}`}
              onClick={() =>
                setSelection(
                  ids.includes(r.run_id)
                    ? ids.filter((x) => x !== r.run_id)
                    : [...ids, r.run_id]
                )
              }
              title={r.image}
            >
              {(r.image || "").split(":").pop()} · {r.outcome?.max_finalized ?? 0}
            </button>
          ))}
        </div>
      </div>

      {runs.length > 0 && (
        <>
          <div className="panel">
            <h2>Finalized slot over time</h2>
            <ResponsiveContainer width="100%" height={380}>
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
                {runs.map((r, i) => (
                  <Line
                    key={r.run.run_id}
                    type="monotone"
                    dataKey={r.run.run_id}
                    name={(r.run.image || "").split(":").pop()}
                    stroke={COLORS[i % COLORS.length]}
                    dot={false}
                    strokeWidth={2}
                    connectNulls
                    isAnimationActive={false}
                  />
                ))}
              </LineChart>
            </ResponsiveContainer>
          </div>

          <div className="panel">
            <h2>Config diff</h2>
            <table>
              <thead>
                <tr>
                  <th>run</th>
                  <th>devnet</th>
                  <th>image</th>
                  <th>finalized</th>
                  <th>stalled</th>
                  <th>fixes</th>
                </tr>
              </thead>
              <tbody>
                {runs.map((r) => (
                  <tr key={r.run.run_id}>
                    <td>
                      <a href={`/runs/${r.run.run_id}`}>{r.run.run_id}</a>
                    </td>
                    <td>
                      <span className={`badge ${r.run.devnet}`}>{r.run.devnet}</span>
                    </td>
                    <td className="muted">{(r.run.image || "").split(":").pop()}</td>
                    <td>{r.run.outcome?.max_finalized ?? 0}</td>
                    <td>{r.run.outcome?.stalled ? `@${r.run.outcome.stall_slot}` : "no"}</td>
                    <td>
                      {FIX_KEYS.filter((k) => r.run.fixes?.[k]).map((k) => (
                        <span className="chip" key={k}>
                          {FIX_LABELS[k]}
                        </span>
                      ))}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}
    </div>
  );
}
