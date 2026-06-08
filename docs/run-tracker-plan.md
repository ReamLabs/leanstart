# leanstart Run Tracker — Implementation Plan

A complete, shareable, deployable web app that records **every** `leanstart` devnet
run and displays all information collected during it (config, validator set,
finalization trajectory, proving/aggregation timing, per-node logs, outcome).

This is a planning doc only. Nothing here is built yet.

---

## 1. Goals & non-goals

**Goals**
- Every `leanstart run` is automatically captured as a structured, permanent record.
- A public, shareable web app: list of runs + a rich detail page per run.
- Detail page shows: invocation/config, client+host allocation, image tags, genesis,
  finalization trajectory (head/justified/finalized over time), proving/aggregation
  timing, peer mesh, errors, and per-node logs.
- Cross-run comparison: filter/sort by client, image tag, flags; spot regressions
  (e.g. ream `offloop` vs `tguard` vs ethlambda; finalized-slot reached, time-to-finality).
- Deployable by you with one command; shareable via URL.

**Non-goals (v1)**
- Driving/launching runs from the web UI (read-only tracker; launching stays in the CLI).
- Auth/multi-tenant (v1 is single-project, optionally behind a shared password).
- Replacing Grafana for deep metric exploration (we link out to it for time ranges).

---

## 2. Data sources (what a run already produces)

| Source | Location | Content |
|---|---|---|
| Run logs | `./output/runs/<ts>/{<client>_<n>.log, run.log}` | per-node + orchestrator logs |
| Genesis | `./output/genesis/{config.yaml, annotated_validators.yaml, nodes.yaml}` | validator set, genesis time, keys metadata |
| Invocation | CLI args | clients, `@host` placement, `--devnet5`, image tags, flags |
| Live metrics | Prometheus (`lean_*`) via kube-prometheus-stack | head/justified/finalized slot, agg timing histograms, peers, attestations |
| Cluster | `kubectl`/helm | namespace, pod→node placement, image actually pulled |

**The gap today:** none of this is recorded as one queryable *run record*. It's scattered
across the output dir + Prometheus (limited retention, no notion of "which run").

---

## 3. The keystone: a run manifest (`run.json`)

The single highest-leverage change. `leanstart run` writes `run.json` into the run dir
at **start** and updates it at **end**. Everything downstream keys off this.

```jsonc
{
  "run_id": "2026-06-07_10-37-08",          // = run dir name; primary key
  "started_at": "2026-06-07T16:37:08Z",
  "ended_at":   "2026-06-07T16:52:10Z",
  "status": "running | finalized | stalled | error | torn_down",
  "leanstart_git_sha": "…",
  "invocation": "ream:1@big ream:1@nbg1 ream:1@nbg2 --devnet5 --host-network …",
  "spec": {
    "devnet5": true, "host_network": true, "skip_metrics": true,
    "image_pull_policy": "Always", "subnets": 1, "validators_per_pod": 1
  },
  "clients": [
    { "name": "ream", "image": "snaiyer1/ream:devnet5-tguard",
      "node_id": "ream_0", "host": "big", "is_aggregator": true,
      "validator_indices": [0] }
    // …per pod
  ],
  "genesis": { "genesis_time": 1780850328, "num_validators": 3 },
  "outcome": {                                  // filled at end / by snapshotter
    "max_finalized_slot": 216,
    "max_justified_slot": 219,
    "head_slot_at_end": 222,
    "time_to_finalize_200_s": 840,
    "agg_time_avg_s": 1.18,
    "block_build_time_avg_s": 0.009,
    "missing_parent_warnings": 0,
    "goal": { "target_finalized": 200, "met": true }
  },
  "timeseries_ref": "metrics.ndjson",          // sampled metrics over the run
  "log_files": ["ream_0.log", "ream_1.log", "ream_2.log", "run.log"],
  "notes": "off-loop + conjectured + rate1 + target-guard"
}
```

Plus a **`metrics.ndjson`** in the run dir — one JSON line per sample
(`{t, slot, head, justified, finalized, agg_time_avg, peers, …}`), written by a
lightweight in-process sampler (reuse the exact `:8080/metrics` poll from the
finalization monitors used in this session). This makes the finalization chart a
pure client-side render with no Prometheus dependency or retention worry.

**Implementation in `leanstart`** (`src/cli/run.rs`): a `RunManifest` struct built
from `DevnetSpec` + `ValidatorConfig` + resolved images; `write_manifest()` at start;
a spawned sampler task polling each pod's `:8080/metrics` every ~5s appending to
`metrics.ndjson`; `finalize_manifest()` on exit computing the `outcome` block.
~150–200 lines, no new deps (serde already present).

---

## 4. Architecture

```
leanstart run ──writes──> output/runs/<id>/{run.json, metrics.ndjson, *.log}
                                   │
                          (uploader: `leanstart runs push` OR a watcher)
                                   ▼
                         ┌───────────────────┐
                         │  API (ingest+read) │  ── Postgres (Neon)
                         └───────────────────┘     • runs (manifest cols + jsonb)
                                   ▲                • samples (timeseries)
                                   │                • logs in object storage / DB
                         ┌───────────────────┐
                         │  Web app (Next.js) │  list + detail + compare
                         └───────────────────┘
```

**Stack (chosen for "shareable + easy deploy"):**
- **DB:** Neon Postgres (already available in this environment via the Neon MCP — I can
  provision a project/branch and schema directly).
- **App:** Next.js (App Router) — SSR list/detail pages + a thin API route for ingest.
  Deploys to Vercel in one click; pairs natively with Neon.
- **Logs:** small runs → store gzipped log text in a `logs` table or Neon; larger →
  an object store (Vercel Blob / S3) with a URL in the manifest. v1: gzip into DB
  (a 3-node run's logs are a few MB).
- **Charts:** Recharts/visx on the client, fed by `samples`.
- **Ingest auth:** a shared `INGEST_TOKEN` header so only your machine can push runs.

**Schema (sketch)**
```sql
runs(    run_id pk, started_at, ended_at, status, git_sha, invocation,
         spec jsonb, clients jsonb, genesis jsonb, outcome jsonb, notes )
samples( run_id fk, t, slot, head, justified, finalized, agg_time_avg, peers )
logs(    run_id fk, node_id, gz bytea )
```

---

## 5. Ingest path

Two options (can ship both):
- **Push (default):** `leanstart runs push [<run_id>]` POSTs `run.json` + `metrics.ndjson`
  + gzipped logs to `/api/ingest` with the token. Run it after a run, or auto-call it
  at the end of `leanstart run`.
- **Live:** the in-process sampler POSTs samples every ~5s to `/api/ingest/sample` so
  the web app shows the *current* run climbing in real time (covers the "live monitor"
  use case too).

---

## 6. Web app pages

- **`/` Runs list** — table: id, time, clients, image tag(s), flags, finalized slot,
  goal pass/fail, duration. Filter by client/image/flag; sort by finalized slot.
- **`/runs/[id]` Detail** —
  - Header: status badge, invocation, git sha, images, host/aggregator map.
  - **Finalization chart**: head/justified/finalized vs slot (the key visual).
  - Timing panel: agg time avg/histogram, block build time, missing-parent count.
  - Validator/peer panel: who's aggregator, placement, peer mesh.
  - Log viewer: per-node, searchable, ANSI-stripped, with a "jump to first error".
  - Link out to Grafana for the run's exact time window.
- **`/compare?ids=a,b`** — overlay finalization curves + a diff of spec/outcome.

---

## 7. Deployment

- **App + DB:** Vercel (Next.js) + Neon Postgres — both free-tier friendly, one-command
  deploy, public shareable URL. `DATABASE_URL` + `INGEST_TOKEN` as env vars.
- **Alt (self-host on leannet):** containerize the Next.js app + run Postgres on the
  cluster; expose via the existing Traefik ingress. Use if you want everything on Hetzner.
- Recommended v1: **Vercel + Neon** (least ops, instantly shareable).

---

## 8. Phased build

1. **Manifest capture** (`leanstart`): `run.json` + `metrics.ndjson` + outcome. *(foundation; everything depends on it)*
2. **DB + ingest API**: Neon schema; `/api/ingest`; `leanstart runs push`.
3. **Web list + detail** (SSR) with the finalization chart + log viewer.
4. **Live sampling** to `/api/ingest/sample` for real-time current-run view.
5. **Compare** view + filters; polish; deploy to Vercel.

Estimate: Phase 1 ≈ half a day; Phases 2–3 ≈ 1–2 days; Phases 4–5 ≈ 1 day.

---

## 9. Open questions to settle before building

- Vercel+Neon (managed, public) vs self-host on leannet (all on Hetzner)?
- Retention/volume: how many runs/day? (drives logs-in-DB vs object store).
- Live real-time view needed in v1, or post-hoc archive enough?
- Backfill: do you want past runs imported, or only new runs going forward?
- Access: fully public, or behind a shared password?
