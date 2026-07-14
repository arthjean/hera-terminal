use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::FixtureLoadError;

pub const M6_HOST_EVIDENCE_MANIFEST_SCHEMA: &str = "hera.m6_host_evidence_manifest";
pub const M6_HOST_METRICS_SCHEMA: &str = "hera.m6_host_metrics";
pub const M6_HOST_EVIDENCE_VERSION: u32 = 1;
pub const M6_MAX_HOST_EVIDENCE_BYTES: u64 = 256 * 1024;
pub const M6_MAX_HOST_EVIDENCE_ARTIFACTS: usize = 256;
const M6_HOST_SOURCE_COMMAND: &str = "paneflow --features hera-host";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M6HostEvidenceManifest {
    schema: String,
    version: u32,
    run_id: String,
    hera_commit: String,
    paneflow_commit: String,
    artifacts: Vec<M6HostEvidenceArtifact>,
}

impl M6HostEvidenceManifest {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref();
        let raw = read_capped(path)?;
        let public_value: serde_json::Value = serde_json::from_str(&raw)
            .map_err(|error| invalid_schema(path, "$", format!("invalid JSON: {error}")))?;
        validate_public_payload(path, "$", &public_value, PublicPayloadKind::Manifest)?;
        let manifest: Self = parse_json(path, &raw)?;
        manifest.validate(path)?;
        Ok(manifest)
    }

    pub fn artifacts(&self) -> &[M6HostEvidenceArtifact] {
        &self.artifacts
    }

    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    pub fn hera_commit(&self) -> &str {
        &self.hera_commit
    }

    pub fn paneflow_commit(&self) -> &str {
        &self.paneflow_commit
    }

    pub fn validate_artifact_files(
        &self,
        manifest_path: &Path,
        repo_root: &Path,
    ) -> Result<(), FixtureLoadError> {
        let canonical_root = fs::canonicalize(repo_root).map_err(|error| FixtureLoadError::Io {
            path: repo_root.to_path_buf(),
            message: error.to_string(),
        })?;
        let mut canonical_paths = HashSet::new();
        for (index, artifact) in self.artifacts.iter().enumerate() {
            let declared_path = repo_root.join(&artifact.path);
            let path = fs::canonicalize(&declared_path).map_err(|error| FixtureLoadError::Io {
                path: declared_path,
                message: error.to_string(),
            })?;
            if !path.starts_with(&canonical_root) {
                return Err(invalid_schema(
                    manifest_path,
                    format!("artifacts[{index}].path"),
                    "artifact path resolves outside the repository root",
                ));
            }
            if !canonical_paths.insert(path.clone()) {
                return Err(invalid_schema(
                    manifest_path,
                    format!("artifacts[{index}].path"),
                    "duplicate canonical artifact target",
                ));
            }
            let metadata = fs::metadata(&path).map_err(|error| FixtureLoadError::Io {
                path: path.clone(),
                message: error.to_string(),
            })?;
            if !metadata.is_file() {
                return Err(invalid_schema(
                    manifest_path,
                    format!("artifacts[{index}].path"),
                    "artifact target must be a regular file",
                ));
            }
            let raw = read_capped(&path)?;
            let public_value: serde_json::Value = serde_json::from_str(&raw).map_err(|error| {
                invalid_schema(
                    manifest_path,
                    format!("artifacts[{index}].path"),
                    format!("artifact JSON is invalid: {error}"),
                )
            })?;
            validate_public_payload(&path, "$", &public_value, PublicPayloadKind::HostMetrics)?;
            let digest = format!("{:x}", Sha256::digest(raw.as_bytes()));
            if digest != artifact.sha256 {
                return Err(invalid_schema(
                    manifest_path,
                    format!("artifacts[{index}].sha256"),
                    "artifact SHA-256 digest does not match the manifest",
                ));
            }
            let metrics: M6HostMetricsArtifact = parse_json(&path, &raw)?;
            metrics.validate(&path)?;
            if metrics.source_command != artifact.source_command {
                return Err(invalid_schema(
                    manifest_path,
                    format!("artifacts[{index}].source_command"),
                    "manifest source command does not match the artifact",
                ));
            }
            if metrics.run_id != self.run_id || metrics.run_id != artifact.run_id {
                return Err(invalid_schema(
                    manifest_path,
                    format!("artifacts[{index}].run_id"),
                    "manifest and artifact must belong to the same run",
                ));
            }
            if artifact.hera_commit != self.hera_commit
                || artifact.paneflow_commit != self.paneflow_commit
            {
                return Err(invalid_schema(
                    manifest_path,
                    format!("artifacts[{index}]"),
                    "artifact revision binding does not match the manifest",
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn load_rollup(
        &self,
        manifest_path: &Path,
        repo_root: &Path,
    ) -> Result<M6HostEvidenceRollup, FixtureLoadError> {
        self.validate_artifact_files(manifest_path, repo_root)?;
        let mut rollup = M6HostEvidenceRollup::default();
        for artifact in &self.artifacts {
            let path = repo_root.join(&artifact.path);
            let raw = read_capped(&path)?;
            let metrics: M6HostMetricsArtifact = parse_json(&path, &raw)?;
            rollup.pane_count = rollup.pane_count.saturating_add(1);
            rollup.duration_seconds = rollup
                .duration_seconds
                .max((metrics.runtime.duration_ms / 1_000.0).floor() as u64);
            rollup.p0_mismatches = rollup
                .p0_mismatches
                .saturating_add(metrics.comparison.p0_mismatches);
            rollup.unsupported_checkpoints = rollup
                .unsupported_checkpoints
                .saturating_add(metrics.comparison.unsupported_checkpoints);
            rollup.fallback_count = rollup.fallback_count.saturating_add(metrics.fallback.count);
            rollup.dropped_bytes = rollup
                .dropped_bytes
                .saturating_add(metrics.runtime.dropped_bytes);
            rollup.latency_samples = rollup
                .latency_samples
                .saturating_add(metrics.latency.samples as u64);
            if let Some(p95) = metrics.latency.p95 {
                rollup.latency_p95_ms = Some(
                    rollup
                        .latency_p95_ms
                        .map_or(p95, |current: f64| current.max(p95)),
                );
            }
            if let Some(peak) = metrics.memory.peak_bytes {
                rollup.hera_peak_bytes = Some(
                    rollup
                        .hera_peak_bytes
                        .map_or(peak, |current: u64| current.max(peak)),
                );
            }
        }
        Ok(rollup)
    }

    fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        if self.schema != M6_HOST_EVIDENCE_MANIFEST_SCHEMA {
            return Err(invalid_schema(
                path,
                "schema",
                format!(
                    "expected {M6_HOST_EVIDENCE_MANIFEST_SCHEMA}, got {}",
                    self.schema
                ),
            ));
        }
        if self.version != M6_HOST_EVIDENCE_VERSION {
            return Err(invalid_schema(
                path,
                "version",
                format!("expected {M6_HOST_EVIDENCE_VERSION}, got {}", self.version),
            ));
        }
        validate_scoped_uuid(path, "run_id", &self.run_id, "run-")?;
        validate_commit(path, "hera_commit", &self.hera_commit)?;
        validate_commit(path, "paneflow_commit", &self.paneflow_commit)?;
        if self.artifacts.is_empty() {
            return Err(invalid_schema(path, "artifacts", "artifact list is empty"));
        }
        if self.artifacts.len() > M6_MAX_HOST_EVIDENCE_ARTIFACTS {
            return Err(invalid_schema(
                path,
                "artifacts",
                format!(
                    "artifact count {} exceeds {M6_MAX_HOST_EVIDENCE_ARTIFACTS}",
                    self.artifacts.len()
                ),
            ));
        }

        let mut paths: HashSet<PathBuf> = HashSet::new();
        for (index, artifact) in self.artifacts.iter().enumerate() {
            artifact.validate(path, index)?;
            if !paths.insert(normalized_repo_path(&artifact.path)) {
                return Err(invalid_schema(
                    path,
                    format!("artifacts[{index}].path"),
                    format!("duplicate artifact path: {}", artifact.path),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M6HostEvidenceArtifact {
    path: String,
    source_command: String,
    run_id: String,
    hera_commit: String,
    paneflow_commit: String,
    sha256: String,
}

impl M6HostEvidenceArtifact {
    fn validate(&self, manifest_path: &Path, index: usize) -> Result<(), FixtureLoadError> {
        let prefix = format!("artifacts[{index}]");
        validate_repo_path(manifest_path, &format!("{prefix}.path"), &self.path)?;
        if self.source_command.trim().is_empty() {
            return Err(invalid_schema(
                manifest_path,
                format!("{prefix}.source_command"),
                "source command is required",
            ));
        }
        if self.source_command != M6_HOST_SOURCE_COMMAND {
            return Err(invalid_schema(
                manifest_path,
                format!("{prefix}.source_command"),
                format!("expected {M6_HOST_SOURCE_COMMAND}"),
            ));
        }
        if contains_private_path(&self.source_command) {
            return Err(invalid_schema(
                manifest_path,
                format!("{prefix}.source_command"),
                "source command contains an absolute private path",
            ));
        }
        validate_scoped_uuid(
            manifest_path,
            &format!("{prefix}.run_id"),
            &self.run_id,
            "run-",
        )?;
        validate_commit(
            manifest_path,
            &format!("{prefix}.hera_commit"),
            &self.hera_commit,
        )?;
        validate_commit(
            manifest_path,
            &format!("{prefix}.paneflow_commit"),
            &self.paneflow_commit,
        )?;
        if self.sha256.len() != 64 || !self.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(invalid_schema(
                manifest_path,
                format!("{prefix}.sha256"),
                "expected a 64-character hexadecimal SHA-256 digest",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct M6HostEvidenceRollup {
    pub pane_count: u32,
    pub duration_seconds: u64,
    pub p0_mismatches: u64,
    pub unsupported_checkpoints: u64,
    pub fallback_count: u64,
    pub dropped_bytes: u64,
    pub latency_samples: u64,
    pub latency_p95_ms: Option<f64>,
    pub hera_peak_bytes: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct M6HostMetricsArtifact {
    schema: String,
    version: u32,
    source_command: String,
    run_id: String,
    pane_pseudonym: String,
    selected_engine: String,
    authoritative_engine: String,
    fallback: M6HostFallbackMetrics,
    runtime: M6HostRuntimeMetrics,
    comparison: M6HostComparisonMetrics,
    latency: M6HostLatencyMetrics,
    memory: M6HostMemoryMetrics,
}

impl M6HostMetricsArtifact {
    fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        if self.schema != M6_HOST_METRICS_SCHEMA {
            return Err(invalid_schema(
                path,
                "schema",
                format!("expected {M6_HOST_METRICS_SCHEMA}, got {}", self.schema),
            ));
        }
        if self.version != M6_HOST_EVIDENCE_VERSION {
            return Err(invalid_schema(
                path,
                "version",
                format!("expected {M6_HOST_EVIDENCE_VERSION}, got {}", self.version),
            ));
        }
        if self.source_command.trim().is_empty() {
            return Err(invalid_schema(
                path,
                "source_command",
                "source command is required",
            ));
        }
        if self.source_command != M6_HOST_SOURCE_COMMAND {
            return Err(invalid_schema(
                path,
                "source_command",
                format!("expected {M6_HOST_SOURCE_COMMAND}"),
            ));
        }
        validate_scoped_uuid(path, "run_id", &self.run_id, "run-")?;
        validate_scoped_uuid(path, "pane_pseudonym", &self.pane_pseudonym, "pane-")?;
        if !matches!(
            self.selected_engine.as_str(),
            "alacritty" | "hera" | "unknown"
        ) {
            return Err(invalid_schema(
                path,
                "selected_engine",
                "expected alacritty, hera or unknown",
            ));
        }
        if !matches!(
            self.authoritative_engine.as_str(),
            "alacritty" | "hera_authoritative"
        ) {
            return Err(invalid_schema(
                path,
                "authoritative_engine",
                "expected alacritty or hera_authoritative",
            ));
        }
        self.fallback.validate(path, &self.authoritative_engine)?;
        self.runtime.validate(path)?;
        self.comparison.validate(path, &self.authoritative_engine)?;
        self.latency.validate(path, self.runtime.output_batches)?;
        self.memory.validate(path)?;
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct M6HostComparisonMetrics {
    checkpoints: u64,
    equal_checkpoints: u64,
    p0_mismatches: u64,
    unsupported_checkpoints: u64,
    #[serde(default)]
    pending_coherence: u64,
}

impl M6HostComparisonMetrics {
    fn validate(&self, path: &Path, authority: &str) -> Result<(), FixtureLoadError> {
        let expected = self
            .equal_checkpoints
            .saturating_add(self.p0_mismatches)
            .saturating_add(self.unsupported_checkpoints);
        if self.checkpoints != expected {
            return Err(invalid_schema(
                path,
                "comparison.checkpoints",
                format!("expected {expected}, got {}", self.checkpoints),
            ));
        }
        if authority == "hera_authoritative" && self.checkpoints == 0 {
            return Err(invalid_schema(
                path,
                "comparison.checkpoints",
                "Hera-authoritative evidence requires at least one comparison checkpoint",
            ));
        }
        if authority == "hera_authoritative" && self.pending_coherence > 0 {
            return Err(invalid_schema(
                path,
                "comparison.pending_coherence",
                "final Hera-authoritative evidence cannot retain pending checkpoints",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct M6HostFallbackMetrics {
    count: u64,
    reason_class: Option<String>,
    elapsed_ms: Option<f64>,
}

impl M6HostFallbackMetrics {
    fn validate(&self, path: &Path, authority: &str) -> Result<(), FixtureLoadError> {
        if self.count > 1 {
            return Err(invalid_schema(
                path,
                "fallback.count",
                "per-pane fallback count must be zero or one",
            ));
        }
        match self.count {
            0 if self.reason_class.is_some() || self.elapsed_ms.is_some() => Err(invalid_schema(
                path,
                "fallback",
                "zero fallback count cannot carry a reason or elapsed time",
            )),
            1 if self.reason_class.as_deref().is_none_or(str::is_empty) => Err(invalid_schema(
                path,
                "fallback.reason_class",
                "fallback reason class is required",
            )),
            1 if !valid_fallback_reason(self.reason_class.as_deref().unwrap_or_default()) => {
                Err(invalid_schema(
                    path,
                    "fallback.reason_class",
                    "unknown fallback reason class",
                ))
            }
            1 if authority != "alacritty" => Err(invalid_schema(
                path,
                "authoritative_engine",
                "a fallback event must end with Alacritty authority",
            )),
            1 => validate_finite_nonnegative(path, "fallback.elapsed_ms", self.elapsed_ms)
                .and_then(|elapsed| {
                    if elapsed <= 100.0 {
                        Ok(())
                    } else {
                        Err(invalid_schema(
                            path,
                            "fallback.elapsed_ms",
                            "fallback exceeded the 100 ms recovery budget",
                        ))
                    }
                }),
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct M6HostRuntimeMetrics {
    output_batches: u64,
    #[serde(default)]
    snapshot_publications: u64,
    dropped_bytes: u64,
    #[serde(default)]
    queue_high_water_bytes: u64,
    #[serde(default)]
    accepted_sequence: u64,
    #[serde(default)]
    applied_sequence: u64,
    #[serde(default)]
    incomplete_sequence: bool,
    resize_count: u64,
    duration_ms: f64,
    exit_class: String,
}

impl M6HostRuntimeMetrics {
    fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        let _ = self.dropped_bytes;
        let _ = self.resize_count;
        if self.queue_high_water_bytes > 1024 * 1024 {
            return Err(invalid_schema(
                path,
                "runtime.queue_high_water_bytes",
                "Hera ingest queue exceeded the 1 MiB pane budget",
            ));
        }
        if self.snapshot_publications > self.output_batches {
            return Err(invalid_schema(
                path,
                "runtime.snapshot_publications",
                "snapshot publications cannot exceed accepted output batches",
            ));
        }
        if self.applied_sequence > self.accepted_sequence
            || self.incomplete_sequence != (self.applied_sequence < self.accepted_sequence)
        {
            return Err(invalid_schema(
                path,
                "runtime.incomplete_sequence",
                "accepted/applied sequence accounting is inconsistent",
            ));
        }
        validate_finite_nonnegative(path, "runtime.duration_ms", Some(self.duration_ms))?;
        if !matches!(
            self.exit_class.as_str(),
            "success" | "failure" | "unknown" | "closed"
        ) {
            return Err(invalid_schema(
                path,
                "runtime.exit_class",
                "unknown exit class",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct M6HostLatencyMetrics {
    unit: String,
    samples: usize,
    p50: Option<f64>,
    p95: Option<f64>,
    p99: Option<f64>,
}

impl M6HostLatencyMetrics {
    fn validate(&self, path: &Path, output_batches: u64) -> Result<(), FixtureLoadError> {
        if self.unit != "milliseconds" {
            return Err(invalid_schema(
                path,
                "latency.unit",
                "latency unit must be milliseconds",
            ));
        }
        if self.samples as u64 > output_batches || self.samples > 4_096 {
            return Err(invalid_schema(
                path,
                "latency.samples",
                "latency samples exceed recorded or bounded output batches",
            ));
        }
        if self.samples < 100 {
            if self.p50.is_some() || self.p95.is_some() || self.p99.is_some() {
                return Err(invalid_schema(
                    path,
                    "latency",
                    "percentiles require at least 100 output batch samples",
                ));
            }
            return Ok(());
        }
        let p50 = validate_finite_nonnegative(path, "latency.p50", self.p50)?;
        let p95 = validate_finite_nonnegative(path, "latency.p95", self.p95)?;
        let p99 = validate_finite_nonnegative(path, "latency.p99", self.p99)?;
        if p50 > p95 || p95 > p99 {
            return Err(invalid_schema(
                path,
                "latency",
                "latency percentiles must be monotone",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct M6HostMemoryMetrics {
    source: String,
    baseline_bytes: Option<u64>,
    peak_bytes: Option<u64>,
    delta_bytes: Option<i64>,
}

impl M6HostMemoryMetrics {
    fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        if self.source.trim().is_empty() {
            return Err(invalid_schema(
                path,
                "memory.source",
                "process RSS source is required",
            ));
        }
        if !matches!(
            self.source.as_str(),
            "get_process_memory_info"
                | "proc_self_status_vmrss"
                | "getrusage_maxrss"
                | "unsupported_platform"
        ) {
            return Err(invalid_schema(
                path,
                "memory.source",
                "unknown process RSS source",
            ));
        }
        match (self.baseline_bytes, self.peak_bytes, self.delta_bytes) {
            (Some(baseline), Some(peak), Some(delta)) => {
                let expected = (i128::from(peak) - i128::from(baseline))
                    .clamp(i128::from(i64::MIN), i128::from(i64::MAX))
                    as i64;
                if peak < baseline {
                    return Err(invalid_schema(
                        path,
                        "memory.peak_bytes",
                        "peak RSS cannot be lower than baseline RSS",
                    ));
                }
                if delta != expected {
                    return Err(invalid_schema(
                        path,
                        "memory.delta_bytes",
                        format!("expected {expected}, got {delta}"),
                    ));
                }
                Ok(())
            }
            (None, None, None) => Ok(()),
            _ => Err(invalid_schema(
                path,
                "memory",
                "baseline, peak and delta RSS must be all measured or all absent",
            )),
        }
    }
}

fn parse_json<T: for<'de> Deserialize<'de>>(path: &Path, raw: &str) -> Result<T, FixtureLoadError> {
    serde_json::from_str(raw).map_err(|error| {
        let message = error.to_string();
        let field = missing_field(&message).unwrap_or_else(|| "$".to_owned());
        invalid_schema(path, field, message)
    })
}

fn missing_field(message: &str) -> Option<String> {
    let rest = message.strip_prefix("missing field `")?;
    rest.split_once('`').map(|(field, _)| field.to_owned())
}

fn read_capped(path: &Path) -> Result<String, FixtureLoadError> {
    let metadata = fs::metadata(path).map_err(|error| FixtureLoadError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    if metadata.len() > M6_MAX_HOST_EVIDENCE_BYTES {
        return Err(FixtureLoadError::Io {
            path: path.to_path_buf(),
            message: format!(
                "M6 host evidence is {} bytes, maximum is {M6_MAX_HOST_EVIDENCE_BYTES}",
                metadata.len()
            ),
        });
    }
    fs::read_to_string(path).map_err(|error| FixtureLoadError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}

#[derive(Clone, Copy)]
enum PublicPayloadKind {
    Manifest,
    HostMetrics,
}

fn validate_public_payload(
    path: &Path,
    field: &str,
    value: &serde_json::Value,
    kind: PublicPayloadKind,
) -> Result<(), FixtureLoadError> {
    match value {
        serde_json::Value::Object(fields) => {
            for (key, child) in fields {
                let child_field = format!("{field}.{key}");
                if forbidden_field(key, kind) {
                    return Err(invalid_schema(
                        path,
                        child_field,
                        "terminal content or private identity field is forbidden",
                    ));
                }
                validate_public_payload(path, &child_field, child, kind)?;
            }
        }
        serde_json::Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                validate_public_payload(path, &format!("{field}[{index}]"), child, kind)?;
            }
        }
        serde_json::Value::String(text)
            if contains_private_path(text) || contains_secret_pattern(text) =>
        {
            return Err(invalid_schema(
                path,
                field,
                "absolute private path or credential pattern is forbidden in public M6 evidence",
            ));
        }
        _ => {}
    }
    Ok(())
}

fn forbidden_field(key: &str, kind: PublicPayloadKind) -> bool {
    let key = key.to_ascii_lowercase();
    let common = matches!(
        key.as_str(),
        "raw_bytes"
            | "terminal_bytes"
            | "terminal_lines"
            | "terminal_line"
            | "transcript"
            | "cwd"
            | "username"
            | "hostname"
            | "token"
            | "access_token"
            | "command"
            | "commands"
    );
    common || (matches!(kind, PublicPayloadKind::HostMetrics) && key == "path")
}

fn contains_private_path(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    let contains_windows_absolute = value.as_bytes().windows(3).any(|bytes| {
        bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && matches!(bytes[2], b'\\' | b'/')
    });
    let contains_unix_absolute = value
        .split_whitespace()
        .any(|token| token.starts_with('/') && !token.starts_with("//"));
    contains_windows_absolute
        || contains_unix_absolute
        || lower.contains("c:\\users\\")
        || lower.contains("c:/users/")
        || lower.contains("c:\\dev\\")
        || lower.contains("c:/dev/")
        || lower.contains("/home/")
        || lower.contains("/users/")
        || lower.starts_with("\\\\")
}

fn contains_secret_pattern(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "authorization:",
        "bearer ",
        "api_key",
        "api-key",
        "password=",
        "password:",
        "secret=",
        "token=",
    ]
    .iter()
    .any(|pattern| lower.contains(pattern))
}

fn normalized_repo_path(value: &str) -> PathBuf {
    Path::new(value)
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value),
            Component::CurDir => None,
            _ => None,
        })
        .collect()
}

fn validate_repo_path(path: &Path, field: &str, value: &str) -> Result<(), FixtureLoadError> {
    let candidate = Path::new(value);
    let valid = !value.trim().is_empty()
        && !candidate.is_absolute()
        && !value.contains('\\')
        && candidate
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir));
    if valid {
        Ok(())
    } else {
        Err(invalid_schema(
            path,
            field,
            "artifact path must be a relative repository path without parent traversal",
        ))
    }
}

fn validate_scoped_uuid(
    path: &Path,
    field: &str,
    value: &str,
    prefix: &str,
) -> Result<(), FixtureLoadError> {
    let valid = value.strip_prefix(prefix).is_some_and(|value| {
        let bytes = value.as_bytes();
        bytes.len() == 32
            && bytes.iter().all(u8::is_ascii_hexdigit)
            && bytes[12] == b'4'
            && matches!(bytes[16].to_ascii_lowercase(), b'8' | b'9' | b'a' | b'b')
    });
    if valid {
        Ok(())
    } else {
        Err(invalid_schema(
            path,
            field,
            format!("expected {prefix}<UUIDv4 simple hex> pseudonym"),
        ))
    }
}

fn validate_commit(path: &Path, field: &str, value: &str) -> Result<(), FixtureLoadError> {
    let valid =
        (7..=40).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_hexdigit());
    if valid {
        Ok(())
    } else {
        Err(invalid_schema(
            path,
            field,
            "expected a 7 to 40 character hexadecimal commit id",
        ))
    }
}

fn valid_fallback_reason(value: &str) -> bool {
    matches!(
        value,
        "host_feature_unavailable"
            | "unknown_selector"
            | "hera_initialization_failed"
            | "hera_output_drain_unavailable"
            | "hera_output_drain_busy"
            | "hera_output_drain_poisoned"
            | "hera_output_dropped"
            | "hera_state_unavailable"
            | "hera_state_lock_poisoned"
            | "hera_state_disabled"
            | "hera_snapshot_unavailable"
            | "hera_snapshot_dimensions_mismatch"
            | "hera_snapshot_adaptation_failed"
            | "hera_ingest_incomplete_sequence"
            | "hera_ingest_sequence_gap"
            | "hera_ingest_worker_panicked"
            | "hera_ingest_worker_stopped"
            | "hera_ingest_finalizer_spawn_failed"
            | "hera_snapshot_publication_failed"
            | "hera_resize_failed"
            | "hera_resize_coherence_timeout"
            | "hera_interaction_unavailable"
            | "hera_pty_spawn_failed"
    )
}

fn validate_finite_nonnegative(
    path: &Path,
    field: &str,
    value: Option<f64>,
) -> Result<f64, FixtureLoadError> {
    match value {
        Some(value) if value.is_finite() && value >= 0.0 => Ok(value),
        _ => Err(invalid_schema(
            path,
            field,
            "a finite non-negative measurement is required",
        )),
    }
}

fn invalid_schema(
    path: &Path,
    field: impl Into<String>,
    message: impl Into<String>,
) -> FixtureLoadError {
    FixtureLoadError::InvalidSchema {
        path: path.to_path_buf(),
        field: field.into(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use sha2::Digest;

    use super::M6HostEvidenceManifest;

    fn temp_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("hera-m6-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("evidence/m6")).expect("M6 temp evidence directory");
        root
    }

    fn metrics_json() -> String {
        r#"{
          "schema":"hera.m6_host_metrics","version":1,
          "source_command":"paneflow --features hera-host",
          "run_id":"run-550e8400e29b41d4a716446655440000","pane_pseudonym":"pane-6ba7b8109dad41d180b400c04fd430c8",
          "selected_engine":"hera","authoritative_engine":"alacritty",
          "fallback":{"count":1,"reason_class":"hera_output_dropped","elapsed_ms":1.0},
          "runtime":{"output_batches":100,"dropped_bytes":4,"resize_count":1,"duration_ms":25.0,"exit_class":"success"},
          "comparison":{"checkpoints":100,"equal_checkpoints":99,"p0_mismatches":1,"unsupported_checkpoints":0},
          "latency":{"unit":"milliseconds","samples":100,"p50":0.1,"p95":0.2,"p99":0.3},
          "memory":{"source":"get_process_memory_info","baseline_bytes":1000,"peak_bytes":1200,"delta_bytes":200}
        }"#
        .to_owned()
    }

    fn manifest_json(paths: &[&str]) -> String {
        manifest_json_for(paths, &metrics_json())
    }

    fn manifest_json_for(paths: &[&str], metrics: &str) -> String {
        let sha256 = format!("{:x}", super::Sha256::digest(metrics.as_bytes()));
        let artifacts = paths
            .iter()
            .map(|path| {
                format!(r#"{{"path":"{path}","source_command":"paneflow --features hera-host","run_id":"run-550e8400e29b41d4a716446655440000","hera_commit":"66c9229a","paneflow_commit":"4129f8ac","sha256":"{sha256}"}}"#)
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            r#"{{"schema":"hera.m6_host_evidence_manifest","version":1,"run_id":"run-550e8400e29b41d4a716446655440000","hera_commit":"66c9229a","paneflow_commit":"4129f8ac","artifacts":[{artifacts}]}}"#
        )
    }

    #[test]
    fn manifest_and_host_metrics_validate_as_scrubbed_public_evidence() {
        let root = temp_root("host-valid");
        let metrics_path = root.join("evidence/m6/host.json");
        let manifest_path = root.join("evidence/m6/manifest.json");
        fs::write(&metrics_path, metrics_json()).expect("metrics fixture");
        fs::write(&manifest_path, manifest_json(&["evidence/m6/host.json"]))
            .expect("manifest fixture");

        let manifest =
            M6HostEvidenceManifest::from_path(&manifest_path).expect("manifest must validate");
        manifest
            .validate_artifact_files(&manifest_path, &root)
            .expect("host metrics must validate");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn manifest_rejects_unknown_schema_duplicate_path_and_missing_source_command() {
        let root = temp_root("host-manifest-invalid");
        let path = root.join("evidence/m6/manifest.json");

        fs::write(
            &path,
            manifest_json(&["evidence/m6/a.json", "evidence/m6/a.json"]),
        )
        .expect("duplicate manifest");
        let duplicate = M6HostEvidenceManifest::from_path(&path)
            .expect_err("duplicate paths must fail")
            .to_string();
        assert!(duplicate.contains("duplicate artifact path"));

        fs::write(
            &path,
            r#"{"schema":"hera.m6_unknown","version":1,"run_id":"run-550e8400e29b41d4a716446655440000","hera_commit":"66c9229a","paneflow_commit":"4129f8ac","artifacts":[]}"#,
        )
        .expect("unknown schema manifest");
        let schema = M6HostEvidenceManifest::from_path(&path)
            .expect_err("unknown schema must fail")
            .to_string();
        assert!(schema.contains("field schema"));

        fs::write(
            &path,
            r#"{"schema":"hera.m6_host_evidence_manifest","version":1,"run_id":"run-550e8400e29b41d4a716446655440000","hera_commit":"66c9229a","paneflow_commit":"4129f8ac","artifacts":[{"path":"evidence/m6/a.json"}]}"#,
        )
        .expect("missing source command manifest");
        let missing = M6HostEvidenceManifest::from_path(&path)
            .expect_err("missing source command must fail")
            .to_string();
        assert!(missing.contains("source_command"));

        let artifacts = (0..257)
            .map(|index| {
                format!(
                    r#"{{"path":"evidence/m6/{index}.json","source_command":"paneflow --features hera-host","run_id":"run-550e8400e29b41d4a716446655440000","hera_commit":"66c9229a","paneflow_commit":"4129f8ac","sha256":"356c3f1655c549b1e423514e7501d36e9657d8ccfab1d4a0cd3c643030913d81"}}"#
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        fs::write(
            &path,
            format!(
                r#"{{"schema":"hera.m6_host_evidence_manifest","version":1,"run_id":"run-550e8400e29b41d4a716446655440000","hera_commit":"66c9229a","paneflow_commit":"4129f8ac","artifacts":[{artifacts}]}}"#
            ),
        )
        .expect("oversized artifact list manifest");
        let count = M6HostEvidenceManifest::from_path(&path)
            .expect_err("artifact count must be bounded")
            .to_string();
        assert!(count.contains("artifact count"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn host_manifest_requires_regular_artifact_files() {
        let root = temp_root("host-regular-file");
        let directory = root.join("evidence/m6/not-a-file.json");
        fs::create_dir_all(&directory).expect("directory-shaped artifact");
        let manifest_path = root.join("evidence/m6/manifest.json");
        fs::write(
            &manifest_path,
            manifest_json(&["evidence/m6/not-a-file.json"]),
        )
        .expect("manifest fixture");

        let manifest =
            M6HostEvidenceManifest::from_path(&manifest_path).expect("manifest contract");
        let error = manifest
            .validate_artifact_files(&manifest_path, &root)
            .expect_err("directories must not be parsed as artifacts")
            .to_string();
        assert!(error.contains("regular file"), "{error}");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn host_metrics_reject_terminal_content_identity_tokens_and_private_paths() {
        let root = temp_root("host-private");
        let metrics_path = root.join("evidence/m6/host.json");
        let manifest_path = root.join("evidence/m6/manifest.json");
        fs::write(&manifest_path, manifest_json(&["evidence/m6/host.json"]))
            .expect("manifest fixture");
        let manifest =
            M6HostEvidenceManifest::from_path(&manifest_path).expect("manifest must validate");

        for (field, value) in [
            ("terminal_lines", r#"["secret prompt"]"#),
            ("cwd", r#""C:\\Users\\Arthur\\project""#),
            ("token", r#""secret""#),
            ("hostname", r#""private-host""#),
        ] {
            let injected = metrics_json().replace(
                r#""memory":{"#,
                &format!(r#""{field}":{value},"memory":{{"#),
            );
            fs::write(&metrics_path, injected).expect("private metrics fixture");
            let error = manifest
                .validate_artifact_files(&manifest_path, &root)
                .expect_err("private evidence field must fail")
                .to_string();
            assert!(error.contains(field), "{field}: {error}");
        }

        let semantic_id =
            metrics_json().replace("run-550e8400e29b41d4a716446655440000", "run-Arthur-PC");
        fs::write(&metrics_path, semantic_id).expect("semantic run id fixture");
        let semantic_id = fs::read_to_string(&metrics_path).expect("semantic metrics");
        fs::write(
            &manifest_path,
            manifest_json_for(&["evidence/m6/host.json"], &semantic_id),
        )
        .expect("semantic manifest fixture");
        let manifest =
            M6HostEvidenceManifest::from_path(&manifest_path).expect("manifest must validate");
        let error = manifest
            .validate_artifact_files(&manifest_path, &root)
            .expect_err("semantic run ids must fail")
            .to_string();
        assert!(error.contains("run_id"), "{error}");
        let _ = fs::remove_dir_all(root);
    }
}
