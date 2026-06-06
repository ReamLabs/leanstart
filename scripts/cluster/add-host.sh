#!/usr/bin/env bash
#
# add-host.sh — join a new Hetzner server to the leannet k3s cluster as a worker.
#
# One command to grow the fleet. Run from your laptop (needs SSH access to both
# the control-plane and the new host). The new host joins over the Hetzner
# private network and is labelled so leanstart's `@host` placement can target it.
#
# Usage:
#   scripts/cluster/add-host.sh <ssh-target> <private-ip> <host-label> [iface]
#
# Example:
#   scripts/cluster/add-host.sh root@5.75.151.240 10.0.0.5 nbg4
#
# Arguments:
#   ssh-target   user@public-ip (or ssh alias) for the NEW host, root-capable
#   private-ip   the new host's IP on the Hetzner private network
#   host-label   friendly name used as leanstart.io/host=<label> (e.g. nbg4);
#                target it later with `leanstart ream:3@nbg4`
#   iface        private NIC name (default: enp7s0, the Hetzner default)
#
# Prereqs: the new server must already be attached to the same Hetzner private
# network as the control-plane (do this in the Hetzner console first).

set -euo pipefail

# --- cluster config (edit if your control-plane changes) ---------------------
CP_SSH="${LEANNET_CP_SSH:-shariq@178.104.139.49}"   # control-plane SSH target
CP_PRIV_IP="${LEANNET_CP_PRIV_IP:-10.0.0.4}"        # control-plane private IP
SSH_KEY="${LEANNET_SSH_KEY:-$HOME/.ssh/id_ed25519}"
# -----------------------------------------------------------------------------

if [ "$#" -lt 3 ]; then
  grep '^#' "$0" | sed 's/^# \{0,1\}//' | sed -n '2,30p'
  exit 1
fi

NEW_SSH="$1"
NEW_PRIV_IP="$2"
HOST_LABEL="$3"
IFACE="${4:-enp7s0}"

SSH="ssh -o IdentitiesOnly=yes -o BatchMode=yes -o StrictHostKeyChecking=accept-new -i $SSH_KEY"

echo "==> Fetching join token from control-plane ($CP_SSH)..."
TOKEN=$($SSH "$CP_SSH" 'sudo -n cat /var/lib/rancher/k3s/server/node-token')
[ -n "$TOKEN" ] || { echo "ERROR: could not read node-token from control-plane"; exit 1; }

echo "==> Verifying new host can reach control-plane over the private network..."
$SSH "$NEW_SSH" "ping -c1 -W3 $CP_PRIV_IP >/dev/null" \
  || { echo "ERROR: $NEW_SSH cannot ping $CP_PRIV_IP — is it attached to the private network?"; exit 1; }

echo "==> Installing k3s agent on $NEW_SSH (private IP $NEW_PRIV_IP, label $HOST_LABEL)..."
$SSH "$NEW_SSH" "curl -sfL https://get.k3s.io | \
  K3S_URL=https://${CP_PRIV_IP}:6443 \
  K3S_TOKEN='${TOKEN}' \
  INSTALL_K3S_EXEC='agent --node-ip=${NEW_PRIV_IP} --flannel-iface=${IFACE} --node-label leanstart.io/host=${HOST_LABEL}' \
  sh - 2>&1 | tail -4"

echo "==> Waiting for the new node to become Ready..."
$SSH "$CP_SSH" "sudo -n k3s kubectl wait --for=condition=Ready node -l leanstart.io/host=${HOST_LABEL} --timeout=120s"

echo "==> Cluster nodes:"
$SSH "$CP_SSH" "sudo -n k3s kubectl get nodes -L leanstart.io/host -o wide"

echo
echo "Done. New host is in the cluster as leanstart.io/host=${HOST_LABEL}."
echo "Target it with:  leanstart <client>:<n>@${HOST_LABEL}"
