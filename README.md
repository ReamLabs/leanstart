# leanstart

A local devnet orchestrator for [Lean Ethereum](https://ethresear.ch/t/lean-ethereum/22547) validator clients. Spin up a multi-client devnet with a single command — key generation, genesis, Kubernetes, and metrics included.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/shariqnaiyer/leanstart/master/install.sh | bash
```

Works on **macOS** (x86_64 + Apple Silicon) and **Linux** (x86_64 + aarch64). Installs: **Rust**, **kind**, **kubectl**, **helm**, **Docker**, and the `leanstart` binary. Safe to re-run.

| Platform | Docker install |
|---|---|
| macOS | Docker Desktop via Homebrew cask |
| Linux | Docker Engine via `get.docker.com` |

> **Note:** Docker must be running before launching a devnet. On macOS, open Docker Desktop once after install to complete its setup. On Linux, log out and back in after install to pick up the `docker` group (or run `newgrp docker`).

---

## Quick start

```bash
# Single-client devnet (3 ream pods)
leanstart ream:3

# Multi-client devnet
leanstart ream:2 zeam:2

# Multi-subnet devnet (2 subnets × 3 pods each)
leanstart ream:3 --subnets 2
```

Once running, Grafana is at **http://localhost:3000** (`admin` / `admin`) and Prometheus at **http://localhost:9090**.

---

## Multi-machine devnets

To spread one devnet across several servers (and pin clients to specific hosts with
`name:count@host`), see **[docs/distributed-devnets.md](docs/distributed-devnets.md)**.

---

## Supported clients

| Client | Image |
|---|---|
| `ream` | `snaiyer1/ream:latest-devnet5` |
| `zeam` | `blockblaz/zeam:devnet4` |
| `grandine` | `sifrai/lean:devnet-4` |
| `lantern` | `piertwo/lantern:v0.0.4` |
| `qlean` | `qdrvm/qlean-mini:devnet-4` |
| `ethlambda` | `ghcr.io/lambdaclass/ethlambda:devnet4` |

Client images are always pulled fresh from the registry on each run.

---

## Commands

### `leanstart run` (default)

```
leanstart [run] <clients...> [options]
```

`run` is the default — `leanstart ream:3` and `leanstart run ream:3` are identical.

**Client spec:** `<name>` or `<name>:<count>` — e.g. `ream`, `zeam:2`, `grandine:3`.

| Flag | Default | Description |
|---|---|---|
| `--subnets <N>` | `1` | Number of attestation subnets. Each client allocation is replicated once per subnet (max 5) |
| `--attestation-committee-count <N>` | `--subnets` | Override the committee count independently of subnet count |
| `--validators-per-pod <N>` | `1` | Validators assigned to each pod |
| `--genesis-offset <secs>` | `120` | Seconds until genesis from now (gives pods time to start) |
| `--seed <hex>` | `0000…0001` | 32-byte hex seed for deterministic key generation |
| `--active-epoch <N>` | `18` | Hash-sig key active epoch exponent (2^N total epochs) |
| `--bootnode-count <N>` | `5` | Bootnode pods per client type |
| `--namespace <name>` | `lean-devnet` | Kubernetes namespace |
| `--cluster <name>` | `lean-devnet` | Kind cluster name |
| `--output-dir <path>` | `./output` | Directory for generated artifacts |
| `--skip-kind` | — | Skip Kind cluster creation; use an existing cluster |
| `--context <ctx>` | — | kubectl/helm context override (required with `--skip-kind`) |
| `--skip-metrics` | — | Skip Prometheus + Grafana installation |
| `--config-only` | — | Generate config files only, skip deployment |

---

### `leanstart status`

Show pod status for the running devnet.

```bash
leanstart status
leanstart status --namespace <name>
```

---

### `leanstart destroy`

Tear down the devnet and delete the Kind cluster.

```bash
leanstart destroy
leanstart destroy --cluster <name> --namespace <name>
```

---

### `leanstart generate`

Generate all artifacts (keys, genesis, Helm values) without deploying. Useful for inspecting config or deploying to a remote cluster.

```bash
leanstart generate --clients ream:2,zeam:2 --subnets 2
leanstart deploy   --output-dir ./output
```

---

## Output

Every run logs to `./output/runs/<timestamp>/`:

```
output/
├── genesis/
│   ├── config.yaml                  # Chain config
│   ├── genesis.ssz / genesis.json   # Genesis state
│   ├── nodes.yaml                   # Bootnode peer list
│   ├── annotated_validators.yaml    # Validator registry
│   ├── hash-sig-keys/               # Hash-sig key pairs per validator
│   └── <pod>.key                    # libp2p node keys
├── helm-values.yaml                 # Helm chart values
├── secrets/                         # Per-pod Kubernetes Secrets
└── runs/
    ├── latest -> <timestamp>/       # Symlink to most recent run
    └── <timestamp>/
        ├── run.log                  # Full orchestration log
        ├── <pod>.log                # Streaming pod logs
        └── <pod>.previous.log       # Previous container logs (on crash)
```

---

## Observability

When metrics are enabled (default), the following are port-forwarded automatically:

| Service | URL | Credentials |
|---|---|---|
| Grafana | http://localhost:3000 | `admin` / `admin` |
| Prometheus | http://localhost:9090 | — |

Grafana is pre-configured with two dashboard folders:
- **Lean Ethereum Clients** — the main client dashboard (set as home)
- **Infra** — Kubernetes and Prometheus internals

Each pod exposes:
- `:8080/metrics` — Prometheus metrics
- `:9000` — libp2p / QUIC P2P port
- `:5055` — HTTP REST API (where supported)

---

## Multi-subnet devnets

Passing `--subnets N` replicates each client allocation across N subnets and configures one aggregator per subnet:

```bash
# 2 subnets × 3 ream pods = 6 pods total
leanstart ream:3 --subnets 2
```

Pod naming in multi-subnet mode: `ream-s0-p0`, `ream-s0-p1`, `ream-s1-p0`, etc.
