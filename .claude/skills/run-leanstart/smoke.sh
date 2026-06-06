#!/usr/bin/env bash
#
# smoke.sh — build leanstart and exercise its generation pipeline with NO Docker
# and NO Kubernetes cluster. This is the fast, hermetic "does it run" check.
#
# It drives the real CLI (`leanstart generate --config-only`), which produces
# validator config + Helm values without the Docker-based key/genesis steps, and
# asserts on the output. For a full devnet (which needs Docker + a cluster) see
# SKILL.md — that path is exercised against a real cluster, not here.
#
# Usage:  .claude/skills/run-leanstart/smoke.sh
# Exit:   0 = all checks passed; non-zero = first failure (with a message).

set -euo pipefail
cd "$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"

BIN=target/debug/leanstart
OUT=$(mktemp -d)
trap 'rm -rf "$OUT"' EXIT

pass() { echo "  ✅ $1"; }
fail() { echo "  ❌ $1"; exit 1; }

echo "==> build"
cargo build --quiet
[ -x "$BIN" ] || fail "binary not built at $BIN"
pass "built $BIN"

echo "==> CLI sanity: --help and unknown-client error path"
"$BIN" --help >/dev/null 2>&1 || fail "leanstart --help failed"
pass "--help works"
# A bad client must fail with a clear error (exercises arg parsing + validation).
if "$BIN" run nosuchclient --config-only --output-dir "$OUT/bad" >/dev/null 2>&1; then
  fail "unknown client should have errored"
fi
pass "unknown client rejected"

echo "==> generate --config-only (no Docker/cluster): placement + multi-host"
"$BIN" generate \
  --clients "ream:1@nbg1,ream:2@nbg2,zeam:1@nbg3" \
  --config-only --output-dir "$OUT/gen" >/dev/null
VC="$OUT/gen/genesis/validator-config.yaml"
HV="$OUT/gen/helm-values.yaml"
[ -f "$VC" ] || fail "no validator-config.yaml"
[ -f "$HV" ] || fail "no helm-values.yaml"
pass "generated artifacts"

echo "==> assert: continuous per-client pod naming across @host allocations"
names=$(grep -E '^- name:' "$VC" | awk '{print $3}' | tr '\n' ' ')
echo "    names: $names"
[ "$names" = "ream_0 ream_1 ream_2 zeam_0 " ] || fail "unexpected pod names: $names"
pass "ream_0..ream_2 + zeam_0 (continuous numbering)"

echo "==> assert: first pod is the aggregator"
grep -A8 '^- name: ream_0' "$VC" | grep -q 'isAggregator: true' || fail "ream_0 not aggregator"
pass "ream_0 isAggregator=true"

echo "==> assert: @host placement reaches Helm values as nodeSelectorHost"
grep -q 'nodeSelectorHost: nbg1' "$HV" || fail "missing nodeSelectorHost: nbg1"
grep -q 'nodeSelectorHost: nbg2' "$HV" || fail "missing nodeSelectorHost: nbg2"
pass "nodeSelectorHost pins present in helm-values.yaml"

echo "==> chart lints (helm template renders with generated values)"
if command -v helm >/dev/null 2>&1; then
  helm template t helm/lean-devnet -f "$HV" \
    --set genesis.external=true --set prometheus.enabled=false >/dev/null \
    || fail "helm template failed"
  pass "helm template renders"
else
  echo "  ⚠️  helm not found — skipping chart render"
fi

echo
echo "SMOKE OK"
