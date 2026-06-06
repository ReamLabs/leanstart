# leannet — the leanstart k3s cluster

A multi-node [k3s](https://k3s.io) cluster spanning Hetzner servers, joined over a
Hetzner Cloud **private network**. `leanstart` deploys distributed devnets onto it and
places clients on specific hosts via `@host`.

## Current fleet

| k8s node | host label | role | public IP | private IP | NIC |
|---|---|---|---|---|---|
| ubuntu-32gb-nbg1-1 | `nbg1` | control-plane | 178.104.139.49 | 10.0.0.4 | enp7s0 |
| ubuntu-16gb-nbg1-1 | `nbg2` | worker | 5.75.151.239 | 10.0.0.3 | enp7s0 |
| ubuntu-16gb-nbg1-2 | `nbg3` | worker | 46.225.52.108 | 10.0.0.2 | enp7s0 |

- Private network: `10.0.0.0/16` (Hetzner Cloud, eu-central).
- Pod/flannel network: `10.42.0.0/16` (flannel VXLAN over `enp7s0`).
- Nodes carry `leanstart.io/host=<label>`; `leanstart <client>@<label>` pins there.

## Laptop access

The cluster context is `leannet` in `~/.kube/config` (API reached via the control-plane
public IP, which is in the cert SAN list):

```bash
kubectl --context leannet get nodes -o wide
```

## How the control-plane was installed

```bash
curl -sfL https://get.k3s.io | sudo INSTALL_K3S_EXEC="server \
  --node-ip=10.0.0.4 --flannel-iface=enp7s0 --advertise-address=10.0.0.4 \
  --tls-san=10.0.0.4 --tls-san=178.104.139.49 \
  --write-kubeconfig-mode 0644 --node-label leanstart.io/host=nbg1" sh -
```

Workers were joined with the same private-network bindings (see `add-host.sh`).

## Adding a host

1. In the Hetzner console, attach the new server to the same private network.
2. From your laptop:

```bash
scripts/cluster/add-host.sh <user@public-ip> <private-ip> <host-label> [iface]
# e.g.
scripts/cluster/add-host.sh root@5.75.151.240 10.0.0.5 nbg4
```

Overridable via env: `LEANNET_CP_SSH`, `LEANNET_CP_PRIV_IP`, `LEANNET_SSH_KEY`.

## Firewall

Intra-private-network traffic (`10.0.0.0/16`) is trusted. Required k3s ports between
nodes: `6443/tcp` (API), `8472/udp` (flannel VXLAN), `10250/tcp` (kubelet). If you add a
Hetzner Cloud Firewall, allow these on the private subnet and keep them closed on the
public interface.
