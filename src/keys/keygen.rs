use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Generate a deterministic Ed25519 private key from seed + index.
pub fn deterministic_privkey(seed: &[u8; 32], index: u32) -> String {
    let mut mac = HmacSha256::new_from_slice(seed).expect("HMAC accepts any key size");
    mac.update(&index.to_be_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// Write node private key files to the output directory.
/// Each validator entry gets a `{name}.key` file containing its hex privkey.
pub fn write_node_keys(
    validators: &[(String, String)], // (name, privkey_hex)
    output_dir: &Path,
) -> Result<()> {
    fs::create_dir_all(output_dir)?;
    for (name, privkey) in validators {
        let path = output_dir.join(format!("{name}.key"));
        fs::write(&path, privkey)?;
    }
    println!("Wrote {} node key files to {}", validators.len(), output_dir.display());
    Ok(())
}

/// Generate hash-sig validator keys using the hash-sig-cli Docker image.
///
/// Runs: `docker run blockblaz/hash-sig-cli:latest generate --num-validators N
///        --log-num-active-epochs E --output-dir /genesis/hash-sig-keys --export-format both`
pub fn generate_hash_sig_keys(
    num_validators: u32,
    active_epoch: u32,
    output_dir: &Path,
) -> Result<()> {
    let keys_dir = output_dir.join("hash-sig-keys");
    fs::create_dir_all(&keys_dir)?;

    let uid = unsafe { libc::getuid() };
    let gid = unsafe { libc::getgid() };

    let status = Command::new("docker")
        .args([
            "run",
            "--rm",
            "--user",
            &format!("{uid}:{gid}"),
            "-v",
            &format!("{}:/genesis", output_dir.display()),
            "blockblaz/hash-sig-cli:latest",
            "generate",
            "--num-validators",
            &num_validators.to_string(),
            "--log-num-active-epochs",
            &active_epoch.to_string(),
            "--output-dir",
            "/genesis/hash-sig-keys",
            "--export-format",
            "both",
        ])
        .status()
        .context("Failed to run hash-sig-cli Docker container")?;

    if !status.success() {
        bail!("hash-sig-cli exited with status {status}");
    }

    println!(
        "Generated hash-sig keys for {num_validators} validators in {}",
        keys_dir.display()
    );
    Ok(())
}

/// Materialize hash-sig keys for `num_validators` by copying from a pre-generated
/// pool (`leanstart keygen`) instead of running the slow hash-sig-cli Docker keygen.
///
/// hash-sig keys are deterministic, so a pool built on any machine with the same
/// `blockblaz/hash-sig-cli` image is byte-identical — generate it once on a fast
/// laptop and reuse it for every (much slower) server deploy. The pool MUST have
/// been generated with the same `active_epoch` (different `log_num_active_epochs`
/// ⇒ structurally different keys) and contain at least `num_validators` — both
/// are validated against the pool manifest so a mismatch fails loudly instead of
/// shipping wrong keys. The full manifest is copied as-is; `filtered_manifest`
/// slices it per-pod downstream.
pub fn copy_keys_from_pool(
    pool_dir: &Path,
    num_validators: u32,
    active_epoch: u32,
    output_dir: &Path,
) -> Result<()> {
    let pool_keys = pool_dir.join("hash-sig-keys");
    let manifest = pool_keys.join("validator-keys-manifest.yaml");
    let manifest_text = fs::read_to_string(&manifest).with_context(|| {
        format!(
            "Failed to read pool manifest {} (did you run `leanstart keygen --output {}`?)",
            manifest.display(),
            pool_dir.display()
        )
    })?;

    // Different log_num_active_epochs ⇒ different keys; refuse a silent mismatch.
    let want = format!("log_num_active_epochs: {active_epoch}");
    if !manifest_text.lines().any(|line| line.trim() == want) {
        let got = manifest_text
            .lines()
            .find(|line| line.trim_start().starts_with("log_num_active_epochs:"))
            .map(str::trim)
            .unwrap_or("<none>");
        bail!(
            "Key pool active_epoch mismatch: deploy wants `{want}` but pool has `{got}`. \
             Regenerate with `leanstart keygen --active-epoch {active_epoch}`."
        );
    }

    let pool_count = manifest_text
        .lines()
        .find_map(|line| line.trim().strip_prefix("num_validators:"))
        .and_then(|n| n.trim().parse::<u32>().ok())
        .unwrap_or(0);
    if pool_count < num_validators {
        bail!(
            "Key pool too small: need {num_validators} validators but pool has {pool_count}. \
             Regenerate with `leanstart keygen --count {num_validators}`."
        );
    }

    // Only the `.ssz` keys are consumed downstream (injection ships
    // `validator_N_*_key_sk.ssz`; pubkeys are carried inline in the manifest).
    // The `.json` exports are ~55 MB each and unused, so we skip them — keeping
    // a pool materialization cheap and the on-disk pool ~7x smaller.
    let keys_dir = output_dir.join("hash-sig-keys");
    fs::create_dir_all(&keys_dir)?;
    for v in 0..num_validators {
        for role in ["attester", "proposer"] {
            for kind in ["pk", "sk"] {
                let name = format!("validator_{v}_{role}_key_{kind}.ssz");
                let src = pool_keys.join(&name);
                fs::copy(&src, keys_dir.join(&name))
                    .with_context(|| format!("Failed to copy pool key {}", src.display()))?;
            }
        }
    }
    fs::copy(&manifest, keys_dir.join("validator-keys-manifest.yaml"))?;
    println!(
        "Materialized hash-sig keys for {num_validators} validators from pool {} (active_epoch=2^{active_epoch})",
        pool_dir.display()
    );
    Ok(())
}
