//! Per-run metadata record (`run.json`).
//!
//! Written into each `output/runs/<timestamp>/` dir at deploy time so every run
//! self-describes its topology, config, images, genesis, and the "fix facets"
//! used for filtering/comparison on the devnet showcase site. The companion
//! `metrics.json` (Prometheus snapshot) and full per-pod logs are produced later
//! by `leanstart runs snapshot` — see `src/cli/runs.rs`.

use std::path::Path;
use std::process::Command;
use std::{env, fs, time};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::cli::run::RunArgs;
use crate::config::clients::get_client;
use crate::config::generator::ValidatorConfig;
use crate::config::spec::DevnetSpec;

/// Bump when the on-disk shape changes so the importer can branch.
const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRecord {
    pub schema: u32,
    /// The `<timestamp>` run-dir name (primary key on the site).
    pub run_id: String,
    /// `"devnet4"` | `"devnet5"` — the headline filter.
    pub devnet: String,
    /// Full argv of the invocation, for display.
    pub invocation: String,
    pub namespace: String,
    pub context: String,
    /// Genesis time (unix secs) — the natural run start for metric windows.
    pub started_at: u64,
    /// Wall-clock when this record was written (unix secs).
    pub captured_at: u64,
    pub clients: Vec<ClientRecord>,
    pub topology: Vec<NodeRecord>,
    pub flags: Flags,
    pub genesis: Genesis,
    pub fixes: Fixes,
    /// Free-form: raw image tag(s) and anything not captured structurally.
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientRecord {
    pub name: String,
    pub instances: u32,
    pub host: Option<String>,
    pub image: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeRecord {
    /// Generation node id, e.g. `ream_0`.
    pub node_id: String,
    /// Kubernetes pod name, e.g. `ream-0-0`.
    pub pod: String,
    pub client: String,
    /// Pinned host (`@host`) or the scheduled k8s node name if discoverable.
    pub host: Option<String>,
    /// Live pod IP (best-effort via kubectl; empty if unavailable).
    pub ip: Option<String>,
    /// Where metrics are scraped, `<ip>:8080`.
    pub metrics_endpoint: Option<String>,
    pub is_aggregator: bool,
    pub subnet: u32,
    /// Number of validators this pod owns.
    pub validator_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Flags {
    pub devnet5: bool,
    pub injected: bool,
    pub all_aggregators: bool,
    pub host_network: bool,
    pub skip_kind: bool,
    pub skip_metrics: bool,
    pub image_pull_policy: String,
    pub subnets: u32,
    pub validators_per_pod: u32,
    pub attestation_committee_count: Option<u32>,
    pub active_epoch: u32,
    pub genesis_offset: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Genesis {
    pub genesis_time: Option<u64>,
    pub num_validators: Option<u32>,
    pub seconds_per_slot: Option<u32>,
    pub attestation_committee_count: Option<u32>,
}

/// Boolean facets for the finalization-fix stack, derived from the ream image
/// tag. These are the key comparison axis on the showcase ("which runs had
/// off-loop aggregation AND head-state finalized?"). See the ream-fix-isolation
/// notes for the tag→fix mapping.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Fixes {
    pub proving_conjectured: bool,
    /// WHIR log(1/rate): 1 or 2 (0 = unknown / stock).
    pub log_inv_rate: u8,
    pub gossip_disparity: bool,
    pub block_builder_prefilter: bool,
    pub target_guard: bool,
    pub offloop_aggregation: bool,
    pub headstate_finalized: bool,
}

/// Map a ream image tag to its fix facets. Unknown tags → all-false (caller
/// keeps the raw tag in `notes`).
fn fixes_for_image(image: &str) -> Fixes {
    // Match on the tag portion after the last ':'.
    let tag = image.rsplit(':').next().unwrap_or(image);
    let all = |log_inv_rate: u8| Fixes {
        proving_conjectured: true,
        log_inv_rate,
        gossip_disparity: true,
        block_builder_prefilter: true,
        target_guard: true,
        offloop_aggregation: true,
        headstate_finalized: true,
    };
    match tag {
        // Full working stack (finalizes >=200 / 2k). r2 ships rate=2; the
        // -gd1 suffix is the gossip-disparity variant (still the full set).
        "devnet5-finconsist" => all(1),
        "devnet5-r2" | "devnet5-r2-gd1" => all(2),
        // Isolation builds (see ream-fix-isolation): each drops one or more.
        "devnet5-iso6" => Fixes {
            headstate_finalized: true,
            log_inv_rate: 1,
            ..Default::default()
        },
        "devnet5-ab1" => Fixes {
            log_inv_rate: 2, // proven (no conjecture) + rate2 in the ablation
            gossip_disparity: true,
            block_builder_prefilter: true,
            target_guard: true,
            offloop_aggregation: true,
            headstate_finalized: true,
            proving_conjectured: false,
        },
        "devnet5-core" => Fixes {
            proving_conjectured: true,
            log_inv_rate: 1,
            offloop_aggregation: true,
            headstate_finalized: true,
            ..Default::default()
        },
        "devnet5-core3" => Fixes {
            proving_conjectured: true,
            log_inv_rate: 1,
            block_builder_prefilter: true,
            offloop_aggregation: true,
            headstate_finalized: true,
            ..Default::default()
        },
        "devnet5-c34" => Fixes {
            proving_conjectured: true,
            log_inv_rate: 1,
            block_builder_prefilter: true,
            target_guard: true,
            offloop_aggregation: true,
            headstate_finalized: true,
            ..Default::default()
        },
        // Incremental discovery builds.
        "devnet5-conj-r1" => Fixes {
            proving_conjectured: true,
            log_inv_rate: 1,
            ..Default::default()
        },
        // Stock / unknown.
        _ => Fixes::default(),
    }
}

fn now_secs() -> u64 {
    time::SystemTime::now()
        .duration_since(time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Pull a scalar `KEY: value` out of the genesis config.yaml (best-effort).
fn parse_config_scalar<T: std::str::FromStr>(config: &str, key: &str) -> Option<T> {
    for line in config.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix(key) {
            let rest = rest.trim_start();
            if let Some(val) = rest.strip_prefix(':') {
                return val.trim().parse().ok();
            }
        }
    }
    None
}

/// Best-effort: query each pod's live IP and scheduled node name via kubectl.
/// Returns a map keyed by pod name. Never fails the run — on any error the
/// fields are simply left empty.
fn discover_pod_addrs(context: &str, namespace: &str, pods: &[String]) -> Vec<(String, String)> {
    pods.iter()
        .map(|pod| {
            let q = |jsonpath: &str| -> String {
                Command::new("kubectl")
                    .args([
                        "--context", context, "get", "pod", pod, "-n", namespace, "-o",
                        jsonpath,
                    ])
                    .output()
                    .ok()
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                    .unwrap_or_default()
            };
            let ip = q("jsonpath={.status.podIP}");
            let node = q("jsonpath={.spec.nodeName}");
            (ip, node)
        })
        .collect()
}

/// Build and write `run.json` into `run_dir`. Best-effort: any kubectl/genesis
/// read failures degrade gracefully rather than failing the deploy.
#[allow(clippy::too_many_arguments)]
pub fn write_run_record(
    run_dir: &Path,
    spec: &DevnetSpec,
    vc: &ValidatorConfig,
    args: &RunArgs,
    context: &str,
    pod_names: &[(String, String)],
    genesis_dir: &Path,
) -> Result<()> {
    let run_id = run_dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    // Per-client image + facet derivation. The showcase keys `fixes` off the
    // ream image; for mixed/other clients we still record the image string.
    let clients: Vec<ClientRecord> = spec
        .clients
        .iter()
        .map(|c| ClientRecord {
            name: c.name.clone(),
            instances: c.instances,
            host: c.host.clone(),
            image: get_client(&c.name)
                .map(|d| d.image.to_string())
                .unwrap_or_else(|| "unknown".to_string()),
        })
        .collect();

    // Fixes come from the ream image when present, else the first client image.
    let ream_image = clients
        .iter()
        .find(|c| c.name == "ream")
        .or_else(|| clients.first())
        .map(|c| c.image.clone())
        .unwrap_or_default();
    let fixes = fixes_for_image(&ream_image);

    // Live pod addresses (best-effort).
    let pod_only: Vec<String> = pod_names.iter().map(|(p, _)| p.clone()).collect();
    let addrs = discover_pod_addrs(context, &spec.namespace, &pod_only);

    let topology: Vec<NodeRecord> = pod_names
        .iter()
        .enumerate()
        .map(|(i, (pod, node_id))| {
            let entry = vc.validators.iter().find(|e| &e.name == node_id);
            let (ip, node) = addrs.get(i).cloned().unwrap_or_default();
            let ip = if ip.is_empty() { None } else { Some(ip) };
            let metrics_endpoint = ip.as_ref().map(|ip| format!("{ip}:8080"));
            // Prefer the pinned @host; fall back to the k8s node it landed on.
            let host = entry
                .and_then(|e| e.host.clone())
                .or_else(|| if node.is_empty() { None } else { Some(node) });
            NodeRecord {
                node_id: node_id.clone(),
                pod: pod.clone(),
                client: entry
                    .map(|e| e.client.clone())
                    .unwrap_or_else(|| node_id.split('_').next().unwrap_or("").to_string()),
                host,
                ip,
                metrics_endpoint,
                is_aggregator: entry.map(|e| e.is_aggregator).unwrap_or(false),
                subnet: entry.map(|e| e.subnet).unwrap_or(0),
                validator_count: entry.map(|e| e.count).unwrap_or(0),
            }
        })
        .collect();

    // Genesis scalars from config.yaml (devnet5 uses NUM_VALIDATORS; devnet4
    // write_config_yaml uses VALIDATOR_COUNT — try both).
    let config_txt = fs::read_to_string(genesis_dir.join("config.yaml")).unwrap_or_default();
    let genesis = Genesis {
        genesis_time: parse_config_scalar(&config_txt, "GENESIS_TIME"),
        num_validators: parse_config_scalar(&config_txt, "NUM_VALIDATORS")
            .or_else(|| parse_config_scalar(&config_txt, "VALIDATOR_COUNT")),
        seconds_per_slot: parse_config_scalar(&config_txt, "SECONDS_PER_SLOT"),
        attestation_committee_count: parse_config_scalar(
            &config_txt,
            "ATTESTATION_COMMITTEE_COUNT",
        ),
    };

    let captured_at = now_secs();
    let record = RunRecord {
        schema: SCHEMA_VERSION,
        run_id,
        devnet: if spec.devnet5 { "devnet5" } else { "devnet4" }.to_string(),
        invocation: env::args().collect::<Vec<_>>().join(" "),
        namespace: spec.namespace.clone(),
        context: context.to_string(),
        // GENESIS_TIME is the meaningful "started"; fall back to now.
        started_at: genesis.genesis_time.unwrap_or(captured_at),
        captured_at,
        clients,
        topology,
        flags: Flags {
            devnet5: spec.devnet5,
            injected: spec.injected,
            all_aggregators: spec.all_aggregators,
            host_network: args.host_network,
            skip_kind: args.skip_kind,
            skip_metrics: args.skip_metrics,
            image_pull_policy: args.image_pull_policy.clone(),
            subnets: spec.subnets,
            validators_per_pod: spec.validators_per_pod,
            attestation_committee_count: spec.attestation_committee_count,
            active_epoch: spec.active_epoch,
            genesis_offset: spec.genesis_offset,
        },
        genesis,
        fixes,
        notes: format!("image={ream_image}"),
    };

    let path = run_dir.join("run.json");
    let json = serde_json::to_string_pretty(&record)?;
    fs::write(&path, json).with_context(|| format!("Failed to write {}", path.display()))?;
    println!("Wrote {}", path.display());
    Ok(())
}
