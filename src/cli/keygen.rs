use std::path::PathBuf;

use anyhow::Result;
use clap::Args;

use crate::keys::keygen::generate_hash_sig_keys;

/// Pre-generate a pool of hash-sig keys for reuse across devnet runs.
///
/// Example:
///   leanstart keygen --count 100 --output ./key-pool
///
/// Then use the pool with:
///   leanstart run ream:6 --subnets 2 --key-pool ./key-pool
#[derive(Debug, Args)]
pub struct KeygenArgs {
    /// Number of validator key pairs to generate.
    #[arg(long, default_value = "100")]
    pub count: u32,

    /// Directory to write the generated keys into.
    /// Keys land in <output>/hash-sig-keys/.
    #[arg(long, default_value = "./key-pool")]
    pub output: PathBuf,

    /// Hash-sig active epoch exponent (2^N). Must match the value used in `run`.
    #[arg(long, default_value = "18")]
    pub active_epoch: u32,
}

pub fn run(args: KeygenArgs) -> Result<()> {
    println!(
        "Generating {} hash-sig key pairs (active_epoch=2^{}) into {}...",
        args.count,
        args.active_epoch,
        args.output.display()
    );
    generate_hash_sig_keys(args.count, args.active_epoch, &args.output)?;
    println!(
        "\nDone. Reuse with: leanstart run <clients> --key-pool {}",
        args.output.display()
    );
    Ok(())
}
