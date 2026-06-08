"use client";

import { useEffect, useState } from "react";
import { FIX_KEYS, FIX_LABELS } from "@/lib/fixes";
import FinalizationChart from "./FinalizationChart";
import LogViewer from "./LogViewer";

export default function RunDetail({ params }: { params: { id: string } }) {
  const [data, setData] = useState<any>(null);
  const [err, setErr] = useState("");

  useEffect(() => {
    fetch(`/api/runs/${params.id}`)
      .then((r) => (r.ok ? r.json() : Promise.reject("not found")))
      .then(setData)
      .catch((e) => setErr(String(e)));
  }, [params.id]);

  if (err) return <p className="muted">Run not found.</p>;
  if (!data) return <p className="muted">loading…</p>;

  const { run, series, logNodes } = data;
  const o = run.outcome || {};
  const topo: any[] = Array.isArray(run.host_map) ? run.host_map : [];

  const fmtTime = (s?: number) =>
    s ? new Date(s * 1000).toISOString().replace("T", " ").replace(".000Z", "Z") : "—";
  const fmtDur = (s?: number) =>
    s == null ? "—" : `${Math.floor(s / 3600)}h${Math.floor((s % 3600) / 60)}m`;

  return (
    <div>
      <p>
        <a href="/">← all runs</a>
      </p>
      <h1 style={{ fontSize: 20, margin: "6px 0 14px" }}>
        {run.run_id}{" "}
        <span className={`badge ${run.devnet}`}>{run.devnet}</span>{" "}
        <span className="badge">{run.client}</span>
      </h1>

      <div className="panel">
        <div className="statwrap">
          <div>
            <div className="muted">finalized</div>
            <div className="stat" style={{ color: "var(--good)" }}>
              {o.max_finalized ?? 0}
            </div>
          </div>
          <div>
            <div className="muted">justified</div>
            <div className="stat">{o.max_justified ?? 0}</div>
          </div>
          <div>
            <div className="muted">head</div>
            <div className="stat">{o.max_head ?? 0}</div>
          </div>
          <div>
            <div className="muted">stalled?</div>
            <div className="stat" style={{ color: o.stalled ? "var(--warn)" : "var(--muted)" }}>
              {o.stalled ? `@${o.stall_slot}` : "no"}
            </div>
          </div>
          <div>
            <div className="muted">time→2000</div>
            <div className="stat">{fmtDur(o.time_to_2000)}</div>
          </div>
          <div>
            <div className="muted">agg avg</div>
            <div className="stat">{o.agg_avg ? `${o.agg_avg.toFixed(2)}s` : "—"}</div>
          </div>
        </div>
      </div>

      <div className="panel">
        <h2>Finalization</h2>
        <FinalizationChart series={series} />
      </div>

      <div className="grid2">
        <div className="panel">
          <h2>Config</h2>
          <div className="kv">
            <span className="k">image</span>
            <span>{run.image}</span>
            <span className="k">started</span>
            <span>{fmtTime(run.started_at)}</span>
            <span className="k">namespace</span>
            <span>{run.namespace || "—"}</span>
            <span className="k">context</span>
            <span>{run.context || "—"}</span>
            <span className="k">invocation</span>
            <span style={{ wordBreak: "break-all" }}>{run.invocation || "—"}</span>
            <span className="k">source</span>
            <span>{run.source}</span>
          </div>
          <h2 style={{ marginTop: 14 }}>Fixes</h2>
          <div>
            {FIX_KEYS.filter((k) => run.fixes?.[k]).map((k) => (
              <span className="chip" key={k}>
                {FIX_LABELS[k]}
              </span>
            ))}
            {run.fixes?.log_inv_rate ? (
              <span className="chip">rate={run.fixes.log_inv_rate}</span>
            ) : null}
            {!FIX_KEYS.some((k) => run.fixes?.[k]) && (
              <span className="muted">none recorded</span>
            )}
          </div>
        </div>

        <div className="panel">
          <h2>Topology</h2>
          {topo.length ? (
            <table>
              <thead>
                <tr>
                  <th>node</th>
                  <th>pod</th>
                  <th>host</th>
                  <th>agg?</th>
                  <th>vals</th>
                </tr>
              </thead>
              <tbody>
                {topo.map((n, i) => (
                  <tr key={i}>
                    <td>{n.node_id || n.name}</td>
                    <td className="muted">{n.pod || "—"}</td>
                    <td>{n.host || "—"}</td>
                    <td>{n.is_aggregator ? "★" : ""}</td>
                    <td>{n.validator_count ?? n.instances ?? "—"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : (
            <p className="muted">No topology recorded.</p>
          )}
          {run.genesis?.genesis_time ? (
            <div className="kv" style={{ marginTop: 12 }}>
              <span className="k">genesis_time</span>
              <span>{fmtTime(run.genesis.genesis_time)}</span>
              <span className="k">validators</span>
              <span>{run.genesis.num_validators ?? "—"}</span>
            </div>
          ) : null}
        </div>
      </div>

      <div className="panel">
        <h2>Logs</h2>
        <LogViewer id={run.run_id} nodes={logNodes} />
      </div>
    </div>
  );
}
