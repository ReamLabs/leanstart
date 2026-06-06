# Distributed devnets across multiple machines

Run a single Lean Ethereum devnet whose client pods are spread across **many servers**,
launched from one command, with control over which client runs on which host.

`leanstart` normally runs everything in a local `kind` cluster (one machine). To go
multi-machine you instead point it at a real **multi-node Kubernetes cluster** (we use
[k3s](https://k3s.io)) and use `@host` placement to distribute pods. This guide is the
end-to-end path that works **today**.

> Reference deployment: a 3-node cluster on Hetzner Cloud called **leannet**
> (`nbg1`=control-plane 30G, `nbg2`/`nbg3`=workers 15G), joined over a Hetzner private
> network. Substitute your own hosts/IPs throughout. Operational details for that specific
> cluster live in [`scripts/cluster/README.md`](../scripts/cluster/README.md).

---

## Overview

```
your laptop                          ┌── nbg1 (control-plane) ──┐
  leanstart  ──(kubectl/helm)──────▶ │  k3s server              │
  (generates keys/genesis,           │  ream_0 (aggregator)     │
   injects per-pod, places pods)     └──────────────────────────┘
                                     ┌── nbg2 (worker) ─┐  ┌── nbg3 (worker) ─┐
                                     │ ream_1  ream_2   │  │ ream_3  ream_4   │
                                     └──────────────────┘  └──────────────────┘
        all nodes share one flannel pod network → pods peer cross-machine
```

Key ideas:
- **One k3s cluster** spans all machines; nodes are labelled `leanstart.io/host=<name>`.
- **`@host` placement**: `leanstart ream:2@nbg2` pins those pods to the node labelled
  `nbg2`. No `@host` ⇒ pods auto-spread across nodes.
- **`--skip-kind`** switches leanstart into *injected* mode: it deploys to your existing
  cluster and injects IP-correct genesis + each pod's keys directly into the pods (works
  across machines; no shared storage required).

---

## Prerequisites

- 2+ Linux servers (Ubuntu) that can reach each other on a **private network** (lower
  latency, and avoids exposing the k3s API/flannel publicly). Note each server's private
  IP and NIC name (`ip -o -4 addr show`).
- SSH access to all of them (root, or a sudo user).
- On the **machine you launch from** (your laptop): `kubectl`, `helm`, `docker`
  (leanstart uses Docker locally to generate keys + genesis), and the `leanstart` binary
  (`cargo build --release`, or `curl -fsSL .../install.sh | bash`).

---

## Step 1 — Bring up the multi-node cluster (one-time)

### 1a. Control-plane (first server)

Install k3s bound to the server's **private** IP/NIC so cross-node traffic stays on the
private network. Add the public IP to the cert SANs so you can reach the API from your
laptop.

```bash
# on the control-plane server (PRIV=its private IP, e.g. 10.0.0.4; PUB=its public IP)
curl -sfL https://get.k3s.io | sudo INSTALL_K3S_EXEC="server \
  --node-ip=$PRIV --flannel-iface=enp7s0 --advertise-address=$PRIV \
  --tls-san=$PRIV --tls-san=$PUB \
  --write-kubeconfig-mode 0644 --node-label leanstart.io/host=nbg1" sh -
```

Grab the join token (you'll need it for workers):

```bash
sudo cat /var/lib/rancher/k3s/server/node-token
```

### 1b. Workers (every other server)

```bash
# on each worker (PRIV=its private IP; CP_PRIV=control-plane private IP; TOKEN from above;
# LABEL=nbg2, nbg3, …)
curl -sfL https://get.k3s.io | sudo K3S_URL=https://$CP_PRIV:6443 K3S_TOKEN="$TOKEN" \
  INSTALL_K3S_EXEC="agent --node-ip=$PRIV --flannel-iface=enp7s0 \
  --node-label leanstart.io/host=$LABEL" sh -
```

> **Shortcut for the leannet cluster:** once the control-plane exists, adding any further
> host is a single command from your laptop:
> ```bash
> scripts/cluster/add-host.sh <user@public-ip> <private-ip> <host-label>
> ```

### 1c. Get a kubeconfig on your laptop

Copy `/etc/rancher/k3s/k3s.yaml` from the control-plane, replace `127.0.0.1` with the
control-plane's **public** IP, and save it as a context (e.g. `leannet`):

```bash
ssh user@<cp-public-ip> 'sudo cat /etc/rancher/k3s/k3s.yaml' > /tmp/k3s.yaml
sed -i '' 's#https://127.0.0.1:6443#https://<cp-public-ip>:6443#' /tmp/k3s.yaml   # macOS sed
# merge into ~/.kube/config (rename the context to leannet), then:
kubectl --context leannet get nodes -L leanstart.io/host -o wide
```

You should see every node `Ready` with its `leanstart.io/host` label and **private**
internal IP.

---

## Step 2 — Run a distributed devnet

The placement syntax is `name[:count][@host]`:

| Spec | Meaning |
|---|---|
| `ream:5` | 5 ream pods, **auto-spread** across all nodes |
| `ream:2@nbg2` | 2 ream pods pinned to the node labelled `nbg2` |
| `ream:1@nbg1 ream:2@nbg2 ream:2@nbg3` | same client split across hosts (continuous numbering) |
| `ream:2@nbg1 zeam:2@nbg2 grandine:1` | mixed clients; grandine auto-spreads |

The **first pod of the first allocation is the aggregator**. So to put 1 aggregator on the
big box and 2 validators on each smaller box:

```bash
leanstart ream:1@nbg1 ream:2@nbg2 ream:2@nbg3 \
  --skip-kind --context leannet --skip-metrics
```

Flags that make it target the remote cluster:

| Flag | Why |
|---|---|
| `--skip-kind` | Don't create a local kind cluster; deploy to the existing one (enables injected mode) |
| `--context leannet` | The kubeconfig context for your cluster |
| `--skip-metrics` | Don't install the metrics stack per-run (install it once instead — see Step 4) |
| `--active-epoch 8` | *Optional.* Smaller/faster hash-sig keygen — handy for quick tests (default 18) |
| `--genesis-offset 200` | *Optional.* Seconds until genesis; give pods time to pull images + start |

What happens: leanstart generates keys/genesis locally, deploys the Helm chart, the pods
block in an init container, leanstart reads their assigned IPs, regenerates the signed
peer records (ENRs) with the real IPs, injects genesis + each pod's keys into the pods,
and releases them — so every client starts once with correct cross-machine peering.

### Verify

```bash
# placement: which pod is on which node
kubectl --context leannet get pods -n lean-devnet -o wide

# peering + finalization (chain only finalizes if cross-machine gossip works)
kubectl --context leannet logs ream-0-0 -n lean-devnet | grep -E "Connected Peers|Finaliz"
```

---

## Step 3 — Where the logs go

- **On your laptop:** `output/runs/<timestamp>/<pod>.log` (one per pod) plus `run.log`
  (orchestration). `output/runs/latest` symlinks to the newest run. These stream live via
  `kubectl logs -f` and stop if the laptop sleeps/closes.
- **On the cluster (source of truth):** `kubectl --context leannet logs <pod> -n lean-devnet -f`

---

## Step 4 — Metrics (Grafana)

Install the metrics stack **once** on the cluster; afterwards every devnet is scraped
automatically.

```bash
helm repo add prometheus-community https://prometheus-community.github.io/helm-charts
helm repo update
helm upgrade --install lean-prometheus-stack prometheus-community/kube-prometheus-stack \
  --namespace monitoring --create-namespace --kube-context leannet \
  --set alertmanager.enabled=false --set kubeStateMetrics.enabled=false \
  --set nodeExporter.enabled=false --set grafana.adminPassword=admin \
  --set grafana.service.type=NodePort --set grafana.service.nodePort=30300 \
  --set grafana.sidecar.dashboards.folderAnnotation=grafana_folder \
  --set prometheus.prometheusSpec.serviceMonitorSelectorNilUsesHelmValues=false \
  --set prometheus.prometheusSpec.podMonitorSelectorNilUsesHelmValues=false \
  --wait --timeout 10m
```

Then **run devnets WITHOUT `--skip-metrics`** so the chart's `ServiceMonitor` is created
and Prometheus scrapes the pods. Grafana is then at:

```
http://<cp-public-ip>:30300     (admin / admin)
→ dashboard folder "Lean Ethereum Clients"
```

> ⚠️ A NodePort with `admin/admin` is publicly reachable. Set a real password and/or
> restrict the port for anything beyond a throwaway devnet.

If you deployed a devnet *with* `--skip-metrics` and want to scrape it after the fact,
apply a ServiceMonitor manually:

```yaml
apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata: { name: lean-devnet, namespace: lean-devnet, labels: { app.kubernetes.io/part-of: lean-devnet } }
spec:
  selector: { matchLabels: { app.kubernetes.io/part-of: lean-devnet } }
  namespaceSelector: { matchNames: [lean-devnet] }
  endpoints: [ { port: metrics, interval: 15s, path: /metrics } ]
```

---

## Step 5 — Tear down / add capacity

```bash
# tear down the devnet (keeps the cluster + metrics)
leanstart destroy --namespace lean-devnet
# or: helm --kube-context leannet uninstall lean-devnet -n lean-devnet
#     kubectl --context leannet delete ns lean-devnet

# add another machine to run more clients on
scripts/cluster/add-host.sh root@<new-public-ip> <new-private-ip> nbg4
#   then place onto it:  leanstart ream:3@nbg4 ...
```

---

## Supported clients

`ream`, `zeam`, `grandine`, `lantern`, `qlean`, `ethlambda`, `lighthouse` (see the main
[README](../README.md)). All can be placed with `@host`.

## Notes & limits

- **Networking:** flannel pod IPs are routable across nodes, so peering "just works" once
  the cluster is up. Keep nodes on a private network; don't expose 6443/8472/10250 publicly.
- **Storage:** in injected mode there's no shared volume — each pod gets only its own
  validator keys injected, so it scales across machines.
- **Same-client across hosts:** repeating a client (`ream:1@nbg1 ream:2@nbg2`) is supported;
  pods are numbered continuously (`ream_0`, `ream_1`, …) and `ream_0` is the aggregator.
- **`--skip-metrics`** avoids a per-run metrics install; pair it with a one-time cluster
  install (Step 4) so dashboards still work.
