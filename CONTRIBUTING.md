## Contributing to leanstart

Thanks for your interest in improving leanstart!

`leanstart` is a devnet orchestrator for Lean consensus validators. Contributions of all sizes are welcome — bug fixes, new client integrations, Helm chart improvements, docs.

If you contribute to this project, your contributions will be made under the MIT license.

### Code of Conduct

This project adheres to the [Rust Code of Conduct][rust-coc].

### Getting started

1. Fork and clone the repo.
2. Install Rust (stable) and the standard toolchain.
3. `cargo build` and `cargo test` to verify your environment.
4. For end-to-end testing you'll also want a local [`kind`](https://kind.sigs.k8s.io/) cluster, `kubectl`, and `helm`.

### Pull requests

- Keep PRs focused — one concern per PR.
- Run `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` before opening a PR.
- PR titles should be in the imperative mood (e.g. "fix genesis offset overflow", not "fixed").

### Reporting issues

Open an issue at [github.com/ReamLabs/leanstart/issues](https://github.com/ReamLabs/leanstart/issues) with a clear description and reproduction steps.

For security-sensitive issues, see [SECURITY.md](SECURITY.md) instead.

[rust-coc]: https://www.rust-lang.org/policies/code-of-conduct
