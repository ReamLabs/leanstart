# leanstart devnets — showcase site

A public, filterable web app for every Lean Ethereum devnet run produced by
`leanstart`. Browse runs by **devnet (devnet4/devnet5)**, client, image, status,
and **fix facets**; open a run to see its topology, finalization chart, outcome
stats, and full per-node logs. Clean, DigitalOcean-inspired UI.

## Stack
- **Next.js 14** (App Router) — gallery + run-detail pages + API routes.
- **Neon Postgres** — `runs` / `samples` / `logs` (see `lib/db.ts`).
- **Recharts** — finalization charts.

## How data gets here
Each `leanstart run` writes `run.json` into its run dir (topology, flags, images,
genesis, fix facets). `leanstart runs snapshot` adds `metrics.json` (Prometheus
range snapshot + outcome) and full per-pod logs. Then:

- **Backfill / local:** `npm run import` scans `../output/runs/*` and upserts
  every run into Neon (reuses `lib/ingest.ts`; old runs fall back to log parsing).
- **Push (going forward):** `leanstart runs push <id>` POSTs a gzipped run bundle
  to `POST /api/ingest` (bearer `INGEST_TOKEN`), which extracts and upserts it.

`lib/ingest.ts` is the single source of truth for parsing + upserting, used by
both the importer and the ingest route.

## Local dev
```bash
cd web
npm install
# .env.local must contain DATABASE_URL (Neon) and INGEST_TOKEN
npm run import      # backfill output/runs into Neon
npm run dev         # http://localhost:3939
```

## Env
- `DATABASE_URL` — Neon connection string.
- `INGEST_TOKEN` — bearer token required by `/api/ingest` (matches the token used
  by `leanstart runs push`).

## Deploy (Vercel)
Set `DATABASE_URL` + `INGEST_TOKEN` as Vercel env vars; deploy. The site is
read-only/public; only `/api/ingest` is token-gated.

## Notes
- Sample series are downsampled to ~500 points per metric/pod to stay within the
  Neon free tier; raw resolution lives in the run dir's `metrics.json`.
- Backfilled runs without `run.json` derive fix facets from the image tag
  (`fixesForImage` in `lib/ingest.ts`, mirroring leanstart's `run_record.rs`).
- Log-derived timelines are trimmed to the last chain segment so a run reflects
  its own genesis, not an earlier incarnation captured in the same streamed log.
