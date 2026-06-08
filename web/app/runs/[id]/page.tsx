"use client";

import { useEffect, useState } from "react";
import FinalizationChart from "./FinalizationChart";
import LogViewer from "./LogViewer";

const STATUS_META: Record<string, { cls: string; label: string }> = {
  "finalized-2k": { cls: "green", label: "finalized ≥2k" },
  finalized: { cls: "green", label: "finalized" },
  partial: { cls: "amber", label: "partial" },
  "no-finality": { cls: "slate", label: "no finality" },
};

export default function RunDetail({ params }: { params: { id: string } }) {
  const [data, setData] = useState<any>(null);
  const [err, setErr] = useState("");

  useEffect(() => {
    fetch(`/api/runs/${params.id}`)
      .then((r) => (r.ok ? r.json() : Promise.reject("not found")))
      .then(setData)
      .catch((e) => setErr(String(e)));
  }, [params.id]);

  if (err)
    return (
      <div className="card">
        <div className="empty">Run not found.</div>
      </div>
    );
  if (!data)
    return (
      <div>
        <div className="crumb">
          <a href="/">Runs</a> / {params.id}
        </div>
        <div className="card">
          <div className="cardbody">
            <div className="skel" style={{ width: "30%", height: 18, marginBottom: 14 }} />
            <div className="skel" style={{ width: "100%", height: 280 }} />
          </div>
        </div>
      </div>
    );

  const { run, series, logNodes } = data;
  const o = run.outcome || {};
  const topo: any[] = Array.isArray(run.host_map) ? run.host_map : [];
  // Aggregate client allocations by name (a devnet may declare the same client
  // several times, e.g. ream:1@big ream:1@nbg1; and multi-client runs list each).
  const rawClients: { name: string; instances?: number; image?: string }[] =
    Array.isArray(run.clients) && run.clients.length
      ? run.clients
      : run.client
      ? [{ name: run.client, image: run.image }]
      : [];
  const clients = Object.values(
    rawClients.reduce((acc: Record<string, any>, c) => {
      const k = c.name;
      acc[k] = acc[k] || { name: k, instances: 0, image: c.image };
      acc[k].instances += c.instances ?? 1;
      acc[k].image = acc[k].image || c.image;
      return acc;
    }, {})
  ) as { name: string; instances: number; image?: string }[];
  const sm = STATUS_META[run.status] || { cls: "slate", label: run.status || "—" };
  const devnetCls = run.devnet === "devnet5" ? "blue" : "violet";

  const fmtTime = (s?: number) =>
    s ? new Date(s * 1000).toISOString().replace("T", " ").replace(".000Z", " UTC") : "—";
  const fmtDur = (s?: number) =>
    s == null ? "—" : `${Math.floor(s / 3600)}h ${Math.floor((s % 3600) / 60)}m`;

  return (
    <div>
      <div className="crumb">
        <a href="/">Runs</a> / {run.run_id}
      </div>

      <div className="pagehead" style={{ display: "flex", alignItems: "center", gap: 12, flexWrap: "wrap" }}>
        <h1 className="mono" style={{ fontSize: 19 }}>
          {run.run_id}
        </h1>
        <span className={`pill ${devnetCls}`}>{run.devnet}</span>
        <span className={`pill ${sm.cls}`}>
          <span className="dot" />
          {sm.label}
        </span>
        {clients.map((c) => (
          <span className="pill slate" key={c.name}>
            {c.name}
            {c.instances ? ` ×${c.instances}` : ""}
          </span>
        ))}
      </div>

      {/* stat row */}
      <div className="card" style={{ marginBottom: 16 }}>
        <div className="statrow">
          <div className="s">
            <div className="label">Finalized</div>
            <div className="v green">{(o.max_finalized ?? 0).toLocaleString()}</div>
          </div>
          <div className="s">
            <div className="label">Justified</div>
            <div className="v">{(o.max_justified ?? 0).toLocaleString()}</div>
          </div>
          <div className="s">
            <div className="label">Head</div>
            <div className="v">{(o.max_head ?? 0).toLocaleString()}</div>
          </div>
          <div className="s">
            <div className="label">Stalled</div>
            <div className={`v ${o.stalled ? "amber" : "muted"}`}>
              {o.stalled ? `@ ${o.stall_slot}` : "no"}
            </div>
          </div>
          <div className="s">
            <div className="label">Time → 2000</div>
            <div className="v">{fmtDur(o.time_to_2000)}</div>
          </div>
          <div className="s">
            <div className="label">Agg avg</div>
            <div className="v">{o.agg_avg ? `${o.agg_avg.toFixed(2)}s` : "—"}</div>
          </div>
        </div>
      </div>

      {/* chart */}
      <div className="card" style={{ marginBottom: 16 }}>
        <div className="cardhead">
          <h2>Finalization</h2>
          <span className="faint" style={{ fontSize: 12 }}>max-over-pods · time from genesis</span>
        </div>
        <div className="cardbody">
          <FinalizationChart series={series} />
        </div>
      </div>

      <div className="detailgrid">
        {/* config */}
        <div className="card">
          <div className="cardhead">
            <h2>Configuration</h2>
          </div>
          <div className="cardbody">
            <div className="kv">
              <span className="k">Clients</span>
              <span className="val">
                {clients.length ? (
                  clients.map((c) => (
                    <div key={c.name} style={{ marginBottom: 4 }}>
                      <span className="pill slate" style={{ marginRight: 6 }}>
                        {c.name}
                        {c.instances ? ` ×${c.instances}` : ""}
                      </span>
                      <span className="mono faint" style={{ fontSize: 11.5 }}>
                        {c.image || "—"}
                      </span>
                    </div>
                  ))
                ) : (
                  "—"
                )}
              </span>
              <span className="k">Started</span>
              <span className="val">{fmtTime(run.started_at)}</span>
              <span className="k">Namespace</span>
              <span className="val">{run.namespace || "—"}</span>
              <span className="k">Context</span>
              <span className="val">{run.context || "—"}</span>
              <span className="k">Source</span>
              <span className="val">{run.source}</span>
              <span className="k">Invocation</span>
              <span className="val mono" style={{ fontSize: 11.5 }}>
                {run.invocation || "—"}
              </span>
            </div>
          </div>
        </div>

        {/* topology */}
        <div className="card">
          <div className="cardhead">
            <h2>Topology</h2>
            <span className="faint" style={{ fontSize: 12 }}>{topo.length} node{topo.length === 1 ? "" : "s"}</span>
          </div>
          <div className="cardbody flush">
            {topo.length ? (
              <table className="mini">
                <thead>
                  <tr>
                    <th>Node</th>
                    <th>Pod</th>
                    <th>Host</th>
                    <th>Agg</th>
                    <th>Vals</th>
                  </tr>
                </thead>
                <tbody>
                  {topo.map((n, i) => (
                    <tr key={i}>
                      <td className="mono">{n.node_id || n.name}</td>
                      <td className="mono faint">{n.pod || "—"}</td>
                      <td>{n.host || "—"}</td>
                      <td>{n.is_aggregator ? <span className="pill blue" style={{ padding: "0 7px" }}>★</span> : ""}</td>
                      <td>{n.validator_count ?? n.instances ?? "—"}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            ) : (
              <div className="empty">No topology recorded.</div>
            )}
            {run.genesis?.genesis_time ? (
              <div style={{ padding: "12px 18px", borderTop: "1px solid var(--border)" }}>
                <div className="kv">
                  <span className="k">Genesis time</span>
                  <span className="val">{fmtTime(run.genesis.genesis_time)}</span>
                  <span className="k">Validators</span>
                  <span className="val">{run.genesis.num_validators ?? "—"}</span>
                </div>
              </div>
            ) : null}
          </div>
        </div>
      </div>

      {/* logs */}
      <div className="card" style={{ marginTop: 16 }}>
        <div className="cardhead">
          <h2>Logs</h2>
        </div>
        <LogViewer id={run.run_id} nodes={logNodes} />
      </div>
    </div>
  );
}
