"use client";

import { useEffect, useMemo, useState } from "react";

type Run = {
  run_id: string;
  started_at: number | null;
  status: string;
  client: string;
  image: string;
  clients: { name: string; instances?: number; image?: string }[];
  devnet: string;
  flags: any;
  outcome: any;
  source: string;
};

// Distinct client names for a run (handles multi-client devnets; falls back to
// the legacy single `client` field).
const clientNames = (r: Run): string[] => {
  const names = (r.clients || []).map((c) => c?.name).filter(Boolean) as string[];
  const uniq = Array.from(new Set(names));
  return uniq.length ? uniq : r.client ? [r.client] : [];
};

const STATUS_META: Record<string, { cls: string; label: string }> = {
  "finalized-2k": { cls: "green", label: "finalized ≥2k" },
  finalized: { cls: "green", label: "finalized" },
  partial: { cls: "amber", label: "partial" },
  "no-finality": { cls: "slate", label: "no finality" },
};
const statusMeta = (s: string) => STATUS_META[s] || { cls: "slate", label: s || "—" };
const devnetCls = (d: string) => (d === "devnet5" ? "blue" : "violet");
const maxFin = (r: Run) => Number(r.outcome?.max_finalized ?? 0);
const BAR_MAX = 2500;

export default function Gallery() {
  const [runs, setRuns] = useState<Run[]>([]);
  const [loading, setLoading] = useState(true);
  const [q, setQ] = useState("");
  const [sort, setSort] = useState<{ k: string; dir: 1 | -1 }>({ k: "run_id", dir: -1 });

  const [fDevnet, setFDevnet] = useState<Set<string>>(new Set());
  const [fClient, setFClient] = useState<Set<string>>(new Set());
  const [fStatus, setFStatus] = useState<Set<string>>(new Set());

  useEffect(() => {
    fetch("/api/runs")
      .then((r) => r.json())
      .then((d) => {
        setRuns(d.runs || []);
        setLoading(false);
      });
    const p = new URLSearchParams(window.location.search);
    const get = (k: string) => new Set(p.getAll(k));
    setFDevnet(get("devnet"));
    setFClient(get("client"));
    setFStatus(get("status"));
    if (p.get("q")) setQ(p.get("q")!);
  }, []);

  useEffect(() => {
    const p = new URLSearchParams();
    fDevnet.forEach((v) => p.append("devnet", v));
    fClient.forEach((v) => p.append("client", v));
    fStatus.forEach((v) => p.append("status", v));
    if (q) p.set("q", q);
    const s = p.toString();
    window.history.replaceState(null, "", s ? `?${s}` : window.location.pathname);
  }, [fDevnet, fClient, fStatus, q]);

  const has = (set: Set<string>, v: string) => set.size === 0 || set.has(v);
  const anyFilter = fDevnet.size || fClient.size || fStatus.size || q;

  const clientMatch = (x: Run) =>
    fClient.size === 0 || clientNames(x).some((n) => fClient.has(n));

  const filtered = useMemo(() => {
    let r = runs.filter(
      (x) =>
        has(fDevnet, x.devnet) &&
        clientMatch(x) &&
        has(fStatus, x.status) &&
        (!q ||
          x.run_id.includes(q) ||
          clientNames(x).some((n) => n.includes(q)))
    );
    const get = (x: Run) =>
      sort.k === "finalized" ? maxFin(x) : ((x as any)[sort.k] ?? "");
    return [...r].sort((a, b) => {
      const va = get(a),
        vb = get(b);
      return (va < vb ? -1 : va > vb ? 1 : 0) * sort.dir;
    });
  }, [runs, fDevnet, fClient, fStatus, q, sort]);

  const counts = (key: keyof Run) => {
    const m = new Map<string, number>();
    for (const r of runs) m.set((r[key] as string) || "—", (m.get((r[key] as string) || "—") || 0) + 1);
    return [...m.entries()].sort((a, b) => b[1] - a[1]);
  };
  // Client facet counts a run once per distinct client it contains.
  const clientCounts = () => {
    const m = new Map<string, number>();
    for (const r of runs) for (const n of clientNames(r)) m.set(n, (m.get(n) || 0) + 1);
    return [...m.entries()].sort((a, b) => b[1] - a[1]);
  };

  // summary
  const total = runs.length;
  const d5 = runs.filter((r) => r.devnet === "devnet5").length;
  const finalizing = runs.filter((r) => r.status?.startsWith("finalized")).length;
  const best = runs.reduce((m, r) => Math.max(m, maxFin(r)), 0);

  const toggle = (set: Set<string>, setter: (s: Set<string>) => void, v: string) => {
    const n = new Set(set);
    n.has(v) ? n.delete(v) : n.add(v);
    setter(n);
  };
  const reset = () => {
    setFDevnet(new Set());
    setFClient(new Set());
    setFStatus(new Set());
    setQ("");
  };

  const Facet = ({
    label,
    count,
    active,
    onClick,
    pill,
  }: {
    label: React.ReactNode;
    count: number;
    active: boolean;
    onClick: () => void;
    pill?: string;
  }) => (
    <div className={`facet ${active ? "active" : ""}`} onClick={onClick}>
      <span>
        {pill ? <span className={`pill ${pill}`} style={{ marginRight: 0 }}>{label}</span> : label}
      </span>
      <span className="fcount">{count}</span>
    </div>
  );

  const Th = ({ k, label, cls }: { k: string; label: string; cls?: string }) =>
    cls === "nosort" ? (
      <th className="nosort">{label}</th>
    ) : (
      <th onClick={() => setSort((s) => ({ k, dir: s.k === k && s.dir === -1 ? 1 : -1 }))}>
        {label} {sort.k === k && <span className="arr">{sort.dir === -1 ? "↓" : "↑"}</span>}
      </th>
    );

  return (
    <div>
      <div className="pagehead">
        <h1>Devnet runs</h1>
        <p>Every Lean Ethereum devnet launched with leanstart — topology, metrics, and logs.</p>
      </div>

      <div className="tiles">
        <div className="tile">
          <div className="label">Total runs</div>
          <div className="value">{total}</div>
        </div>
        <div className="tile">
          <div className="label">By devnet</div>
          <div className="value sm">
            {d5} <span className="muted">devnet5</span> · {total - d5}{" "}
            <span className="muted">devnet4</span>
          </div>
          <div className="hint">protocol generations tracked</div>
        </div>
        <div className="tile">
          <div className="label">Reached finality</div>
          <div className="value">{finalizing}</div>
          <div className="hint">runs with finalized ≥ 200</div>
        </div>
        <div className="tile">
          <div className="label">Best finalized slot</div>
          <div className="value" style={{ color: "var(--green)" }}>{best.toLocaleString()}</div>
        </div>
      </div>

      <div className="gallery">
        <aside className="rail">
          <div className="railhead">
            <h2>Filters</h2>
            <button className="clear" onClick={reset} disabled={!anyFilter}>
              Clear
            </button>
          </div>

          <div className="fgroup">
            <h3>Devnet</h3>
            {counts("devnet").map(([v, c]) => (
              <Facet
                key={v}
                label={v}
                pill={devnetCls(v)}
                count={c}
                active={fDevnet.has(v)}
                onClick={() => toggle(fDevnet, setFDevnet, v)}
              />
            ))}
          </div>

          <div className="fgroup">
            <h3>Client</h3>
            {clientCounts().map(([v, c]) => (
              <Facet key={v} label={v} count={c} active={fClient.has(v)} onClick={() => toggle(fClient, setFClient, v)} />
            ))}
          </div>

          <div className="fgroup">
            <h3>Status</h3>
            {counts("status").map(([v, c]) => (
              <Facet
                key={v}
                label={statusMeta(v).label}
                count={c}
                active={fStatus.has(v)}
                onClick={() => toggle(fStatus, setFStatus, v)}
              />
            ))}
          </div>
        </aside>

        <section>
          <div className="searchbar">
            <span className="ic">⌕</span>
            <input
              type="text"
              placeholder="Search run id, image, or client…"
              value={q}
              onChange={(e) => setQ(e.target.value)}
            />
          </div>

          <div className="toolbar">
            <span className="count">
              {loading ? "Loading…" : `Showing ${filtered.length} of ${runs.length} runs`}
            </span>
          </div>

          <div className="card">
            <div className="cardbody flush">
              <div className="tablewrap">
                <table className="runs">
                  <thead>
                    <tr>
                      <Th k="run_id" label="Run" />
                      <Th k="devnet" label="Devnet" />
                      <Th k="client" label="Clients" cls="nosort" />
                      <Th k="finalized" label="Finalized" />
                      <Th k="status" label="Status" />
                    </tr>
                  </thead>
                  <tbody>
                    {loading
                      ? Array.from({ length: 8 }).map((_, i) => (
                          <tr key={i}>
                            {Array.from({ length: 5 }).map((__, j) => (
                              <td key={j}>
                                <div className="skel" style={{ width: `${50 + ((i * j) % 40)}%` }} />
                              </td>
                            ))}
                          </tr>
                        ))
                      : filtered.map((r) => {
                          const sm = statusMeta(r.status);
                          const fin = maxFin(r);
                          return (
                            <tr key={r.run_id}>
                              <td>
                                <a className="runlink" href={`/runs/${r.run_id}`}>
                                  {r.run_id}
                                </a>
                              </td>
                              <td>
                                <span className={`pill ${devnetCls(r.devnet)}`}>{r.devnet}</span>
                              </td>
                              <td className="fixcell">
                                {clientNames(r).map((n) => (
                                  <span className="chip" key={n}>
                                    {n}
                                  </span>
                                )) || "—"}
                              </td>
                              <td>
                                <div className="fincell">
                                  <span className="n">{fin.toLocaleString()}</span>
                                  <span className="bar">
                                    <i style={{ width: `${Math.min(100, (fin / BAR_MAX) * 100)}%` }} />
                                  </span>
                                  {r.outcome?.stalled ? (
                                    <span className="warnflag" title={`stalled at ${r.outcome.stall_slot}`}>
                                      ⚠
                                    </span>
                                  ) : null}
                                </div>
                              </td>
                              <td>
                                <span className={`pill ${sm.cls}`}>
                                  <span className="dot" />
                                  {sm.label}
                                </span>
                              </td>
                            </tr>
                          );
                        })}
                    {!loading && filtered.length === 0 && (
                      <tr>
                        <td colSpan={5}>
                          <div className="empty">No runs match these filters.</div>
                        </td>
                      </tr>
                    )}
                  </tbody>
                </table>
              </div>
            </div>
          </div>
        </section>
      </div>
    </div>
  );
}
