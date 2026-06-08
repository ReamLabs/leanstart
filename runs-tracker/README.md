# leanstart run tracker

A zero-dependency website that tracks **every** `leanstart run` and displays the
information collected during it. It reads the `output/runs/<timestamp>/` tree that
`leanstart run` already writes — no changes to leanstart required.

## Run

```bash
python3 runs-tracker/server.py            # serves http://localhost:8099
python3 runs-tracker/server.py --port 9000 --runs ./output/runs
```

Open the URL. The run list auto-refreshes every 15s, so a run that's currently
streaming shows up live.

## What it shows

Per run:
- **Outcome** — max finalized slot, max head slot, did-it-finalize.
- **Finalization timeline** — head / justified / finalized slot over time, parsed
  from the node logs (ream's `REAM's CHAIN STATUS` blocks and ethlambda's
  `our_finalized_slot=…` status lines), rendered as an inline SVG chart.
- **Spec** — clients, images, subnet/validator summary, and the command, parsed
  from `run.log`.
- **Node logs** — per-node, ANSI-stripped, tail view.
- **Genesis** — `config.yaml` if it was snapshotted into the run dir.

## Architecture / extending

- `server.py` — stdlib `http.server`; scans the runs dir, parses logs, serves
  `GET /api/runs`, `GET /api/runs/<id>`, `GET /api/runs/<id>/log/<name>`.
- `index.html` — single-file frontend (no external JS/CSS).

Notes for richer tracking (out of scope of this fork, but where to hook):
- `leanstart run` currently **reuses** `output/genesis/` (overwritten each run);
  to keep per-run genesis, copy it into the run dir at deploy time, or have the
  run write a `run.json` (spec + final metrics) — the server already prefers a
  per-run `config.yaml`/`genesis/config.yaml` if present.
- For live cluster metrics beyond what logs contain (e.g.
  `lean_committee_signatures_aggregation_time_seconds`), snapshot each pod's
  `:8080/metrics` over the run window into the run dir, or deep-link the run's
  time range into the existing Grafana dashboard.
