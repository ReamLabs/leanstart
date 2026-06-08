/**
 * Shared run-ingest core used by both the backfill importer (reads from the
 * filesystem) and the /api/ingest route (reads from an uploaded tarball). Given
 * the parsed files for a run, it builds the `runs` row + samples + logs and
 * upserts them into Neon.
 */

export type RunFiles = {
  runJson: any | null;
  metricsJson: any | null;
  runLog: string;
  logs: Record<string, string>; // node_id -> raw log body
};

type SampleRow = { metric: string; pod: string; t: number; v: number };

const ANSI = /\x1b\[[0-9;]*m/g;
const TS = /(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2})/;
const RE_HEAD = /Head Slot:\s*(\d+)/;
const RE_JUST = /Latest Justified:\s*Slot\s*(\d+)/;
const RE_FIN = /Latest Finalized:\s*Slot\s*(\d+)/;
const RE_E_HEAD = /our_head_slot=(\d+)/;
const RE_E_FIN = /our_finalized_slot=(\d+)/;
export const strip = (s: string) => s.replace(ANSI, "");
const toEpoch = (iso: string) => Math.floor(Date.parse(iso + "Z") / 1000);

/**
 * Derive fix facets from a ream image tag — the TS mirror of
 * `fixes_for_image` in leanstart's src/run_record.rs, so backfilled runs (which
 * have no run.json) still get their fix chips.
 */
export function fixesForImage(image: string) {
  const tag = (image || "").split(":").pop() || "";
  // Fix flags: #0 gossip_disparity, #1 proving_conjectured(+rate), #3
  // block_builder_prefilter, #4 target_guard, #5 offloop_aggregation,
  // #6 headstate_finalized. The incremental builds are cumulative in discovery
  // order; isolation builds are exact (see the ream-fix-isolation matrix).
  const f = (
    keys: string[],
    log_inv_rate = 1
  ): Record<string, boolean | number> => {
    const o: Record<string, boolean | number> = {};
    for (const k of keys) o[k] = true;
    if (log_inv_rate) o.log_inv_rate = log_inv_rate;
    return o;
  };
  const P = "proving_conjectured",
    GD = "gossip_disparity",
    BB = "block_builder_prefilter",
    TG = "target_guard",
    OL = "offloop_aggregation",
    HS = "headstate_finalized";
  switch (tag) {
    // Stock bases — no fixes.
    case "latest-devnet5":
    case "devnet5":
      return {};
    // Incremental discovery builds (cumulative).
    case "devnet5-conj-r1":
      return f([P]);
    case "devnet5-offloop":
      return f([P, OL]);
    case "devnet5-tguard":
      return f([P, OL, TG]);
    case "devnet5-bbuild":
    case "devnet5-gossipagg":
    case "devnet5-session":
    case "devnet5-rebased":
      return f([P, OL, TG, BB]); // approx for gossipagg/session/rebased
    case "devnet5-gossip64":
      return f([P, OL, TG, BB, GD]);
    // The working full set.
    case "devnet5-finconsist":
      return f([P, GD, BB, TG, OL, HS], 1);
    case "devnet5-r2":
    case "devnet5-r2-gd1":
      return f([P, GD, BB, TG, OL, HS], 2);
    // Isolation builds (exact).
    case "devnet5-iso6":
      return f([HS]);
    case "devnet5-ab1": // full minus #1 (proven, not conjectured), rate2
      return { ...f([GD, BB, TG, OL, HS], 2), proving_conjectured: false };
    case "devnet5-core":
      return f([P, OL, HS]);
    case "devnet5-core3":
      return f([P, BB, OL, HS]);
    case "devnet5-c34": // full minus #0
      return f([P, BB, TG, OL, HS]);
    default:
      return {};
  }
}

export function parseSpec(runLog: string) {
  const spec = { clients: [] as any[], devnet: "devnet4", invocation: "" };
  for (const raw of runLog.split("\n")) {
    const line = strip(raw).replace(/\r$/, "");
    const m = line.match(/^\s+([a-z]+) x(\d+) \((.+)\)\s*$/);
    if (m) spec.clients.push({ name: m[1], instances: +m[2], image: m[3] });
    if (line.includes("devnet5") || line.includes("--devnet5")) spec.devnet = "devnet5";
    if (line.includes("leanstart run") || line.includes("leanstart ream"))
      spec.invocation = line.trim();
  }
  return spec;
}

type Pt = { t: number; head: number; justified: number | null; finalized: number };
export function parseTimeline(log: string): Pt[] {
  const pts: Pt[] = [];
  let lastTs: number | null = null;
  let pending: any = null;
  for (const raw of log.split("\n")) {
    const line = strip(raw);
    const tsm = line.match(TS);
    if (tsm) lastTs = toEpoch(tsm[1]);
    const mh = line.match(RE_HEAD),
      mj = line.match(RE_JUST),
      mf = line.match(RE_FIN);
    if (mh) pending = { t: lastTs, head: +mh[1] };
    if (mj && pending) pending.justified = +mj[1];
    if (mf && pending) {
      pending.finalized = +mf[1];
      if (pending.t != null) pts.push(pending);
      pending = null;
    }
    const eh = line.match(RE_E_HEAD),
      ef = line.match(RE_E_FIN);
    if (eh && ef && lastTs != null)
      pts.push({ t: lastTs, head: +eh[1], justified: null, finalized: +ef[1] });
  }
  return pts;
}

/** Upsert one run (row + samples + logs). `sql` is a neon tagged-template fn. */
export async function ingestRun(sql: any, id: string, files: RunFiles) {
  const { runJson, metricsJson, runLog, logs } = files;
  const spec = parseSpec(runLog || "");
  const samples: SampleRow[] = [];

  let row: any;
  if (runJson) {
    const ream =
      runJson.clients?.find((c: any) => c.name === "ream") || runJson.clients?.[0];
    row = {
      run_id: id,
      started_at: runJson.started_at ?? null,
      captured_at: runJson.captured_at ?? null,
      client: ream?.name ?? null,
      image: ream?.image ?? null,
      clients: (runJson.clients ?? []).map((c: any) => ({
        name: c.name,
        instances: c.instances,
        image: c.image,
      })),
      devnet: runJson.devnet ?? spec.devnet,
      invocation: runJson.invocation ?? spec.invocation,
      namespace: runJson.namespace ?? null,
      context: runJson.context ?? null,
      host_map: runJson.topology ?? [],
      flags: runJson.flags ?? {},
      genesis: runJson.genesis ?? {},
      fixes: runJson.fixes ?? {},
      notes: runJson.notes ?? null,
      source: "leanstart",
    };
  } else {
    const c0 = spec.clients[0];
    row = {
      run_id: id,
      started_at: null,
      captured_at: null,
      client: c0?.name ?? null,
      image: c0?.image ?? null,
      clients: spec.clients.map((c: any) => ({
        name: c.name,
        instances: c.instances,
        image: c.image,
      })),
      devnet: spec.devnet,
      invocation: spec.invocation,
      namespace: null,
      context: null,
      host_map: spec.clients,
      flags: {},
      genesis: {},
      fixes: c0 ? fixesForImage(c0.image) : {},
      notes: c0 ? `image=${c0.image}` : null,
      source: "import",
    };
  }

  if (metricsJson?.series) {
    for (const [metric, podSeries] of Object.entries<any>(metricsJson.series)) {
      for (const s of podSeries as any[])
        for (const [t, v] of s.values as [number, number][])
          samples.push({ metric, pod: s.pod, t, v });
    }
  } else {
    for (const [node, body] of Object.entries(logs)) {
      // A single streamed log can concatenate multiple chain incarnations
      // (redeploys reusing the pod). Keep only the last segment — the points
      // after the final genesis reset (a drop in finalized) — so a run reflects
      // its own chain, not an earlier one's ceiling.
      for (const p of lastChainSegment(parseTimeline(body))) {
        samples.push({ metric: "lean_head_slot", pod: node, t: p.t, v: p.head });
        if (p.justified != null)
          samples.push({ metric: "lean_justified_slot", pod: node, t: p.t, v: p.justified });
        samples.push({ metric: "lean_finalized_slot", pod: node, t: p.t, v: p.finalized });
      }
    }
  }

  // Downsample each (metric,pod) series to keep the DB small (free-tier 512MB).
  // Log-derived timelines emit a point every slot → tens of thousands of rows;
  // ~500 points per series is plenty for charting. Always keep the last point.
  const downsampled = downsample(samples, 500);

  const finVals = samples.filter((s) => s.metric === "lean_finalized_slot").map((s) => s.v);
  const justVals = samples.filter((s) => s.metric === "lean_justified_slot").map((s) => s.v);
  const headVals = samples.filter((s) => s.metric === "lean_head_slot").map((s) => s.v);
  const maxFin = finVals.length ? Math.max(...finVals) : 0;
  const outcome = metricsJson?.outcome ?? {
    max_finalized: maxFin,
    max_justified: justVals.length ? Math.max(...justVals) : 0,
    max_head: headVals.length ? Math.max(...headVals) : 0,
    stalled: false,
  };
  row.outcome = outcome;
  const mf = outcome.max_finalized ?? maxFin;
  row.status =
    mf >= 2000 ? "finalized-2k" : mf >= 200 ? "finalized" : mf > 0 ? "partial" : "no-finality";

  await sql`
    INSERT INTO runs (run_id, started_at, captured_at, status, client, image, clients, devnet,
                      invocation, namespace, context, host_map, flags, genesis, fixes,
                      outcome, notes, source)
    VALUES (${row.run_id}, ${row.started_at}, ${row.captured_at}, ${row.status},
            ${row.client}, ${row.image}, ${JSON.stringify(row.clients ?? [])}, ${row.devnet},
            ${row.invocation}, ${row.namespace},
            ${row.context}, ${JSON.stringify(row.host_map)}, ${JSON.stringify(row.flags)},
            ${JSON.stringify(row.genesis)}, ${JSON.stringify(row.fixes)},
            ${JSON.stringify(row.outcome)}, ${row.notes}, ${row.source})
    ON CONFLICT (run_id) DO UPDATE SET
      started_at=EXCLUDED.started_at, captured_at=EXCLUDED.captured_at, status=EXCLUDED.status,
      client=EXCLUDED.client, image=EXCLUDED.image, clients=EXCLUDED.clients, devnet=EXCLUDED.devnet,
      invocation=EXCLUDED.invocation, namespace=EXCLUDED.namespace, context=EXCLUDED.context,
      host_map=EXCLUDED.host_map, flags=EXCLUDED.flags, genesis=EXCLUDED.genesis,
      fixes=EXCLUDED.fixes, outcome=EXCLUDED.outcome, notes=EXCLUDED.notes, source=EXCLUDED.source
  `;

  await sql`DELETE FROM samples WHERE run_id=${id}`;
  if (downsampled.length) {
    await sql`
      INSERT INTO samples (run_id, metric, pod, t, v)
      SELECT ${id}, x.metric, x.pod, x.t, x.v
      FROM jsonb_to_recordset(${JSON.stringify(downsampled)}::jsonb)
        AS x(metric text, pod text, t double precision, v double precision)
    `;
  }

  await sql`DELETE FROM logs WHERE run_id=${id}`;
  for (let [node, body] of Object.entries(logs)) {
    body = strip(body);
    // Postgres TOAST-compresses text, but cap the tail to bound storage.
    if (body.length > 600_000) body = body.slice(-600_000);
    await sql`
      INSERT INTO logs (run_id, node_id, body) VALUES (${id}, ${node}, ${body})
      ON CONFLICT (run_id, node_id) DO UPDATE SET body=EXCLUDED.body
    `;
  }

  return {
    id,
    status: row.status,
    maxFin: mf,
    samples: downsampled.length,
    logs: Object.keys(logs).length,
  };
}

/** Keep only the points after the last genesis reset (finalized drop). */
function lastChainSegment(pts: Pt[]): Pt[] {
  let start = 0;
  for (let i = 1; i < pts.length; i++) {
    // A drop in finalized (beyond a tiny tolerance) marks a new chain.
    if (pts[i].finalized < pts[i - 1].finalized) start = i;
  }
  return pts.slice(start);
}

/** Keep ~max points per (metric,pod) series, preserving first and last. */
function downsample(rows: SampleRow[], max: number): SampleRow[] {
  const groups = new Map<string, SampleRow[]>();
  for (const r of rows) {
    const k = `${r.metric} ${r.pod}`;
    (groups.get(k) ?? groups.set(k, []).get(k)!).push(r);
  }
  const out: SampleRow[] = [];
  for (const g of groups.values()) {
    g.sort((a, b) => a.t - b.t);
    if (g.length <= max) {
      out.push(...g);
      continue;
    }
    const step = Math.ceil(g.length / max);
    for (let i = 0; i < g.length; i += step) out.push(g[i]);
    if (out[out.length - 1] !== g[g.length - 1]) out.push(g[g.length - 1]);
  }
  return out;
}
