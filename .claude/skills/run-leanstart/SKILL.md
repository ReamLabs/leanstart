---
name: run-leanstart
description: Build, run, and drive leanstart — spin up a Lean Ethereum devnet (local kind or a multi-node cluster), screenshot/inspect it, run the smoke test. Use when asked to run, start, launch, build, test, deploy, or screenshot leanstart / a devnet.
---

# Run leanstart

`leanstart` is a Rust CLI that spins up multi-client **Lean Ethereum** validator devnets
on Kubernetes (key generation → genesis → Helm deploy → metrics). It runs in two modes:

- **Local** (default): creates a `kind` cluster on this machine and deploys into it.
- **Remote multi-node** (`--skip-kind --context <ctx>`): deploys to an existing
  Kubernetes cluster and distributes client pods across hosts via `@host` placement
  ("gated-init" mode — see Gotchas).

The agent harness is **`.claude/skills/run-leanstart/smoke.sh`** — it builds the binary
and drives the real CLI's generation pipeline with **no Docker and no cluster**, asserting
on output (placement, aggregator selection, pod naming, Helm rendering). Use it as the
fast "does it work" check. Full devnets need Docker + a cluster (see below).

> Paths below are relative to the repo root. Verified on macOS (Apple Silicon) with the
> toolchain already installed; the repo's `install.sh` installs everything on macOS + Linux.

## Prerequisites

Tools: **Rust/cargo, Docker, kubectl, helm**, and **kind** (only for the local path).
The repo bundles an installer that sets all of these up on macOS and Linux:

```bash
curl -fsSL https://raw.githubusercontent.com/shariqnaiyer/leanstart/master/install.sh | bash
```

Docker must be **running** before any full devnet (leanstart uses Docker to generate
hash-sig keys and run the `eth-beacon-genesis` tool). Verify: `docker info`.

## Build

```bash
cargo build
```

## Run — agent path (fast, no infra): the smoke driver

```bash
.claude/skills/run-leanstart/smoke.sh
```

Builds, runs `leanstart generate --config-only`, and asserts: continuous per-client pod
naming across `@host` allocations (`ream_0..ream_2 zeam_0`), `ream_0` is the aggregator,
`@host` pins land in `helm-values.yaml` as `nodeSelectorHost`, and the Helm chart renders.
Prints `SMOKE OK` and exits 0 on success. No Docker or cluster required.

## Run — a full devnet (needs Docker + a cluster)

Client spec is `name[:count][@host]`. No `@host` ⇒ pods auto-spread across nodes.

**Local (kind)** — the canonical single-machine run:

```bash
leanstart ream:2 --active-epoch 8
# 2 ream pods in a local kind cluster; Grafana on http://localhost:3000 (admin/admin)
```

**Remote multi-node** — distribute clients across hosts of an existing cluster. This is
the exact command used to run 9 validators across 5 servers (1 aggregator on the big box,
2 on each of four workers):

```bash
leanstart ream:1@nbg1 ream:2@nbg2 ream:2@nbg3 ream:2@nbg4 ream:2@nbg5 \
  --skip-kind --context leannet --skip-metrics --active-epoch 8 --genesis-offset 260
```

Nodes must be labelled `leanstart.io/host=<name>` (the `@host` value). Full multi-machine
setup — cluster bring-up, adding hosts, metrics — is in
[`docs/distributed-devnets.md`](../../../docs/distributed-devnets.md). Drop the flags by
creating `~/.leanstart/config.yaml` (`context:`, `skip_kind:`, `skip_metrics:`).

### Drive / inspect a running devnet

```bash
kubectl --context leannet get pods -n lean-devnet -o wide                 # placement
kubectl --context leannet logs ream-0-0 -n lean-devnet | grep -E "Connected Peers|Finaliz"
leanstart status                                                          # pod status
leanstart destroy --namespace lean-devnet                                 # tear down
```

Per-pod logs also stream to `output/runs/<timestamp>/<pod>.log` on the launching machine
(`output/runs/latest` → newest). Metrics, when enabled, are in Grafana (local: port-forward
`localhost:3000`; remote example cluster: NodePort on the control-plane public IP).

## Test

```bash
cargo test          # unit + integration (placement parsing, generation, helm values)
```

## Gotchas (battle scars)

- **`@host` needs labelled nodes.** A pinned pod (`ream:2@nbg4`) stays `Pending` if no node
  has `leanstart.io/host=nbg4`. Label: `kubectl label node <node> leanstart.io/host=nbg4`.
- **Repeated same-client allocations** (`ream:1@nbg1 ream:2@nbg2`) are numbered continuously
  (`ream_0`, `ream_1`, …); **`ream_0` (first pod overall) is the aggregator**. Don't expect
  per-allocation index resets.
- **Remote = "injected" gated-init.** With `--skip-kind`, pods deliberately sit in
  `Init:0/1` until the orchestrator reads their IPs, regenerates the **signed ENRs** (the IP
  is baked into the signed record — it can't be DNS or rewritten in-init), and `kubectl cp`s
  IP-correct genesis + that pod's keys in, then drops a `/config/.ready` sentinel. Pods
  staying in Init for a minute is **normal**, not a hang.
- **hash-sig keys are huge** (~8 MB `.ssz`, ~55 MB `.json` per validator; ~1.4 GB for 6).
  They can't go in ConfigMaps/Secrets (1 MiB limit) or be shipped wholesale to a remote
  cluster — injected mode ships only each pod's own `.ssz` files. Only Directory-mode clients
  (grandine/lantern/qlean) read `/config/hash-sig-keys`; ream/zeam/ethlambda don't.
- **`--active-epoch` dominates keygen time/size.** Default `18` makes large keys slowly; use
  `--active-epoch 8` for fast test devnets.
- **`--genesis-offset`** must exceed the time to pull images + start pods, or the chain's
  genesis passes before pods are up. Fresh nodes pulling client images for the first time
  need a bigger offset (e.g. 240–260s).
- **`--skip-metrics` ⇒ no ServiceMonitor.** The chart's ServiceMonitor needs the Prometheus
  Operator CRD; with `--skip-metrics` it's omitted (so a no-metrics deploy doesn't fail on a
  missing CRD). To scrape such a devnet, install kube-prometheus-stack once cluster-wide and
  apply a ServiceMonitor selecting `app.kubernetes.io/part-of=lean-devnet` (port `metrics`).
- **Client images are pulled fresh each run** from public registries; first pull on a new
  node is slow.

## Troubleshooting

- **`Helm install failed: no matches for kind "ServiceMonitor"`** — the Prometheus Operator
  CRD isn't installed. Either run without `--skip-metrics` (installs the metrics stack) or,
  on a cluster that already has it, ensure metrics are enabled. (Fixed in code by gating the
  ServiceMonitor on `prometheus.enabled`, which `--skip-metrics` sets false.)
- **Pod stuck `Pending`** — usually an unlabelled `@host` target; check
  `kubectl describe pod <pod>` for the unschedulable reason.
- **`leanstart run` doesn't return after "Devnet is running!"** — the background
  `kubectl logs -f` streamers can keep the process attached; the devnet itself is up. Safe to
  Ctrl-C / kill the leanstart process; the devnet keeps running.
- **`docker` errors during a full run** — Docker isn't running; `leanstart` needs it for
  keygen + genesis. Start Docker and re-run.
