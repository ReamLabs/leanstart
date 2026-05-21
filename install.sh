#!/usr/bin/env bash
# install.sh — bootstrap a new machine for leanstart.
# Works on macOS (x86_64 + arm64) and Linux (x86_64 + aarch64).
# Safe to re-run; every step is idempotent.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/shariqnaiyer/leanstart/master/install.sh | bash
#   — or —
#   ./install.sh   (from a cloned repo)
set -euo pipefail

# ── colours ───────────────────────────────────────────────────────────────────
BLUE='\033[0;34m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; RED='\033[0;31m'; NC='\033[0m'
info() { echo -e "${BLUE}==>${NC} $*"; }
ok()   { echo -e "${GREEN}  ✓${NC} $*"; }
warn() { echo -e "${YELLOW}  ⚠${NC} $*"; }
die()  { echo -e "${RED}  ✗${NC} $*" >&2; exit 1; }

# ── platform ──────────────────────────────────────────────────────────────────
OS="$(uname -s)"    # Darwin | Linux
ARCH="$(uname -m)"  # x86_64 | arm64 | aarch64

case "$ARCH" in
  x86_64)        ARCH_GO="amd64" ;;
  arm64|aarch64) ARCH_GO="arm64" ;;
  *) die "Unsupported architecture: $ARCH" ;;
esac

[[ "$OS" == "Darwin" || "$OS" == "Linux" ]] || die "Unsupported OS: $OS"

# ── sudo helper ───────────────────────────────────────────────────────────────
# Use sudo only when not already root and sudo is available.
SUDO=""
if [[ "$EUID" -ne 0 ]]; then
  command -v sudo &>/dev/null && SUDO="sudo"
fi

sys() { $SUDO "$@"; }  # run a command elevated if needed

# ── detect run mode ───────────────────────────────────────────────────────────
# When piped via curl, BASH_SOURCE[0] is empty/unset or "bash" — not a real file.
REPO_DIR=""
if [[ -n "${BASH_SOURCE[0]:-}" && -f "${BASH_SOURCE[0]}" ]]; then
  REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
fi

# ── Rust ──────────────────────────────────────────────────────────────────────
if ! command -v rustup &>/dev/null; then
  info "Installing Rust toolchain..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path
fi
source "${CARGO_HOME:-$HOME/.cargo}/env" 2>/dev/null || true
command -v cargo &>/dev/null || die "cargo not found after rustup install — open a new shell and re-run"
ok "Rust $(rustc --version | awk '{print $2}')"

# ── macOS dependencies ────────────────────────────────────────────────────────
install_macos_deps() {
  if ! command -v brew &>/dev/null; then
    info "Installing Homebrew..."
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
    # Apple Silicon puts brew in /opt/homebrew
    [[ -f /opt/homebrew/bin/brew ]] && eval "$(/opt/homebrew/bin/brew shellenv)"
  fi
  ok "Homebrew $(brew --version | head -1 | awk '{print $2}')"

  for pkg in kind kubectl helm; do
    if ! command -v "$pkg" &>/dev/null; then
      info "Installing $pkg..."
      brew install "$pkg"
    fi
    ok "$pkg"
  done

  # Docker Desktop
  if ! command -v docker &>/dev/null; then
    if [[ -d "/Applications/Docker.app" ]]; then
      warn "Docker.app found but 'docker' not in PATH — open Docker Desktop to finish setup"
    else
      info "Installing Docker Desktop..."
      brew install --cask docker
      warn "Open Docker Desktop once to complete setup, then re-run if needed"
    fi
  else
    ok "docker $(docker --version | awk '{print $3}' | tr -d ',')"
  fi
}

# ── Linux dependencies ────────────────────────────────────────────────────────
install_linux_deps() {
  sys mkdir -p /usr/local/bin

  # kind
  if ! command -v kind &>/dev/null; then
    info "Installing kind..."
    KIND_VER="$(curl -fsSL https://api.github.com/repos/kubernetes-sigs/kind/releases/latest \
                | grep '"tag_name"' | cut -d'"' -f4)"
    curl -fsSLo /tmp/kind \
      "https://kind.sigs.k8s.io/dl/${KIND_VER}/kind-linux-${ARCH_GO}"
    chmod +x /tmp/kind
    sys mv /tmp/kind /usr/local/bin/kind
  fi
  ok "kind $(kind version)"

  # kubectl
  if ! command -v kubectl &>/dev/null; then
    info "Installing kubectl..."
    K8S_VER="$(curl -fsSL https://dl.k8s.io/release/stable.txt)"
    curl -fsSLo /tmp/kubectl \
      "https://dl.k8s.io/release/${K8S_VER}/bin/linux/${ARCH_GO}/kubectl"
    chmod +x /tmp/kubectl
    sys mv /tmp/kubectl /usr/local/bin/kubectl
  fi
  ok "kubectl $(kubectl version --client 2>/dev/null | head -1)"

  # helm
  if ! command -v helm &>/dev/null; then
    info "Installing helm..."
    curl -fsSL https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash
  fi
  ok "helm $(helm version --short 2>/dev/null)"

  # Docker
  if ! command -v docker &>/dev/null; then
    info "Installing Docker..."
    curl -fsSL https://get.docker.com | sh
    if getent group docker &>/dev/null && [[ -n "${USER:-}" ]]; then
      sys usermod -aG docker "$USER"
      warn "Added $USER to the 'docker' group — log out and back in (or: newgrp docker)"
    fi
  else
    ok "docker $(docker --version | awk '{print $3}' | tr -d ',')"
  fi
}

# ── install deps for this platform ───────────────────────────────────────────
case "$OS" in
  Darwin) install_macos_deps ;;
  Linux)  install_linux_deps ;;
esac

# ── build + install leanstart ─────────────────────────────────────────────────
if [[ -n "$REPO_DIR" ]]; then
  # Running from a cloned repo — build from local source
  info "Building leanstart (release)..."
  cargo build --release --manifest-path "$REPO_DIR/Cargo.toml"
  ok "leanstart built"

  info "Installing leanstart to /usr/local/bin..."
  sys cp "$REPO_DIR/target/release/leanstart" /usr/local/bin/leanstart
else
  # Running via curl | bash — install directly from git
  info "Installing leanstart from git..."
  cargo install --git https://github.com/shariqnaiyer/leanstart --locked
fi

ok "leanstart → $(command -v leanstart)"

# ── done ─────────────────────────────────────────────────────────────────────
echo ""
echo -e "${GREEN}All set!${NC} Run ${BLUE}leanstart --help${NC} to get started."
