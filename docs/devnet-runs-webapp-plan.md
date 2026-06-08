# Devnet Runs Webapp — plan (filterable run explorer)

A deployable web app to browse **every** leanstart devnet run with rich **filtering** and
side-by-side comparison. Supersedes/extends `docs/run-tracker-plan.md` (read that for the
base architecture); this doc focuses on the **data model + filtering**, which is the point.

## Why
This session alone produced ~15 ream/ethlambda variants (conj-r1, offloop, tguard, bbuild,
gossipagg, session, finconsist, iso6, ab1, core, core3, c34, the 2k run, plus ethlambda).
Finding "which config finalized, how far, how fast" meant scrolling logs. A filterable run
explorer makes that one query.

## The filter dimensions (the heart of it)
Every run is tagged with facets so the UI can filter/sort/group/compare by them:

**Identity / config**
- client: `ream` | `ethlambda` | (future) zeam/grandine/…
- image tag: e.g. `snaiyer1/ream:devnet5-finconsist`
- devnet: `devnet5` | `devnet4`
- topology: node count, per-host placement (`big`/`nbg1`/`nbg2`), aggregator(s), `--all-aggregators`
- flags: `host_network`, `image_pull_policy`, subnets, validators/pod, committee count
- genesis: num_validators, genesis_time, seconds_per_slot
- **fix facets** (boolean tags per run — the key comparison axis here):
  `proving_conjectured`, `log_inv_rate`, `gossip_disparity`, `block_builder_prefilter`,
  `target_guard`, `offloop_aggregation`, `headstate_finalized`. (Derived from image/commit;
  lets you filter "runs WITH off-loop AND head-state-finalized".)
- code: ream git branch + commit sha, leanMultisig rev

**Outcome / metrics** (computed at end of run)
- status: `finalizing` | `finalized>=N` | `stalled` | `errored` | `running`
- max_finalized_slot, max_justified_slot, final head slot
- stalled? + stall slot (finalized froze) + stall duration
- time_to_finalize_200 / _1000 / _2000 (seconds)
- agg_time_avg_s (committee_signatures_aggregation), block_build_time_avg_s
- finalization lag profile (finalized-vs-head), bursty vs smooth
- duration, started_at, ended_at

**Filter UX**: faceted sidebar (checkboxes per client/flag/fix + range sliders for
finalized-slot / agg-time / date), free-text on invocation, sort by any metric, saved filters
via URL query params (shareable). "Compare selected" overlays finalization curves of N runs.

## Data model
Per-run record (one row), backed by the **`run.json`** the CLI emits (see base plan §3) plus a
`metrics.ndjson` timeseries. Postgres (Neon) schema:
```
runs(run_id pk, started_at, ended_at, status, client, image, devnet, git_sha,
     invocation, host_map jsonb, flags jsonb, genesis jsonb,
     fixes jsonb,            -- {proving_conjectured:true, offloop:true, ...}  ← faceted
     outcome jsonb,          -- {max_finalized, stalled, stall_slot, time_to_2000, agg_avg, ...}
     notes)
samples(run_id fk, t, slot, head, justified, finalized, safe_target, agg_avg)  -- for charts
logs(run_id fk, node_id, gz bytea)
```
Indexes on client, status, max_finalized_slot, started_at, and a GIN index on `fixes`/`flags`
jsonb for fast facet queries.

## Capture (so runs self-describe)
1. `leanstart run` writes `run.json` at start/end into the run dir (the keystone — base plan §3).
   Crucially include the **fixes** facet block (derive from image tag / a manifest baked into the
   ream image, or a `--run-tags k=v` CLI passthrough).
2. In-process sampler polls each pod `:8080/metrics` ~5s → `metrics.ndjson` (head/justified/
   finalized/safe_target/agg timing). Reuse the exact curl used by the monitors this session.
3. `leanstart runs push [<id>]` (token-auth) uploads run.json + metrics.ndjson + gzipped logs
   to `/api/ingest`; optional live sampling to `/api/ingest/sample` for a real-time view.
4. Backfill: an importer scans existing `output/runs/<ts>/` + logs to seed past runs (the
   prototype already parses these; reuse its parsers).

## Stack & deploy
Next.js (App Router) on **Vercel** + **Neon Postgres** (Neon available via MCP here) — one-command
deploy, shareable URL, faceted SQL queries on the jsonb facets. Charts: Recharts/visx fed by
`samples`. Alt: self-host on leannet behind Traefik (all on Hetzner). v1 access: shared password
or public read-only + token-gated ingest.

## Pages
- `/` Runs table — faceted filter sidebar + sortable columns (client, image, fixes chips,
  finalized slot, status, duration, date). URL-encoded filters (shareable).
- `/runs/[id]` Detail — config + fixes chips, finalization chart (head/justified/finalized/
  safe_target), timing panel, per-node log viewer, Grafana time-range deep link.
- `/compare?ids=a,b,c` — overlay finalization curves + diff of config/fixes/outcome (e.g.
  bbuild vs core vs finconsist → see exactly which fix moved the finalized ceiling).

## Phases
1. `run.json` + `metrics.ndjson` capture (incl. fixes facets) in leanstart — foundation.
2. Neon schema + ingest API + `leanstart runs push` + backfill importer.
3. Runs table with faceted filtering (the core ask) + detail page chart.
4. Compare view + live sampling.
5. Polish, auth, deploy to Vercel.

## Open questions
- Deploy target: Vercel+Neon (managed, public URL) vs self-host on leannet?
- Where do the `fixes` facets come from — bake a manifest into each ream image, derive from
  image tag, or pass `--run-tags` on the CLI?
- Public read-only vs password-gated?
- Backfill all historical `output/runs/` or only new runs going forward?
