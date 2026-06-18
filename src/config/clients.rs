use serde::{Deserialize, Serialize};

/// How a client handles hash-sig keys.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HashSigMode {
    /// No hash-sig flags (ethlambda, ream, zeam, lighthouse).
    None,
    /// Per-validator key files via --xmss-pk / --xmss-sk (qlean).
    PerValidator,
    /// Directory flag via --hash-sig-key-dir (grandine, lantern).
    Directory,
}

/// Definition of a Lean client type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientDef {
    pub name: &'static str,
    pub image: &'static str,
    /// Whether the image tag varies by CPU architecture (lantern).
    pub arch_aware: bool,
    /// Kubernetes securityContext.seccompProfile.type = Unconfined (zeam).
    pub seccomp_unconfined: bool,
    pub hash_sig_mode: HashSigMode,
    /// Whether the client has a separate HTTP port.
    pub has_http_port: bool,
}

/// All known client definitions, extracted from client-cmds/*.sh.
pub static CLIENTS: &[ClientDef] = &[
    ClientDef {
        name: "ethlambda",
        image: "ghcr.io/lambdaclass/ethlambda:devnet4",
        arch_aware: false,
        seccomp_unconfined: false,
        hash_sig_mode: HashSigMode::None,
        has_http_port: true,
    },
    ClientDef {
        name: "qlean",
        image: "qdrvm/qlean-mini:devnet-4",
        arch_aware: true,
        seccomp_unconfined: false,
        hash_sig_mode: HashSigMode::Directory,
        has_http_port: true,
    },
    ClientDef {
        name: "ream",
        image: "ttl.sh/leanstart-ream-parity-v6:24h",
        arch_aware: false,
        seccomp_unconfined: false,
        hash_sig_mode: HashSigMode::None,
        has_http_port: true,
    },
    ClientDef {
        name: "zeam",
        image: "0xpartha/zeam:devnet5",
        arch_aware: false,
        seccomp_unconfined: true,
        hash_sig_mode: HashSigMode::None,
        has_http_port: true,
    },
    ClientDef {
        name: "grandine",
        image: "sifrai/lean:devnet-4",
        arch_aware: false,
        seccomp_unconfined: false,
        hash_sig_mode: HashSigMode::Directory,
        has_http_port: true,
    },
    ClientDef {
        name: "lantern",
        image: "piertwo/lantern:v0.0.5",
        arch_aware: false,
        seccomp_unconfined: false,
        hash_sig_mode: HashSigMode::Directory,
        has_http_port: true,
    },
    ClientDef {
        name: "lighthouse",
        image: "hopinheimer/lighthouse:latest",
        arch_aware: false,
        seccomp_unconfined: false,
        hash_sig_mode: HashSigMode::None,
        has_http_port: false,
    },
];

/// Look up a client definition by name.
pub fn get_client(name: &str) -> Option<&'static ClientDef> {
    CLIENTS.iter().find(|c| c.name == name)
}

/// Build the container args list for a given client pod.
///
/// Placeholders are resolved at generation time:
/// - `node_id`: e.g. "ethlambda_0"
/// - `is_aggregator`: whether this pod is the aggregator
/// - `attestation_committee_count`: optional override
/// - `aggregate_subnet_ids`: CSV of subnet ids (e.g. "0,1,2") an aggregator must
///   subscribe to. Currently honoured only by zeam.
pub fn build_args(
    client: &ClientDef,
    node_id: &str,
    is_aggregator: bool,
    attestation_committee_count: Option<u32>,
    aggregate_subnet_ids: Option<&str>,
    devnet5: bool,
) -> Vec<String> {
    let mut args = Vec::new();

    // ethlambda has a distinct devnet5 CLI (reads the ream-generated registry:
    // config.yaml + annotated_validators.yaml + nodes.yaml + per-validator XMSS
    // keys). Handle it before the devnet4 match below.
    if client.name == "ethlambda" && devnet5 {
        args.extend_from_slice(&[
            "--genesis".into(),
            "/config/config.yaml".into(),
            "--validators".into(),
            "/config/annotated_validators.yaml".into(),
            "--bootnodes".into(),
            "/config/nodes.yaml".into(),
            "--validator-config".into(),
            "/config/validator-config.yaml".into(),
            "--hash-sig-keys-dir".into(),
            "/config/hash-sig-keys".into(),
            "--gossipsub-port".into(),
            "9000".into(),
            "--http-address".into(),
            "0.0.0.0".into(),
            "--api-port".into(),
            "5055".into(),
            "--metrics-port".into(),
            "8080".into(),
            "--node-id".into(),
            node_id.into(),
            "--node-key".into(),
            format!("/config/{node_id}.key"),
            "--data-dir".into(),
            "/data".into(),
        ]);
        if is_aggregator {
            args.push("--is-aggregator".into());
        }
        if let Some(count) = attestation_committee_count {
            args.push("--attestation-committee-count".into());
            args.push(count.to_string());
        }
        if is_aggregator {
            if let Some(ids) = aggregate_subnet_ids {
                if ids.contains(',') {
                    args.push("--aggregate-subnet-ids".into());
                    args.push(ids.into());
                }
            }
        }
        return args;
    }

    // zeam devnet5 CLI (`node` subcommand). Differs from the devnet4 path: hyphenated
    // `--custom-genesis` / `--validator-config` (both the /config dir), and keys via
    // `--sig-keys-dir` (relative to the genesis dir). Reads bootnodes from nodes.yaml
    // in the genesis dir. Genesis derives from config.yaml (no genesis.ssz).
    if client.name == "zeam" && devnet5 {
        args.extend_from_slice(&[
            "node".into(),
            "--custom-genesis".into(),
            "/config".into(),
            "--validator-config".into(),
            "/config".into(),
            "--node-id".into(),
            node_id.into(),
            "--node-key".into(),
            format!("/config/{node_id}.key"),
            "--sig-keys-dir".into(),
            "hash-sig-keys".into(),
            "--data-dir".into(),
            "/data".into(),
            "--metrics-enable".into(),
            "--metrics-port".into(),
            "8080".into(),
            "--api-port".into(),
            "5055".into(),
        ]);
        if is_aggregator {
            args.push("--is-aggregator".into());
        }
        if let Some(count) = attestation_committee_count {
            args.push("--attestation-committee-count".into());
            args.push(count.to_string());
        }
        if is_aggregator {
            if let Some(ids) = aggregate_subnet_ids {
                if ids.contains(',') {
                    args.push("--aggregate-subnet-ids".into());
                    args.push(ids.into());
                }
            }
        }
        return args;
    }

    // lantern devnet5 (v0.0.5) CLI. Changed from v0.0.4: `--validator_config` is now a
    // DIRECTORY (annotated_validators.yaml + validator-config.yaml), `--genesis-state`
    // is deprecated/ignored (devnet5 has no genesis.ssz), and keys come from
    // `--hash-sig-key-dir` (alias of `--xmss-key-dir`).
    if client.name == "lantern" && devnet5 {
        args.extend_from_slice(&[
            "--data-dir".into(),
            "/data".into(),
            "--genesis-config".into(),
            "/config/config.yaml".into(),
            "--validator_config".into(),
            "/config".into(),
            "--nodes-path".into(),
            "/config/nodes.yaml".into(),
            "--node-id".into(),
            node_id.into(),
            "--node-key-path".into(),
            format!("/config/{node_id}.key"),
            "--listen-address".into(),
            "/ip4/0.0.0.0/udp/9000/quic-v1".into(),
            "--metrics-port".into(),
            "8080".into(),
            "--http-port".into(),
            "5055".into(),
            "--hash-sig-key-dir".into(),
            "/config/hash-sig-keys".into(),
            "--log-level".into(),
            "info".into(),
        ]);
        if is_aggregator {
            args.push("--is-aggregator".into());
        }
        if let Some(count) = attestation_committee_count {
            args.push("--attestation-committee-count".into());
            args.push(count.to_string());
        }
        if is_aggregator {
            if let Some(ids) = aggregate_subnet_ids {
                if ids.contains(',') {
                    args.push("--aggregate-subnet-ids".into());
                    args.push(ids.into());
                }
            }
        }
        return args;
    }

    match client.name {
        "ethlambda" => {
            // ethlambda:devnet4's CLI (pq-devnet-4 spec) reads the eth-beacon-genesis
            // registry directly: config.yaml + annotated_validators.yaml + nodes.yaml
            // + validator-config.yaml + per-validator hash-sig keys. (The older
            // --custom-network-config-dir flag was removed.)
            args.extend_from_slice(&[
                "--genesis".into(),
                "/config/config.yaml".into(),
                "--validators".into(),
                "/config/annotated_validators.yaml".into(),
                "--bootnodes".into(),
                "/config/nodes.yaml".into(),
                "--validator-config".into(),
                "/config/validator-config.yaml".into(),
                "--hash-sig-keys-dir".into(),
                "/config/hash-sig-keys".into(),
                "--gossipsub-port".into(),
                "9000".into(),
                "--http-address".into(),
                "0.0.0.0".into(),
                "--api-port".into(),
                "5055".into(),
                "--metrics-port".into(),
                "8080".into(),
                "--node-id".into(),
                node_id.into(),
                "--node-key".into(),
                format!("/config/{node_id}.key"),
                "--data-dir".into(),
                "/data".into(),
            ]);
        }
        "qlean" => {
            args.extend_from_slice(&[
                "--genesis-dir".into(),
                "/config".into(),
                "--data-dir".into(),
                "/data".into(),
                "--node-id".into(),
                node_id.into(),
                "--node-key".into(),
                format!("/config/{node_id}.key"),
                "--listen-addr".into(),
                "/ip4/0.0.0.0/udp/9000/quic-v1".into(),
                "--metrics-host".into(),
                "0.0.0.0".into(),
                "--metrics-port".into(),
                "8080".into(),
                "--api-host".into(),
                "0.0.0.0".into(),
                "--api-port".into(),
                "5055".into(),
            ]);
        }
        "ream" => {
            args.extend_from_slice(&[
                "--data-dir".into(),
                "/data".into(),
                "lean_node".into(),
                "--network".into(),
                "/config/config.yaml".into(),
                "--validator-registry-path".into(),
                "/config/annotated_validators.yaml".into(),
                "--bootnodes".into(),
                "/config/nodes.yaml".into(),
                "--node-id".into(),
                node_id.into(),
                "--private-key-path".into(),
                format!("/config/{node_id}.key"),
                "--socket-port".into(),
                "9000".into(),
                "--metrics".into(),
                "--metrics-address".into(),
                "0.0.0.0".into(),
                "--metrics-port".into(),
                "8080".into(),
                "--http-address".into(),
                "0.0.0.0".into(),
                "--http-port".into(),
                "5055".into(),
            ]);
        }
        "zeam" => {
            args.extend_from_slice(&[
                "node".into(),
                "--custom_genesis".into(),
                "/config".into(),
                "--validator_config".into(),
                "/config".into(),
                "--data-dir".into(),
                "/data".into(),
                "--node-id".into(),
                node_id.into(),
                "--node-key".into(),
                format!("/config/{node_id}.key"),
                "--metrics-enable".into(),
                "--metrics-port".into(),
                "8080".into(),
                "--api-port".into(),
                "5055".into(),
            ]);
        }
        "grandine" => {
            args.extend_from_slice(&[
                "--genesis".into(),
                "/config/config.yaml".into(),
                "--validator-registry-path".into(),
                "/config/annotated_validators.yaml".into(),
                "--bootnodes".into(),
                "/config/nodes.yaml".into(),
                "--node-id".into(),
                node_id.into(),
                "--node-key".into(),
                format!("/config/{node_id}.key"),
                "--port".into(),
                "9000".into(),
                "--address".into(),
                "0.0.0.0".into(),
                "--metrics".into(),
                "--metrics-address".into(),
                "0.0.0.0".into(),
                "--metrics-port".into(),
                "8080".into(),
                "--http-address".into(),
                "0.0.0.0".into(),
                "--http-port".into(),
                "5055".into(),
                "--hash-sig-key-dir".into(),
                "/config/hash-sig-keys".into(),
            ]);
        }
        "lantern" => {
            args.extend_from_slice(&[
                "--data-dir".into(),
                "/data".into(),
                "--genesis-config".into(),
                "/config/config.yaml".into(),
                "--validator-registry-path".into(),
                "/config/annotated_validators.yaml".into(),
                "--genesis-state".into(),
                "/config/genesis.ssz".into(),
                "--validator-config".into(),
                "/config/validator-config.yaml".into(),
                "--nodes-path".into(),
                "/config/nodes.yaml".into(),
                "--node-id".into(),
                node_id.into(),
                "--node-key-path".into(),
                format!("/config/{node_id}.key"),
                "--listen-address".into(),
                "/ip4/0.0.0.0/udp/9000/quic-v1".into(),
                "--metrics-port".into(),
                "8080".into(),
                "--http-port".into(),
                "5055".into(),
                "--log-level".into(),
                "info".into(),
                "--hash-sig-key-dir".into(),
                "/config/hash-sig-keys".into(),
            ]);
        }
        "lighthouse" => {
            args.extend_from_slice(&[
                "lighthouse".into(),
                "lean_node".into(),
                "--datadir".into(),
                "/data".into(),
                "--config".into(),
                "/config/config.yaml".into(),
                "--validators".into(),
                "/config/validator-config.yaml".into(),
                "--nodes".into(),
                "/config/nodes.yaml".into(),
                "--node-id".into(),
                node_id.into(),
                "--private-key".into(),
                format!("/config/{node_id}.key"),
                "--genesis-json".into(),
                "/config/genesis.json".into(),
                "--socket-port".into(),
                "9000".into(),
                "--metrics".into(),
                "--metrics-address".into(),
                "0.0.0.0".into(),
                "--metrics-port".into(),
                "8080".into(),
            ]);
        }
        _ => {}
    }

    if is_aggregator {
        args.push("--is-aggregator".into());
    }
    if matches!(client.name, "zeam" | "ethlambda" | "ream") {
        if let Some(count) = attestation_committee_count {
            args.push("--attestation-committee-count".into());
            args.push(count.to_string());
        }
    }
    if is_aggregator && matches!(client.name, "zeam" | "ethlambda" | "ream") {
        if let Some(ids) = aggregate_subnet_ids {
            if ids.contains(',') {
                args.push("--aggregate-subnet-ids".into());
                args.push(ids.into());
            }
        }
    }

    args
}
