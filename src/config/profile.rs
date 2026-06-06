//! Optional user profile at `~/.leanstart/config.yaml` that supplies defaults
//! for `leanstart run`, so a remote cluster can be targeted without repeating
//! `--skip-kind --context ... --skip-metrics` on every invocation.
//!
//! Example `~/.leanstart/config.yaml`:
//!
//! ```yaml
//! context: leannet      # kubeconfig context of your cluster
//! skip_kind: true       # deploy to that cluster (don't create a local kind)
//! skip_metrics: true    # don't (re)install the metrics stack per run
//! namespace: lean-devnet
//! ```
//!
//! Explicit CLI flags always win; the profile only fills in unset values.

use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
pub struct Profile {
    pub context: Option<String>,
    pub skip_kind: Option<bool>,
    pub skip_metrics: Option<bool>,
    pub namespace: Option<String>,
    pub storage_class: Option<String>,
}

impl Profile {
    /// Load `~/.leanstart/config.yaml`, or an empty profile if absent. A malformed
    /// file is warned about and ignored rather than failing the run.
    pub fn load() -> Self {
        let Some(home) = std::env::var_os("HOME") else {
            return Self::default();
        };
        let path = std::path::Path::new(&home).join(".leanstart/config.yaml");
        let Ok(content) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        match serde_yaml::from_str(&content) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Warning: ignoring {} ({e})", path.display());
                Self::default()
            }
        }
    }

    /// True if any field is set (i.e. a usable profile file was found).
    pub fn is_active(&self) -> bool {
        self.context.is_some()
            || self.skip_kind.is_some()
            || self.skip_metrics.is_some()
            || self.namespace.is_some()
            || self.storage_class.is_some()
    }
}
