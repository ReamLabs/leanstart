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

The first positional arg is treated as a client spec, so `leanstart run ream zeam:2` and `leanstart ream zeam:2` are equivalent.

## Commands

| Command | Description |
|---|---|
| `leanstart run <clients...>` | Generate config and deploy a devnet end-to-end |
| `leanstart generate` | Generate validator config, keys, genesis, and Helm values only |
| `leanstart deploy` | Deploy a previously generated devnet to Kubernetes |
| `leanstart status` | Show pod status in the devnet namespace |
| `leanstart destroy` | Tear down the devnet |

Run `leanstart <command> --help` for full flags.

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
