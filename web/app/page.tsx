"use client";

import { useEffect, useMemo, useState } from "react";
import { FIX_KEYS, FIX_LABELS } from "@/lib/fixes";

type Run = {
  run_id: string;
  started_at: number | null;
  status: string;
  client: string;
  image: string;
  devnet: string;
  flags: any;
  fixes: any;
  outcome: any;
  source: string;
};

const statusClass = (s: string) =>
  s?.startsWith("finalized") ? "ok" : s === "partial" ? "partial" : "none";

export default function Gallery() {
  const [runs, setRuns] = useState<Run[]>([]);
  const [loading, setLoading] = useState(true);
  const [q, setQ] = useState("");
  const [sort, setSort] = useState<{ k: string; dir: 1 | -1 }>({ k: "run_id", dir: -1 });

  // active facet filters
  const [fDevnet, setFDevnet] = useState<Set<string>>(new Set());
  const [fClient, setFClient] = useState<Set<string>>(new Set());
  const [fStatus, setFStatus] = useState<Set<string>>(new Set());
  const [fImage, setFImage] = useState<Set<string>>(new Set());
  const [fFixes, setFFixes] = useState<Set<string>>(new Set());

  useEffect(() => {
    fetch("/api/runs")
      .then((r) => r.json())
      .then((d) => {
        setRuns(d.runs || []);
        setLoading(false);
      });
    // hydrate filters from URL
    const p = new URLSearchParams(window.location.search);
    const get = (k: string) => new Set(p.getAll(k));
    setFDevnet(get("devnet"));
    setFClient(get("client"));
    setFStatus(get("status"));
    setFImage(get("image"));
    setFFixes(get("fix"));
    if (p.get("q")) setQ(p.get("q")!);
  }, []);

  // sync URL
  useEffect(() => {
    const p = new URLSearchParams();
    fDevnet.forEach((v) => p.append("devnet", v));
    fClient.forEach((v) => p.append("client", v));
    fStatus.forEach((v) => p.append("status", v));
    fImage.forEach((v) => p.append("image", v));
    fFixes.forEach((v) => p.append("fix", v));
    if (q) p.set("q", q);
    const s = p.toString();
    window.history.replaceState(null, "", s ? `?${s}` : window.location.pathname);
  }, [fDevnet, fClient, fStatus, fImage, fFixes, q]);

  const has = (set: Set<string>, v: string) => set.size === 0 || set.has(v);
  const hasFixes = (fixes: any) =>
    fFixes.size === 0 || [...fFixes].every((k) => !!fixes?.[k]);

  const filtered = useMemo(() => {
    let r = runs.filter(
      (x) =>
        has(fDevnet, x.devnet) &&
        has(fClient, x.client) &&
        has(fStatus, x.status) &&
        has(fImage, x.image) &&
        hasFixes(x.fixes) &&
        (!q ||
          x.run_id.includes(q) ||
          (x.image || "").includes(q) ||
          (x.client || "").includes(q))
    );
    const get = (x: Run) =>
      sort.k === "finalized"
        ? Number(x.outcome?.max_finalized ?? 0)
        : (x as any)[sort.k] ?? "";
    r = [...r].sort((a, b) => {
      const va = get(a),
        vb = get(b);
      return (va < vb ? -1 : va > vb ? 1 : 0) * sort.dir;
    });
    return r;
  }, [runs, fDevnet, fClient, fStatus, fImage, fFixes, q, sort]);

  // facet counts (computed over runs matching the OTHER filters loosely → here, all)
  const counts = (key: keyof Run) => {
    const m = new Map<string, number>();
    for (const r of runs) {
      const v = (r[key] as string) || "—";
      m.set(v, (m.get(v) || 0) + 1);
    }
    return [...m.entries()].sort((a, b) => b[1] - a[1]);
  };
  const toggle = (set: Set<string>, setter: (s: Set<string>) => void, v: string) => {
    const n = new Set(set);
    n.has(v) ? n.delete(v) : n.add(v);
    setter(n);
  };

  const Facet = ({
    label,
    count,
    active,
    onClick,
  }: {
    label: string;
    count: number;
    active: boolean;
    onClick: () => void;
  }) => (
    <div className={`facet ${active ? "active" : ""}`} onClick={onClick}>
      <span>{label}</span>
      <span className="count">{count}</span>
    </div>
  );

  const Th = ({ k, label }: { k: string; label: string }) => (
    <th
      onClick={() =>
        setSort((s) => ({ k, dir: s.k === k && s.dir === -1 ? 1 : -1 }))
      }
    >
      {label} {sort.k === k ? (sort.dir === -1 ? "▾" : "▴") : ""}
    </th>
  );

  return (
    <div className="gallery">
      <aside className="facets">
        <h3>Devnet</h3>
        {counts("devnet").map(([v, c]) => (
          <Facet
            key={v}
            label={v}
            count={c}
            active={fDevnet.has(v)}
            onClick={() => toggle(fDevnet, setFDevnet, v)}
          />
        ))}

        <h3>Client</h3>
        {counts("client").map(([v, c]) => (
          <Facet
            key={v}
            label={v}
            count={c}
            active={fClient.has(v)}
            onClick={() => toggle(fClient, setFClient, v)}
          />
        ))}

        <h3>Status</h3>
        {counts("status").map(([v, c]) => (
          <Facet
            key={v}
            label={v}
            count={c}
            active={fStatus.has(v)}
            onClick={() => toggle(fStatus, setFStatus, v)}
          />
        ))}

        <button
          className="facetReset"
          onClick={() => {
            setFDevnet(new Set());
            setFClient(new Set());
            setFStatus(new Set());
            setFImage(new Set());
            setFFixes(new Set());
            setQ("");
          }}
        >
          Reset filters
        </button>
      </aside>

      <section>
        <div className="searchrow">
          <input
            type="text"
            placeholder="search run id / image / client…"
            value={q}
            onChange={(e) => setQ(e.target.value)}
          />
        </div>
        <p className="muted">
          {loading ? "loading…" : `${filtered.length} / ${runs.length} runs`}
        </p>
        <table>
          <thead>
            <tr>
              <Th k="run_id" label="Run" />
              <Th k="devnet" label="Devnet" />
              <Th k="client" label="Client" />
              <Th k="image" label="Image" />
              <Th k="finalized" label="Finalized" />
              <Th k="status" label="Status" />
              <th>Fixes</th>
            </tr>
          </thead>
          <tbody>
            {filtered.map((r) => (
              <tr key={r.run_id}>
                <td>
                  <a href={`/runs/${r.run_id}`}>{r.run_id}</a>
                </td>
                <td>
                  <span className={`badge ${r.devnet}`}>{r.devnet}</span>
                </td>
                <td>{r.client}</td>
                <td className="muted">{(r.image || "").split(":").pop()}</td>
                <td>
                  {Number(r.outcome?.max_finalized ?? 0)}
                  {r.outcome?.stalled ? (
                    <span className="muted"> ⚠{r.outcome.stall_slot}</span>
                  ) : null}
                </td>
                <td>
                  <span className={`badge ${statusClass(r.status)}`}>{r.status}</span>
                </td>
                <td>
                  {FIX_KEYS.filter((k) => r.fixes?.[k]).map((k) => (
                    <span className="chip" key={k}>
                      {FIX_LABELS[k]}
                    </span>
                  ))}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>
    </div>
  );
}
