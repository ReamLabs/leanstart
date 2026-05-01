# leanstart

Devnet orchestrator for Lean consensus validators. `leanstart` generates genesis state, validator keys, and Helm values, then deploys a multi-client Lean devnet to a Kubernetes cluster.

## Prerequisites

- Rust (stable)
- A Kubernetes cluster (e.g. [`kind`](https://kind.sigs.k8s.io/)) and `kubectl`
- [Helm](https://helm.sh/) 3.x
- The Lean `generate-genesis.sh` script (path supplied via `--genesis-script` or `GENESIS_SCRIPT`)

## Install

```sh
cargo install --path .
```

Or build from source:

```sh
cargo build --release
./target/release/leanstart --help
```

## Quick start

Run a devnet with one `ream` node and two `zeam` nodes:

```sh
leanstart ream zeam:2
```

The first positional arg is treated as a client spec, so `leanstart run ream zeam:2` and `leanstart ream zeam:2` are equivalent. `run` covers the common path: generate artifacts, then deploy them.

## Commands

| Command | Description |
|---|---|
| `leanstart run <clients...>` | Generate config and deploy a devnet end-to-end |
| `leanstart generate` | Generate validator config, keys, genesis, and Helm values only |
| `leanstart deploy` | Deploy a previously generated devnet to Kubernetes |
| `leanstart status` | Show pod status in the devnet namespace |
| `leanstart destroy` | Tear down the devnet |

Run `leanstart <command> --help` for full flags.

## Generate, then deploy (explicit flow)

`run` is a shortcut. For more control — reproducible artifacts, reviewing the generated Helm values, deploying the same artifacts to multiple clusters, or running in CI — split the pipeline into `generate` and `deploy`.

```sh
# 1. Generate validator config, keys, genesis, and helm-values.yaml
leanstart generate \
  --clients ream:1,zeam:2 \
  --namespace lean-devnet \
  --output-dir ./output \
  --validators-per-pod 1 \
  --genesis-offset 120 \
  --subnets 1

# 2. Inspect what was generated
ls ./output            # helm-values.yaml, genesis/, secrets/
cat ./output/helm-values.yaml

# 3. Deploy the generated artifacts
leanstart deploy --output-dir ./output --namespace lean-devnet

# 4. Watch pods and tear down when done
leanstart status  --namespace lean-devnet
leanstart destroy --namespace lean-devnet
```

The `--output-dir` is the durable artifact: regenerating with the same `--seed` is deterministic, and you can deploy the same directory repeatedly without re-running `generate`.

For air-gapped or offline use, pass `--config-only` to `generate` to skip the Docker-based genesis step and emit just the YAML manifests.

## Running multiple devnets

Each devnet is isolated by its Kubernetes namespace, kind cluster name, and output directory. Vary all three to run several side-by-side:

```sh
# Devnet A: 1 ream + 2 zeam in namespace "devnet-a"
leanstart ream zeam:2 \
  --namespace devnet-a \
  --cluster   devnet-a \
  --output-dir ./output/devnet-a

# Devnet B: 3 grandine in namespace "devnet-b"
leanstart grandine:3 \
  --namespace devnet-b \
  --cluster   devnet-b \
  --output-dir ./output/devnet-b
```

Operate each devnet independently by passing the matching `--namespace`:

```sh
leanstart status  --namespace devnet-a
leanstart destroy --namespace devnet-b
```

## Helm chart

The `helm/lean-devnet` chart is consumed directly by `leanstart` and can also be installed manually:

```sh
helm install lean-devnet ./helm/lean-devnet -f output/helm-values.yaml -n lean-devnet --create-namespace
```

Presets for cluster sizes live in `helm/lean-devnet/presets/`.

## Layout

```
src/        leanstart CLI (Rust)
helm/       Helm chart for the devnet
scripts/    Helper scripts (e.g. local kind deployment with peer discovery)
tests/      Integration tests
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Security issues: see [SECURITY.md](SECURITY.md).

## License

MIT — see [LICENSE](LICENSE).
