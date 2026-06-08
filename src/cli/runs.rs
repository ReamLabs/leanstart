//! `leanstart runs` — post-deploy capture for the devnet showcase.
//!
//! `leanstart run` deploys and exits; the devnet keeps running. These
//! subcommands are invoked later (before `destroy`) to durably capture what a
//! run produced:
//!   - `runs snapshot` — dump full per-pod logs + a Prometheus range snapshot
//!     (`metrics.json`) with an outcome summary into the run dir.
//!   - `runs push` — bundle `run.json` + `metrics.json` + gzipped logs and POST
//!     them to the showcase ingest API.

use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::{fs, thread, time};

use anyhow::{bail, Context, Result};
use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};

use crate::config::profile::Profile;

/// Metrics scraped from each pod and snapshotted from Prometheus.
const METRICS: &[&str] = &[
    "lean_finalized_slot",
    "lean_justified_slot",
    "lean_head_slot",
    "lean_current_slot",
    "lean_safe_target_slot",
    // Committee-signature aggregation timing is a histogram; capture the
    // monotonic sum + count so we can derive the mean per-call time.
    "lean_committee_signatures_aggregation_time_seconds_sum",
    "lean_committee_signatures_aggregation_time_seconds_count",
];

#[derive(Debug, Args)]
pub struct RunsArgs {
    #[command(subcommand)]
    pub command: RunsCommand,
}

#[derive(Debug, Subcommand)]
pub enum RunsCommand {
    /// Snapshot logs + Prometheus metrics for a run into its run dir.
    Snapshot(SnapshotArgs),
    /// Upload a run (run.json + metrics.json + logs) to the showcase.
    Push(PushArgs),
}

#[derive(Debug, Args)]
pub struct SnapshotArgs {
    /// Run id (the `output/runs/<id>` dir name). Defaults to the latest run.
    pub run_id: Option<String>,

    /// Output directory containing `runs/`.
    #[arg(long, default_value = "./output")]
    pub output_dir: PathBuf,

    /// kubectl context. Falls back to the profile, then `leannet`.
    #[arg(long)]
    pub context: Option<String>,

    /// Namespace the devnet runs in.
    #[arg(long, default_value = "lean-devnet")]
    pub namespace: String,

    /// Prometheus base URL. If unset, a temporary port-forward to the cluster
    /// Prometheus is set up automatically.
    #[arg(long)]
    pub prometheus: Option<String>,

    /// Prometheus svc reference for the auto port-forward.
    #[arg(long, default_value = "svc/lean-prometheus-stack-kube-prometheus")]
    pub prometheus_svc: String,

    /// Namespace of the Prometheus service.
    #[arg(long, default_value = "monitoring")]
    pub prometheus_namespace: String,

    /// query_range step in seconds.
    #[arg(long, default_value = "30")]
    pub step: u32,

    /// Skip the full per-pod log dump.
    #[arg(long)]
    pub no_logs: bool,

    /// Skip the Prometheus metrics snapshot.
    #[arg(long)]
    pub no_metrics: bool,
}

#[derive(Debug, Args)]
pub struct PushArgs {
    /// Run id to push. Defaults to the latest run. Ignored with --all.
    pub run_id: Option<String>,

    /// Output directory containing `runs/`.
    #[arg(long, default_value = "./output")]
    pub output_dir: PathBuf,

    /// Push every run dir under `runs/` (backfill).
    #[arg(long)]
    pub all: bool,

    /// Showcase base URL (e.g. https://devnets.example.com). Falls back to the
    /// profile `showcase_url`.
    #[arg(long, env = "LEANSTART_SHOWCASE_URL")]
    pub url: Option<String>,

    /// Bearer token for the ingest API. Falls back to the profile
    /// `showcase_token`.
    #[arg(long, env = "LEANSTART_SHOWCASE_TOKEN")]
    pub token: Option<String>,
}

pub fn run(args: RunsArgs) -> Result<()> {
    match args.command {
        RunsCommand::Snapshot(a) => snapshot(a),
        RunsCommand::Push(a) => push(a),
    }
}

// ---------------------------------------------------------------------------
// snapshot
// ---------------------------------------------------------------------------

/// Minimal view of `run.json` (only the fields snapshot needs).
#[derive(Debug, Deserialize)]
struct RunJsonView {
    #[serde(default)]
    context: Option<String>,
    #[serde(default)]
    namespace: Option<String>,
    #[serde(default)]
    started_at: Option<u64>,
    #[serde(default)]
    topology: Vec<NodeView>,
}

#[derive(Debug, Deserialize)]
struct NodeView {
    node_id: String,
    pod: String,
}

#[derive(Debug, Serialize)]
struct MetricsSnapshot {
    captured_at: u64,
    window: Window,
    step: u32,
    /// metric name -> per-pod series.
    series: std::collections::BTreeMap<String, Vec<PodSeries>>,
    outcome: Outcome,
}

#[derive(Debug, Serialize)]
struct Window {
    start: u64,
    end: u64,
}

#[derive(Debug, Serialize)]
struct PodSeries {
    pod: String,
    /// [unix_secs, value] pairs (value as parsed float).
    values: Vec<(f64, f64)>,
}

#[derive(Debug, Serialize, Default)]
struct Outcome {
    max_finalized: u64,
    max_justified: u64,
    max_head: u64,
    /// True if finalized was flat for a long stretch while head advanced.
    stalled: bool,
    /// Slot finalized was pinned at during the longest flat stretch.
    stall_slot: u64,
    /// Length of the longest flat-finalized stretch (in samples).
    stall_samples: u64,
    /// Seconds from window start to first sample with finalized >= 2000.
    time_to_2000: Option<u64>,
    /// Mean committee aggregation time (seconds) across all samples.
    agg_avg: Option<f64>,
}

fn snapshot(args: SnapshotArgs) -> Result<()> {
    let runs_root = args.output_dir.join("runs");
    let run_dir = resolve_run_dir(&runs_root, args.run_id.as_deref())?;
    let run_id = run_dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    println!("Snapshotting run {run_id} ({})", run_dir.display());

    // Load run.json if present (gives us pods, context, namespace, started_at).
    let rj: Option<RunJsonView> = fs::read_to_string(run_dir.join("run.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok());

    let context = args
        .context
        .clone()
        .or_else(|| rj.as_ref().and_then(|r| r.context.clone()))
        .or_else(|| Profile::load().context)
        .unwrap_or_else(|| "leannet".to_string());
    let namespace = rj
        .as_ref()
        .and_then(|r| r.namespace.clone())
        .unwrap_or_else(|| args.namespace.clone());

    // Resolve the pod list: run.json topology, else kubectl get pods.
    let pods: Vec<(String, String)> = match rj.as_ref() {
        Some(r) if !r.topology.is_empty() => r
            .topology
            .iter()
            .map(|n| (n.pod.clone(), n.node_id.clone()))
            .collect(),
        _ => discover_pods(&context, &namespace)?,
    };
    if pods.is_empty() {
        bail!("No pods found in namespace {namespace} (context {context})");
    }

    // 1. Full per-pod log dump.
    if !args.no_logs {
        for (pod, node_id) in &pods {
            let dest = run_dir.join(format!("{node_id}.log"));
            match dump_pod_log(&context, &namespace, pod) {
                Ok(text) => {
                    fs::write(&dest, text)
                        .with_context(|| format!("write {}", dest.display()))?;
                    println!("  logs: {pod} -> {}", dest.display());
                }
                Err(e) => eprintln!("  warning: log dump for {pod} failed: {e:#}"),
            }
        }
    }

    // 2. Prometheus range snapshot.
    if !args.no_metrics {
        let start = rj
            .as_ref()
            .and_then(|r| r.started_at)
            .unwrap_or_else(|| now_secs().saturating_sub(3 * 3600));
        let end = now_secs();

        let mut pf: Option<PortForward> = None;
        let base = match &args.prometheus {
            Some(url) => url.trim_end_matches('/').to_string(),
            None => {
                pf = Some(PortForward::start(
                    &context,
                    &args.prometheus_namespace,
                    &args.prometheus_svc,
                    9090,
                    9090,
                )?);
                "http://localhost:9090".to_string()
            }
        };

        match snapshot_metrics(&base, start, end, args.step) {
            Ok(snap) => {
                if snap.series.values().all(|v| v.is_empty()) {
                    eprintln!(
                        "  warning: Prometheus returned 0 series. Is the ServiceMonitor \
                         applied? (it is removed on each redeploy — re-apply it, or deploy \
                         without --skip-metrics)"
                    );
                }
                let dest = run_dir.join("metrics.json");
                fs::write(&dest, serde_json::to_string_pretty(&snap)?)?;
                println!(
                    "  metrics: {} -> finalized max {}, {}",
                    dest.display(),
                    snap.outcome.max_finalized,
                    if snap.outcome.stalled {
                        format!("STALLED at {} for {} samples", snap.outcome.stall_slot, snap.outcome.stall_samples)
                    } else {
                        "no long stall".to_string()
                    }
                );
            }
            Err(e) => eprintln!("  warning: metrics snapshot failed: {e:#}"),
        }
        drop(pf); // stops the port-forward.
    }

    println!("Snapshot complete. Push with: leanstart runs push {run_id}");
    Ok(())
}

/// `kubectl logs <pod> --tail=-1` (all containers), stripped of nothing — raw.
fn dump_pod_log(context: &str, namespace: &str, pod: &str) -> Result<String> {
    let out = Command::new("kubectl")
        .args([
            "--context", context, "-n", namespace, "logs", pod, "--tail=-1",
            "--all-containers=true",
        ])
        .output()?;
    if !out.status.success() {
        bail!("{}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// List pods in the namespace as (pod_name, node_id) where node_id reverses the
/// `ream-0-0` -> `ream_0` convention (drop the trailing `-0` replica suffix,
/// swap `-` for `_`).
fn discover_pods(context: &str, namespace: &str) -> Result<Vec<(String, String)>> {
    let out = Command::new("kubectl")
        .args([
            "--context", context, "-n", namespace, "get", "pods", "-o",
            "jsonpath={.items[*].metadata.name}",
        ])
        .output()?;
    if !out.status.success() {
        bail!("{}", String::from_utf8_lossy(&out.stderr).trim());
    }
    let names = String::from_utf8_lossy(&out.stdout);
    Ok(names
        .split_whitespace()
        .map(|pod| {
            // ream-0-0 -> ream_0  (strip the final "-<n>" replica index)
            let node_id = match pod.rsplit_once('-') {
                Some((head, _idx)) => head.replace('-', "_"),
                None => pod.replace('-', "_"),
            };
            (pod.to_string(), node_id)
        })
        .collect())
}

/// Query Prometheus query_range for each metric and assemble the snapshot.
fn snapshot_metrics(base: &str, start: u64, end: u64, step: u32) -> Result<MetricsSnapshot> {
    let mut series = std::collections::BTreeMap::new();
    for metric in METRICS {
        let pod_series = query_range(base, metric, start, end, step).unwrap_or_default();
        series.insert(metric.to_string(), pod_series);
    }
    let outcome = compute_outcome(&series, start);
    Ok(MetricsSnapshot {
        captured_at: now_secs(),
        window: Window { start, end },
        step,
        series,
        outcome,
    })
}

/// One `query_range` call via curl, parsed into per-pod series.
fn query_range(
    base: &str,
    metric: &str,
    start: u64,
    end: u64,
    step: u32,
) -> Result<Vec<PodSeries>> {
    let url = format!(
        "{base}/api/v1/query_range?query={metric}&start={start}&end={end}&step={step}"
    );
    let out = Command::new("curl")
        .args(["-s", "--max-time", "30", &url])
        .output()?;
    if !out.status.success() {
        bail!("curl failed for {metric}");
    }
    let json: serde_json::Value = serde_json::from_slice(&out.stdout)
        .with_context(|| format!("parse Prometheus response for {metric}"))?;
    let results = json
        .get("data")
        .and_then(|d| d.get("result"))
        .and_then(|r| r.as_array())
        .cloned()
        .unwrap_or_default();

    let mut out_series = Vec::new();
    for r in results {
        let pod = r
            .get("metric")
            .and_then(|m| m.get("pod"))
            .and_then(|p| p.as_str())
            .unwrap_or("?")
            .to_string();
        let values = r
            .get("values")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|pair| {
                        let p = pair.as_array()?;
                        let t = p.first()?.as_f64()?;
                        let v: f64 = p.get(1)?.as_str()?.parse().ok()?;
                        Some((t, v))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        out_series.push(PodSeries { pod, values });
    }
    Ok(out_series)
}

/// Derive the outcome summary from the assembled series (uses the max across
/// pods per timestamp for the slot metrics — all pods converge).
fn compute_outcome(
    series: &std::collections::BTreeMap<String, Vec<PodSeries>>,
    start: u64,
) -> Outcome {
    let mut o = Outcome::default();

    // Collapse a metric to a single (t, max-over-pods) timeline sorted by t.
    let timeline = |name: &str| -> Vec<(f64, f64)> {
        let mut by_t: std::collections::BTreeMap<u64, f64> = std::collections::BTreeMap::new();
        if let Some(ps) = series.get(name) {
            for s in ps {
                for (t, v) in &s.values {
                    let e = by_t.entry(*t as u64).or_insert(f64::MIN);
                    if *v > *e {
                        *e = *v;
                    }
                }
            }
        }
        by_t.into_iter().map(|(t, v)| (t as f64, v)).collect()
    };

    let fin = timeline("lean_finalized_slot");
    let just = timeline("lean_justified_slot");
    let head = timeline("lean_head_slot");

    o.max_finalized = fin.iter().map(|(_, v)| *v as u64).max().unwrap_or(0);
    o.max_justified = just.iter().map(|(_, v)| *v as u64).max().unwrap_or(0);
    o.max_head = head.iter().map(|(_, v)| *v as u64).max().unwrap_or(0);

    // time_to_2000: first finalized sample >= 2000.
    o.time_to_2000 = fin
        .iter()
        .find(|(_, v)| *v as u64 >= 2000)
        .map(|(t, _)| (*t as u64).saturating_sub(start));

    // Longest flat-finalized stretch (the "non-finality plateau").
    let mut best_run = 0u64;
    let mut best_val = 0u64;
    let mut cur_run = 0u64;
    let mut cur_val = u64::MAX;
    for (_, v) in &fin {
        let v = *v as u64;
        if v == cur_val {
            cur_run += 1;
        } else {
            cur_val = v;
            cur_run = 1;
        }
        if cur_run > best_run {
            best_run = cur_run;
            best_val = cur_val;
        }
    }
    // "Stalled" heuristic: finalized flat for >=10 consecutive samples (~5min at
    // step 30) while it never reached the head — a real plateau, not the end.
    o.stall_samples = best_run;
    o.stall_slot = best_val;
    o.stalled = best_run >= 10 && best_val < o.max_finalized;

    // Mean aggregation time = Δsum / Δcount over the window. _sum and _count are
    // monotonic histogram counters; per pod take (last - first) and total across
    // pods so the average is call-weighted.
    let counter_delta = |name: &str| -> f64 {
        series
            .get(name)
            .map(|ps| {
                ps.iter()
                    .map(|s| {
                        let first = s.values.first().map(|(_, v)| *v).unwrap_or(0.0);
                        let last = s.values.last().map(|(_, v)| *v).unwrap_or(0.0);
                        (last - first).max(0.0)
                    })
                    .sum()
            })
            .unwrap_or(0.0)
    };
    let d_sum = counter_delta("lean_committee_signatures_aggregation_time_seconds_sum");
    let d_count = counter_delta("lean_committee_signatures_aggregation_time_seconds_count");
    if d_count > 0.0 {
        o.agg_avg = Some(d_sum / d_count);
    }

    o
}

/// A backgrounded `kubectl port-forward` that is killed on drop.
struct PortForward {
    child: Child,
}

impl PortForward {
    fn start(
        context: &str,
        namespace: &str,
        svc: &str,
        local: u16,
        remote: u16,
    ) -> Result<Self> {
        let child = Command::new("kubectl")
            .args([
                "--context", context, "-n", namespace, "port-forward", svc,
                &format!("{local}:{remote}"),
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .context("spawn kubectl port-forward")?;
        // Give it a moment to establish.
        thread::sleep(time::Duration::from_secs(4));
        Ok(Self { child })
    }
}

impl Drop for PortForward {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// ---------------------------------------------------------------------------
// push
// ---------------------------------------------------------------------------

fn push(args: PushArgs) -> Result<()> {
    let runs_root = args.output_dir.join("runs");
    let profile = Profile::load();
    let url = args
        .url
        .clone()
        .or(profile.showcase_url)
        .context("No showcase URL. Pass --url, set LEANSTART_SHOWCASE_URL, or add showcase_url to ~/.leanstart/config.yaml")?;
    let token = args
        .token
        .clone()
        .or(profile.showcase_token)
        .context("No ingest token. Pass --token, set LEANSTART_SHOWCASE_TOKEN, or add showcase_token to ~/.leanstart/config.yaml")?;
    let ingest = format!("{}/api/ingest", url.trim_end_matches('/'));

    let run_dirs: Vec<PathBuf> = if args.all {
        let mut v = Vec::new();
        for entry in fs::read_dir(&runs_root).context("read runs dir")? {
            let p = entry?.path();
            // Skip the `latest` symlink and non-dirs.
            if p.is_dir() && !p.is_symlink() {
                v.push(p);
            }
        }
        v.sort();
        v
    } else {
        vec![resolve_run_dir(&runs_root, args.run_id.as_deref())?]
    };

    let mut ok = 0;
    for dir in &run_dirs {
        match push_one(dir, &ingest, &token) {
            Ok(id) => {
                println!("  pushed {id}");
                ok += 1;
            }
            Err(e) => eprintln!("  skip {}: {e:#}", dir.display()),
        }
    }
    println!("Pushed {ok}/{} run(s) to {ingest}", run_dirs.len());
    Ok(())
}

/// Bundle one run dir into a gzipped tar and POST it to the ingest endpoint.
/// Uses `tar`/`curl` to avoid pulling in archive/http crates.
fn push_one(run_dir: &Path, ingest: &str, token: &str) -> Result<String> {
    let id = run_dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    if !run_dir.join("run.json").exists() {
        bail!("no run.json (run `leanstart runs snapshot` / re-run with a newer leanstart)");
    }
    // tar czf - -C <parent> <id>  | curl --data-binary @-
    let parent = run_dir.parent().context("run dir has no parent")?;
    let tar = Command::new("tar")
        .args(["czf", "-", "-C"])
        .arg(parent)
        .arg(&id)
        .output()
        .context("tar the run dir")?;
    if !tar.status.success() {
        bail!("tar failed: {}", String::from_utf8_lossy(&tar.stderr).trim());
    }

    // POST the tarball via curl reading the body from stdin.
    use std::io::Write;
    let mut child = Command::new("curl")
        .args([
            "-sS", "--fail", "-X", "POST", ingest, "-H",
            &format!("Authorization: Bearer {token}"), "-H",
            "Content-Type: application/gzip", "-H", &format!("X-Run-Id: {id}"),
            "--data-binary", "@-",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("spawn curl")?;
    child
        .stdin
        .as_mut()
        .context("curl stdin")?
        .write_all(&tar.stdout)?;
    let out = child.wait_with_output()?;
    if !out.status.success() {
        bail!("ingest POST failed: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(id)
}

// ---------------------------------------------------------------------------
// shared helpers
// ---------------------------------------------------------------------------

/// Resolve a run dir from an optional id, defaulting to the newest run.
fn resolve_run_dir(runs_root: &Path, run_id: Option<&str>) -> Result<PathBuf> {
    if let Some(id) = run_id {
        let p = runs_root.join(id);
        if !p.is_dir() {
            bail!("No such run: {}", p.display());
        }
        return Ok(p);
    }
    // Newest timestamped dir (ignore the `latest` symlink itself).
    let mut newest: Option<(String, PathBuf)> = None;
    for entry in fs::read_dir(runs_root)
        .with_context(|| format!("read {}", runs_root.display()))?
    {
        let p = entry?.path();
        if !p.is_dir() {
            continue;
        }
        let name = p
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        if name == "latest" {
            continue;
        }
        if newest.as_ref().map(|(n, _)| &name > n).unwrap_or(true) {
            newest = Some((name, p));
        }
    }
    newest
        .map(|(_, p)| p)
        .context("No runs found under output/runs")
}

fn now_secs() -> u64 {
    time::SystemTime::now()
        .duration_since(time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
