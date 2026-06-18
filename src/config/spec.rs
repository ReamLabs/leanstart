use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// High-level specification for a devnet deployment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevnetSpec {
    /// Client allocations as (client_name, instance_count).
    pub clients: Vec<ClientAllocation>,
    /// Number of validators assigned to each pod.
    pub validators_per_pod: u32,
    /// Kubernetes namespace.
    pub namespace: String,
    /// Output directory for generated artifacts.
    pub output_dir: PathBuf,
    /// Exponent for hash-sig active epochs (2^active_epoch).
    pub active_epoch: u32,
    /// Key type (e.g., "hash-sig").
    pub key_type: String,
    /// Seed for deterministic key generation.
    pub seed: [u8; 32],
    /// Seconds from now until genesis time.
    pub genesis_offset: u32,
    /// Kubernetes storage class for PVCs.
    pub storage_class: Option<String>,
    /// Number of bootnode pods per client type.
    pub bootnode_count: u32,
    /// Number of attestation subnets (1..=5). Each client allocation is replicated
    /// once per subnet, and `attestation_committee_count` is set to this value.
    pub subnets: u32,
    /// Explicit override for `config.attestation_committee_count`. Defaults to
    /// `subnets` when None.
    pub attestation_committee_count: Option<u32>,
    /// Multi-node "injected" peering/key delivery mode (set for remote clusters,
    /// i.e. `--skip-kind`). When true the orchestrator gates the init container
    /// and injects IP-correct genesis + per-pod keys instead of using the
    /// kind-only shared PVC + container-restart path.
    pub injected: bool,
    /// devnet5 mode: generate keys + genesis via `ream generate_validator_registry`
    /// (ream-native, hash-sig scheme devnet5 accepts) instead of the devnet4
    /// `hash-sig-cli` + `eth-beacon-genesis` GENESIS_VALIDATORS path. ream-only.
    pub devnet5: bool,
    /// Make every pod an aggregator (--is-aggregator) instead of just the first
    /// pod per subnet. Removes the single-aggregator dependency so each
    /// validator's attestation is always aggregated/broadcast every slot.
    pub all_aggregators: bool,
}

/// Maximum number of subnets supported (matches lean-quickstart MAX_SUBNETS).
pub const MAX_SUBNETS: u32 = 5;

/// A client type and how many instances (pods) to run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientAllocation {
    pub name: String,
    pub instances: u32,
    /// Optional host to pin this client's pods to, from the `@host` suffix
    /// (e.g. `ream:3@nbg1`). Maps to the node label `leanstart.io/host=<host>`.
    /// `None` => pods auto-spread across all nodes.
    #[serde(default)]
    pub host: Option<String>,
}

impl DevnetSpec {
    /// Total number of validators across all clients (counts every subnet).
    #[allow(dead_code)] // used by integration tests
    pub fn total_validators(&self) -> u32 {
        self.clients.iter().map(|c| c.instances).sum::<u32>()
            * self.validators_per_pod
            * self.subnets
    }

    /// Effective attestation_committee_count emitted in config: explicit override
    /// when set, otherwise the number of subnets.
    pub fn effective_committee_count(&self) -> u32 {
        self.attestation_committee_count.unwrap_or(self.subnets)
    }
}

impl ClientAllocation {
    /// Resolve this allocation's `@host` pin for a given subnet. A single host
    /// (or `None`) applies to every subnet; a comma-separated list must have one
    /// entry per subnet, and subnet `subnet_idx` lands on that entry. This is what
    /// lets `ream:1@big0,big1,big2,big3 --subnets 4` put each subnet's replica on
    /// a distinct host (e.g. one aggregator per big node).
    pub fn host_for_subnet(
        &self,
        subnet_idx: u32,
        subnets: u32,
    ) -> anyhow::Result<Option<String>> {
        let Some(raw) = &self.host else {
            return Ok(None);
        };
        let parts: Vec<&str> = raw.split(',').collect();
        match parts.len() {
            1 => Ok(Some(parts[0].to_string())),
            n if n as u32 == subnets => Ok(Some(parts[subnet_idx as usize].to_string())),
            n => anyhow::bail!(
                "@host list for '{}' has {n} entries; must be 1 or == subnets ({subnets})",
                self.name
            ),
        }
    }
}

/// Parse a client spec string like "ream", "zeam:2", "grandine:5", or with a
/// host pin: "ream:3@nbg1", "zeam@nbg2".
///
/// Grammar: `<name>[:<count>][@<host>]`. The `@host` pins the client's pods to
/// the node labelled `leanstart.io/host=<host>`; omitting it auto-spreads.
pub fn parse_client_spec(spec: &str) -> anyhow::Result<ClientAllocation> {
    // Peel off an optional "@host" suffix first. The host may be a single label
    // (`@nbg1`, shared across all subnets) or a comma-separated list whose length
    // equals the subnet count (`@big0,big1,big2,big3`), placing each subnet's
    // replica of this allocation on a distinct host. Validate every token.
    let (left, host) = match spec.split_once('@') {
        Some((l, h)) => {
            if h.is_empty() {
                anyhow::bail!("Empty host in client spec '{spec}'. Use 'name:count@host'");
            }
            for token in h.split(',') {
                if !is_dns_label(token) {
                    anyhow::bail!(
                        "Invalid host '{token}' in '{spec}'. Hosts must be lowercase \
                         alphanumeric or '-' (a Kubernetes label value)"
                    );
                }
            }
            (l, Some(h.to_string()))
        }
        None => (spec, None),
    };

    let parts: Vec<&str> = left.split(':').collect();
    match parts.len() {
        1 if !parts[0].is_empty() => Ok(ClientAllocation {
            name: parts[0].to_string(),
            instances: 1,
            host,
        }),
        2 if !parts[0].is_empty() => Ok(ClientAllocation {
            name: parts[0].to_string(),
            instances: parts[1]
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid instance count in '{spec}'"))?,
            host,
        }),
        _ => anyhow::bail!(
            "Invalid client spec '{spec}'. Use 'name', 'name:count', or 'name:count@host'"
        ),
    }
}

/// Whether `s` is a valid Kubernetes label value usable as a `@host` token:
/// non-empty, <=63 chars, alphanumeric/`-`/`_`/`.`, starting and ending
/// alphanumeric. We keep it conservative (DNS-label-ish) since it lands in a
/// `nodeSelector`.
fn is_dns_label(s: &str) -> bool {
    if s.is_empty() || s.len() > 63 {
        return false;
    }
    let bytes = s.as_bytes();
    let alnum = |b: u8| b.is_ascii_alphanumeric();
    if !alnum(bytes[0]) || !alnum(bytes[bytes.len() - 1]) {
        return false;
    }
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}
