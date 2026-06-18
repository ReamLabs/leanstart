use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::config::clients::get_client;
use crate::config::spec::{DevnetSpec, MAX_SUBNETS};
use crate::keys::keygen::deterministic_privkey;

/// A single entry in the generated validator-config.yaml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorEntry {
    pub name: String,
    /// Client type prefix (e.g. "zeam"). Persisted so we don't have to parse `name`.
    #[serde(skip)]
    pub client: String,
    /// Host to pin this pod to (from `@host`), or None to auto-spread. Deploy-time
    /// metadata only — not part of the genesis input, so skipped in serde.
    #[serde(skip)]
    pub host: Option<String>,
    pub privkey: String,
    #[serde(rename = "enrFields")]
    pub enr_fields: EnrFields,
    #[serde(rename = "metricsPort")]
    pub metrics_port: u16,
    #[serde(rename = "httpPort", skip_serializing_if = "Option::is_none")]
    pub http_port: Option<u16>,
    #[serde(rename = "isAggregator")]
    pub is_aggregator: bool,
    /// Subnet (attestation committee) index this node belongs to.
    pub subnet: u32,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrFields {
    pub ip: String,
    pub quic: u16,
}

/// Top-level validator-config.yaml structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorConfig {
    pub shuffle: String,
    pub deployment_mode: String,
    pub config: ValidatorConfigMeta,
    pub validators: Vec<ValidatorEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorConfigMeta {
    #[serde(rename = "activeEpoch")]
    pub active_epoch: u32,
    #[serde(rename = "keyType")]
    pub key_type: String,
    #[serde(
        rename = "attestation_committee_count",
        skip_serializing_if = "Option::is_none"
    )]
    pub attestation_committee_count: Option<u32>,
}

/// Generate the complete validator-config.yaml from a DevnetSpec.
///
/// When `spec.subnets > 1`, each client's pods are replicated once per subnet.
/// Pod naming is `{client}_{pod_idx}` for single-subnet (backward-compat) and
/// `{client}_s{subnet}_p{pod_idx}` when multi-subnet. Exactly one aggregator
/// is selected per subnet (the first pod of the first client in that subnet).
pub fn generate_validator_config(spec: &DevnetSpec) -> Result<ValidatorConfig> {
    if spec.subnets == 0 || spec.subnets > MAX_SUBNETS {
        bail!(
            "subnets must be between 1 and {} (got {})",
            MAX_SUBNETS,
            spec.subnets
        );
    }

    let mut validators = Vec::new();
    let mut global_pod_index: u32 = 0;
    let multi_subnet = spec.subnets > 1;

    for subnet_idx in 0..spec.subnets {
        let mut first_pod_in_subnet = true;
        // Pod index counter per client name, so the SAME client can appear in
        // multiple allocations with different @host pins and still get unique,
        // continuous pod names (e.g. ream:1@nbg1 ream:2@nbg2 -> ream_0..ream_2).
        let mut per_client_idx: std::collections::HashMap<String, u32> =
            std::collections::HashMap::new();

        // Iterate the client allocations directly (rather than validator_counts())
        // so each entry can carry its `@host` placement. All subnet replicas of a
        // pinned client land on the same host.
        for client in &spec.clients {
            let client_name = &client.name;
            let validator_count = client.instances * spec.validators_per_pod;
            let client_def = get_client(client_name)
                .with_context(|| format!("Unknown client: {client_name}"))?;

            if validator_count == 0 {
                bail!("Client {client_name} has 0 validators allocated");
            }

            let pod_count = validator_count.div_ceil(spec.validators_per_pod);
            let mut remaining = validator_count;

            for _ in 0..pod_count {
                let count = remaining.min(spec.validators_per_pod);
                remaining -= count;

                let pod_idx = {
                    let e = per_client_idx.entry(client_name.clone()).or_insert(0);
                    let v = *e;
                    *e += 1;
                    v
                };

                let name = if multi_subnet {
                    format!("{client_name}_s{subnet_idx}_p{pod_idx}")
                } else {
                    format!("{client_name}_{pod_idx}")
                };
                let privkey = deterministic_privkey(&spec.seed, global_pod_index);

                // Use 0.0.0.0 as placeholder — the genesis tool requires a valid IP.
                // Actual pod IPs are resolved at runtime by the init container.
                let entry = ValidatorEntry {
                    name,
                    client: client_name.clone(),
                    host: client.host_for_subnet(subnet_idx, spec.subnets)?,
                    privkey,
                    enr_fields: EnrFields {
                        ip: "0.0.0.0".to_string(),
                        quic: 9000,
                    },
                    metrics_port: 8080,
                    http_port: if client_def.has_http_port {
                        Some(5055)
                    } else {
                        None
                    },
                    is_aggregator: first_pod_in_subnet || spec.all_aggregators,
                    subnet: subnet_idx,
                    count,
                };

                validators.push(entry);
                global_pod_index += 1;
                first_pod_in_subnet = false;
            }
        }
    }

    // Interleave the registry so that registry position `i` satisfies
    // `i % subnets == subnet_idx`. ream (and leanSpec) assign a validator's
    // attestation committee/subnet as `validator_index % num_committees`, where
    // the index is the position in the registry (annotated_validators / genesis,
    // both built from this vec in order — see append_genesis_validators and
    // pod_validator_indices). Built subnet-major, the per-subnet aggregators (the
    // first pod of each subnet, at positions 0, P, 2P, ... where P = pods/subnet)
    // collapse onto committees `{0, P, 2P, ...} % subnets` — e.g. 6 pods/subnet,
    // 4 subnets => committees 0,2,0,2, leaving committees 1 and 3 with no
    // aggregator so most votes can't be aggregated and justification stalls.
    // Interleaving makes leanstart-subnet S == ream-committee S: each subnet's 6
    // pods all land in committee S and its aggregator (now at position S) is in
    // committee S. Keys stay aligned because genesis pubkeys and injected private
    // keys both follow this same (reordered) vec order.
    if multi_subnet {
        let n = validators.len();
        let per_subnet = n / spec.subnets as usize;
        let mut interleaved = Vec::with_capacity(n);
        for pod_pos in 0..per_subnet {
            for s in 0..spec.subnets as usize {
                interleaved.push(validators[s * per_subnet + pod_pos].clone());
            }
        }
        validators = interleaved;
    }

    let committee_count = if multi_subnet || spec.attestation_committee_count.is_some() {
        Some(spec.effective_committee_count())
    } else {
        None
    };

    Ok(ValidatorConfig {
        shuffle: "roundrobin".to_string(),
        deployment_mode: "kubernetes".to_string(),
        config: ValidatorConfigMeta {
            active_epoch: spec.active_epoch,
            key_type: spec.key_type.clone(),
            attestation_committee_count: committee_count,
        },
        validators,
    })
}

/// Write the validator config to a YAML file.
pub fn write_validator_config(config: &ValidatorConfig, output_dir: &Path) -> Result<()> {
    fs::create_dir_all(output_dir)?;
    let path = output_dir.join("validator-config.yaml");
    let yaml = serde_yaml::to_string(config)?;
    fs::write(&path, yaml)?;
    println!("Wrote {}", path.display());
    Ok(())
}
