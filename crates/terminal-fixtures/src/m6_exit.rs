use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{FixtureLoadError, M6HostEvidenceManifest};

pub const M6_EXIT_EVIDENCE_SCHEMA: &str = "hera.m6_exit_evidence";
pub const M6_EXIT_EVIDENCE_VERSION: u32 = 1;
pub const M6_MAX_EXIT_EVIDENCE_BYTES: u64 = 512 * 1024;

const REQUIRED_WORKLOADS: [M6CanaryWorkload; 6] = [
    M6CanaryWorkload::PowerShellEchoUnicode,
    M6CanaryWorkload::RapidOutput,
    M6CanaryWorkload::Output100kLines,
    M6CanaryWorkload::AlternateScreen,
    M6CanaryWorkload::CodexCli,
    M6CanaryWorkload::ClaudeCode,
];

const REQUIRED_PLATFORM_COMMANDS: [M6PlatformCommandId; 7] = [
    M6PlatformCommandId::DefaultCheck,
    M6PlatformCommandId::DefaultClippy,
    M6PlatformCommandId::DefaultTest,
    M6PlatformCommandId::HeraHostCheck,
    M6PlatformCommandId::HeraHostClippy,
    M6PlatformCommandId::HeraHostTest,
    M6PlatformCommandId::HostGoldens,
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M6ExitEvidence {
    pub schema: String,
    pub version: u32,
    pub generated_at: String,
    pub source_command: String,
    pub hera_commit: String,
    pub paneflow_commit: String,
    pub default_path_status: M6EvidenceStatus,
    pub windows_canary: M6WindowsCanary,
    pub platforms: M6PlatformRows,
    pub interaction_coverage: Vec<M6InteractionCoverage>,
    pub decision: M6ExitDecision,
    pub blockers: Vec<M6Blocker>,
    pub artifacts: M6ExitArtifacts,
}

impl M6ExitEvidence {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref();
        let raw = read_capped(path)?;
        let public: serde_json::Value = serde_json::from_str(&raw)
            .map_err(|error| invalid(path, "$", format!("invalid JSON: {error}")))?;
        validate_public_payload(path, "$", &public)?;
        let mut deserializer = serde_json::Deserializer::from_str(&raw);
        let evidence: Self = serde_path_to_error::deserialize(&mut deserializer)
            .map_err(|error| invalid(path, error.path().to_string(), error.inner().to_string()))?;
        evidence.validate(path)?;
        Ok(evidence)
    }

    pub fn validate_artifact_files(
        &self,
        evidence_path: &Path,
        repo_root: &Path,
    ) -> Result<(), FixtureLoadError> {
        let root = fs::canonicalize(repo_root).map_err(|error| FixtureLoadError::Io {
            path: repo_root.to_path_buf(),
            message: error.to_string(),
        })?;
        let host_manifest_path = resolve_regular_artifact(
            evidence_path,
            &root,
            "artifacts.host_manifest",
            &self.artifacts.host_manifest,
        )?;
        let host_manifest = M6HostEvidenceManifest::from_path(&host_manifest_path)?;
        if host_manifest.hera_commit() != self.hera_commit
            || host_manifest.paneflow_commit() != self.paneflow_commit
        {
            return Err(invalid(
                evidence_path,
                "artifacts.host_manifest",
                "host manifest revisions do not match the exit evidence",
            ));
        }
        let rollup = host_manifest.load_rollup(&host_manifest_path, &root)?;
        self.windows_canary
            .validate_host_rollup(evidence_path, &rollup)?;

        for (field, value) in [
            ("artifacts.baseline", self.artifacts.baseline.as_str()),
            ("artifacts.report", self.artifacts.report.as_str()),
        ] {
            let path = resolve_regular_artifact(evidence_path, &root, field, value)?;
            let raw = fs::read_to_string(&path).map_err(|error| FixtureLoadError::Io {
                path: path.clone(),
                message: error.to_string(),
            })?;
            if contains_private_path(&raw) || contains_secret_pattern(&raw) {
                return Err(invalid(
                    evidence_path,
                    field,
                    "artifact contains a private path or credential pattern",
                ));
            }
        }
        Ok(())
    }

    fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        if self.schema != M6_EXIT_EVIDENCE_SCHEMA {
            return Err(invalid(
                path,
                "schema",
                format!("expected {M6_EXIT_EVIDENCE_SCHEMA}"),
            ));
        }
        if self.version != M6_EXIT_EVIDENCE_VERSION {
            return Err(invalid(
                path,
                "version",
                format!("expected {M6_EXIT_EVIDENCE_VERSION}"),
            ));
        }
        validate_timestamp(path, "generated_at", &self.generated_at)?;
        if self.source_command != "terminal-cli validate-m6-exit-evidence" {
            return Err(invalid(path, "source_command", "unexpected source command"));
        }
        validate_commit(path, "hera_commit", &self.hera_commit)?;
        validate_commit(path, "paneflow_commit", &self.paneflow_commit)?;
        self.windows_canary.validate(path)?;
        self.platforms.validate(path)?;
        validate_interactions(path, &self.interaction_coverage)?;
        validate_blockers(path, &self.blockers)?;
        self.artifacts.validate(path)?;

        let all_platforms_pass = self
            .platforms
            .rows()
            .into_iter()
            .all(|row| row.status == M6EvidenceStatus::Pass);
        let windows_pass = self.windows_canary.status == M6EvidenceStatus::Pass;
        match self.decision {
            M6ExitDecision::BroaderHeraCanary
                if self.default_path_status != M6EvidenceStatus::Pass
                    || !windows_pass
                    || !all_platforms_pass
                    || !self.blockers.is_empty() =>
            {
                Err(invalid(
                    path,
                    "decision",
                    "broader_hera_canary requires a clean default path, passing canary and all platform rows, with no blockers",
                ))
            }
            M6ExitDecision::TargetedHostHardening if !windows_pass => Err(invalid(
                path,
                "decision",
                "targeted_host_hardening requires a passing Windows canary",
            )),
            M6ExitDecision::ReplacementExperimentBlocked
                if windows_pass && self.default_path_status == M6EvidenceStatus::Pass =>
            {
                Err(invalid(
                    path,
                    "decision",
                    "a passing Windows canary with a healthy default path must select targeted_host_hardening when broader rollout is blocked",
                ))
            }
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M6WindowsCanary {
    pub status: M6EvidenceStatus,
    pub duration_seconds: u64,
    pub completed_60_minute_target: bool,
    pub concurrent_panes: u32,
    pub workloads: Vec<M6CanaryWorkload>,
    pub p0_mismatches: u64,
    pub unsupported_checkpoints: u64,
    pub fallback_count: u64,
    pub dropped_bytes: u64,
    pub crash_count: u64,
    pub blank_frame_count: u64,
    pub lost_input_count: u64,
    pub orphan_process_count: u64,
    pub latency_samples: u64,
    pub latency_p95_ms: Option<f64>,
    pub memory: M6CanaryMemory,
    pub blocked_reason: Option<String>,
}

impl M6WindowsCanary {
    fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        let expected = REQUIRED_WORKLOADS.into_iter().collect::<BTreeSet<_>>();
        let actual = self.workloads.iter().copied().collect::<BTreeSet<_>>();
        if actual != expected || actual.len() != self.workloads.len() {
            return Err(invalid(
                path,
                "windows_canary.workloads",
                "workloads must contain the exact M6 canary set once",
            ));
        }
        self.memory.validate(path)?;
        let p95 = self.latency_p95_ms.filter(|value| value.is_finite());
        let passing_measurements = self.completed_60_minute_target
            && self.duration_seconds >= 3_600
            && self.concurrent_panes >= 2
            && self.p0_mismatches == 0
            && self.unsupported_checkpoints == 0
            && self.fallback_count == 0
            && self.dropped_bytes == 0
            && self.crash_count == 0
            && self.blank_frame_count == 0
            && self.lost_input_count == 0
            && self.orphan_process_count == 0
            && self.latency_samples >= 100
            && p95.is_some_and(|value| value <= 2.0)
            && self
                .memory
                .ratio_percent
                .is_some_and(|value| value <= 120.0);
        match self.status {
            M6EvidenceStatus::Pass if !passing_measurements => Err(invalid(
                path,
                "windows_canary.status",
                "pass canary does not satisfy the duration, reliability, latency or memory thresholds",
            )),
            M6EvidenceStatus::Pass if self.blocked_reason.is_some() => Err(invalid(
                path,
                "windows_canary.blocked_reason",
                "pass canary cannot carry a blocked reason",
            )),
            M6EvidenceStatus::Failed | M6EvidenceStatus::Blocked
                if self.blocked_reason.as_deref().is_none_or(str::is_empty) =>
            {
                Err(invalid(
                    path,
                    "windows_canary.blocked_reason",
                    "failed or blocked canary must explain why",
                ))
            }
            _ => Ok(()),
        }
    }

    fn validate_host_rollup(
        &self,
        path: &Path,
        rollup: &crate::m6_host_evidence::M6HostEvidenceRollup,
    ) -> Result<(), FixtureLoadError> {
        let exact = [
            (
                "concurrent_panes",
                u64::from(self.concurrent_panes),
                u64::from(rollup.pane_count),
            ),
            (
                "duration_seconds",
                self.duration_seconds,
                rollup.duration_seconds,
            ),
            ("p0_mismatches", self.p0_mismatches, rollup.p0_mismatches),
            (
                "unsupported_checkpoints",
                self.unsupported_checkpoints,
                rollup.unsupported_checkpoints,
            ),
            ("fallback_count", self.fallback_count, rollup.fallback_count),
            ("dropped_bytes", self.dropped_bytes, rollup.dropped_bytes),
            (
                "latency_samples",
                self.latency_samples,
                rollup.latency_samples,
            ),
        ];
        for (field, declared, observed) in exact {
            if declared != observed {
                return Err(invalid(
                    path,
                    format!("windows_canary.{field}"),
                    format!("host artifacts roll up to {observed}, got {declared}"),
                ));
            }
        }
        if self.completed_60_minute_target != (rollup.duration_seconds >= 3_600) {
            return Err(invalid(
                path,
                "windows_canary.completed_60_minute_target",
                "completion flag does not match host artifact duration",
            ));
        }
        if !same_optional_f64(self.latency_p95_ms, rollup.latency_p95_ms) {
            return Err(invalid(
                path,
                "windows_canary.latency_p95_ms",
                "latency P95 does not match the conservative host artifact roll-up",
            ));
        }
        if self.memory.hera_peak_bytes != rollup.hera_peak_bytes {
            return Err(invalid(
                path,
                "windows_canary.memory.hera_peak_bytes",
                "Hera peak RSS does not match the host artifacts",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M6CanaryMemory {
    pub source: String,
    pub hera_peak_bytes: Option<u64>,
    pub control_peak_bytes: Option<u64>,
    pub ratio_percent: Option<f64>,
}

impl M6CanaryMemory {
    fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        validate_required(path, "windows_canary.memory.source", &self.source)?;
        match (
            self.hera_peak_bytes,
            self.control_peak_bytes,
            self.ratio_percent,
        ) {
            (Some(hera), Some(control), Some(ratio)) if hera > 0 && control > 0 => {
                let expected = hera as f64 / control as f64 * 100.0;
                if !ratio.is_finite() || (ratio - expected).abs() > 0.01 {
                    return Err(invalid(
                        path,
                        "windows_canary.memory.ratio_percent",
                        format!("expected {expected:.4}"),
                    ));
                }
            }
            (Some(hera), None, None) if hera > 0 => {}
            _ => {
                return Err(invalid(
                    path,
                    "windows_canary.memory",
                    "record Hera peak alone for a failed canary, or provide the full paired control measurement",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M6PlatformRows {
    pub windows: M6PlatformRow,
    pub linux: M6PlatformRow,
    pub macos: M6PlatformRow,
}

impl M6PlatformRows {
    fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        self.windows
            .validate(path, "platforms.windows", M6Platform::Windows)?;
        self.linux
            .validate(path, "platforms.linux", M6Platform::Linux)?;
        self.macos
            .validate(path, "platforms.macos", M6Platform::Macos)
    }

    fn rows(&self) -> [&M6PlatformRow; 3] {
        [&self.windows, &self.linux, &self.macos]
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M6PlatformRow {
    pub platform: M6Platform,
    pub status: M6EvidenceStatus,
    pub visual_status: M6EvidenceStatus,
    pub target_triple: String,
    pub toolchain: String,
    pub commands: Vec<M6PlatformCommand>,
    pub blocked_reason: Option<String>,
}

impl M6PlatformRow {
    fn validate(
        &self,
        path: &Path,
        field: &str,
        expected_platform: M6Platform,
    ) -> Result<(), FixtureLoadError> {
        if self.platform != expected_platform {
            return Err(invalid(
                path,
                format!("{field}.platform"),
                "platform row mismatch",
            ));
        }
        validate_required(path, format!("{field}.target_triple"), &self.target_triple)?;
        validate_required(path, format!("{field}.toolchain"), &self.toolchain)?;
        let expected = REQUIRED_PLATFORM_COMMANDS
            .into_iter()
            .collect::<BTreeSet<_>>();
        let actual = self
            .commands
            .iter()
            .map(|item| item.id)
            .collect::<BTreeSet<_>>();
        if actual != expected || actual.len() != self.commands.len() {
            return Err(invalid(
                path,
                format!("{field}.commands"),
                "platform row must contain each required command exactly once",
            ));
        }
        for (index, command) in self.commands.iter().enumerate() {
            command.validate(path, &format!("{field}.commands[{index}]"))?;
        }
        let failed = self
            .commands
            .iter()
            .any(|item| item.status == M6EvidenceStatus::Failed);
        let blocked = self
            .commands
            .iter()
            .any(|item| item.status == M6EvidenceStatus::Blocked);
        match self.status {
            M6EvidenceStatus::Pass
                if failed || blocked || self.visual_status != M6EvidenceStatus::Pass =>
            {
                Err(invalid(
                    path,
                    format!("{field}.status"),
                    "pass row requires every command and visual status to pass",
                ))
            }
            M6EvidenceStatus::Failed
                if !failed && self.visual_status != M6EvidenceStatus::Failed =>
            {
                Err(invalid(
                    path,
                    format!("{field}.status"),
                    "failed row must contain a failed command or failed visual result",
                ))
            }
            M6EvidenceStatus::Blocked
                if failed || (!blocked && self.visual_status != M6EvidenceStatus::Blocked) =>
            {
                Err(invalid(
                    path,
                    format!("{field}.status"),
                    "blocked row must contain blocked command or visual evidence and no failed command",
                ))
            }
            M6EvidenceStatus::Failed | M6EvidenceStatus::Blocked
                if self.blocked_reason.as_deref().is_none_or(str::is_empty) =>
            {
                Err(invalid(
                    path,
                    format!("{field}.blocked_reason"),
                    "non-pass row must explain why",
                ))
            }
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M6PlatformCommand {
    pub id: M6PlatformCommandId,
    pub command: String,
    pub status: M6EvidenceStatus,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<u64>,
    pub blocked_reason: Option<String>,
}

impl M6PlatformCommand {
    fn validate(&self, path: &Path, field: &str) -> Result<(), FixtureLoadError> {
        if self.command != self.id.command() {
            return Err(invalid(
                path,
                format!("{field}.command"),
                format!("expected {}", self.id.command()),
            ));
        }
        match self.status {
            M6EvidenceStatus::Pass if self.exit_code != Some(0) || self.duration_ms.is_none() => {
                Err(invalid(
                    path,
                    field,
                    "pass command requires exit code 0 and measured duration",
                ))
            }
            M6EvidenceStatus::Failed
                if self.exit_code.is_none_or(|code| code == 0) || self.duration_ms.is_none() =>
            {
                Err(invalid(
                    path,
                    field,
                    "failed command requires a non-zero exit code and measured duration",
                ))
            }
            M6EvidenceStatus::Blocked
                if self.exit_code.is_some()
                    || self.duration_ms.is_some()
                    || self.blocked_reason.as_deref().is_none_or(str::is_empty) =>
            {
                Err(invalid(
                    path,
                    field,
                    "blocked command requires only a blocked reason",
                ))
            }
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M6InteractionCoverage {
    pub interaction: String,
    pub status: M6EvidenceStatus,
    pub proof: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M6Blocker {
    pub id: String,
    pub owner: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M6ExitArtifacts {
    pub baseline: String,
    pub host_manifest: String,
    pub report: String,
}

impl M6ExitArtifacts {
    fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        validate_repo_path(path, "artifacts.baseline", &self.baseline)?;
        validate_repo_path(path, "artifacts.host_manifest", &self.host_manifest)?;
        validate_repo_path(path, "artifacts.report", &self.report)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M6CanaryWorkload {
    PowerShellEchoUnicode,
    RapidOutput,
    Output100kLines,
    AlternateScreen,
    CodexCli,
    ClaudeCode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M6EvidenceStatus {
    Pass,
    Failed,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M6ExitDecision {
    BroaderHeraCanary,
    TargetedHostHardening,
    ReplacementExperimentBlocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M6Platform {
    Windows,
    Linux,
    Macos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M6PlatformCommandId {
    DefaultCheck,
    DefaultClippy,
    DefaultTest,
    HeraHostCheck,
    HeraHostClippy,
    HeraHostTest,
    HostGoldens,
}

impl M6PlatformCommandId {
    fn command(self) -> &'static str {
        match self {
            Self::DefaultCheck => "cargo check --workspace",
            Self::DefaultClippy => "cargo clippy --workspace --all-targets -- -D warnings",
            Self::DefaultTest => "cargo test --workspace",
            Self::HeraHostCheck => "cargo check --workspace --features hera-host",
            Self::HeraHostClippy => {
                "cargo clippy --workspace --all-targets --features hera-host -- -D warnings"
            }
            Self::HeraHostTest => "cargo test --workspace --features hera-host",
            Self::HostGoldens => "cargo test -p paneflow-app --features hera-host terminal::",
        }
    }
}

fn validate_interactions(
    path: &Path,
    interactions: &[M6InteractionCoverage],
) -> Result<(), FixtureLoadError> {
    if interactions.is_empty() {
        return Err(invalid(
            path,
            "interaction_coverage",
            "interaction coverage is required",
        ));
    }
    let mut names = BTreeSet::new();
    for (index, item) in interactions.iter().enumerate() {
        validate_required(
            path,
            format!("interaction_coverage[{index}].interaction"),
            &item.interaction,
        )?;
        validate_required(
            path,
            format!("interaction_coverage[{index}].proof"),
            &item.proof,
        )?;
        if !names.insert(item.interaction.as_str()) {
            return Err(invalid(
                path,
                format!("interaction_coverage[{index}].interaction"),
                "interaction names must be unique",
            ));
        }
    }
    Ok(())
}

fn validate_blockers(path: &Path, blockers: &[M6Blocker]) -> Result<(), FixtureLoadError> {
    let mut ids = BTreeSet::new();
    for (index, blocker) in blockers.iter().enumerate() {
        validate_required(path, format!("blockers[{index}].id"), &blocker.id)?;
        validate_required(path, format!("blockers[{index}].owner"), &blocker.owner)?;
        validate_required(path, format!("blockers[{index}].reason"), &blocker.reason)?;
        if !ids.insert(blocker.id.as_str()) {
            return Err(invalid(
                path,
                format!("blockers[{index}].id"),
                "blocker ids must be unique",
            ));
        }
    }
    Ok(())
}

fn validate_public_payload(
    path: &Path,
    field: &str,
    value: &serde_json::Value,
) -> Result<(), FixtureLoadError> {
    match value {
        serde_json::Value::Object(fields) => {
            for (key, child) in fields {
                let key_lower = key.to_ascii_lowercase();
                if matches!(
                    key_lower.as_str(),
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
                ) {
                    return Err(invalid(
                        path,
                        format!("{field}.{key}"),
                        "terminal content or private identity field is forbidden",
                    ));
                }
                validate_public_payload(path, &format!("{field}.{key}"), child)?;
            }
        }
        serde_json::Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                validate_public_payload(path, &format!("{field}[{index}]"), child)?;
            }
        }
        serde_json::Value::String(text)
            if contains_private_path(text) || contains_secret_pattern(text) =>
        {
            return Err(invalid(
                path,
                field,
                "absolute private path or credential pattern is forbidden",
            ));
        }
        _ => {}
    }
    Ok(())
}

fn resolve_regular_artifact(
    evidence_path: &Path,
    root: &Path,
    field: &str,
    value: &str,
) -> Result<PathBuf, FixtureLoadError> {
    let candidate = root.join(value);
    let canonical = fs::canonicalize(&candidate).map_err(|error| FixtureLoadError::Io {
        path: candidate,
        message: error.to_string(),
    })?;
    if !canonical.starts_with(root) {
        return Err(invalid(
            evidence_path,
            field,
            "artifact resolves outside repository root",
        ));
    }
    if !canonical.is_file() {
        return Err(invalid(
            evidence_path,
            field,
            "artifact must be a regular file",
        ));
    }
    Ok(canonical)
}

fn validate_repo_path(
    evidence_path: &Path,
    field: &str,
    value: &str,
) -> Result<(), FixtureLoadError> {
    let candidate = Path::new(value);
    let valid = !value.trim().is_empty()
        && !candidate.is_absolute()
        && !value.contains('\\')
        && candidate
            .components()
            .all(|component| matches!(component, Component::Normal(_)));
    if valid {
        Ok(())
    } else {
        Err(invalid(
            evidence_path,
            field,
            "artifact path must be a relative slash-separated repository path",
        ))
    }
}

fn validate_required(
    path: &Path,
    field: impl Into<String>,
    value: &str,
) -> Result<(), FixtureLoadError> {
    if value.trim().is_empty() {
        Err(invalid(path, field, "value is required"))
    } else {
        Ok(())
    }
}

fn validate_timestamp(path: &Path, field: &str, value: &str) -> Result<(), FixtureLoadError> {
    validate_required(path, field, value)?;
    if !value.contains('T') || !value.ends_with('Z') {
        return Err(invalid(
            path,
            field,
            "timestamp must be UTC ISO-8601 ending in Z",
        ));
    }
    Ok(())
}

fn validate_commit(path: &Path, field: &str, value: &str) -> Result<(), FixtureLoadError> {
    let valid =
        (7..=40).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_hexdigit());
    if valid {
        Ok(())
    } else {
        Err(invalid(
            path,
            field,
            "expected a 7 to 40 character hexadecimal commit id",
        ))
    }
}

fn contains_private_path(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    value.as_bytes().windows(3).any(|bytes| {
        bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && matches!(bytes[2], b'\\' | b'/')
    }) || lower.contains("c:\\users\\")
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

fn same_optional_f64(left: Option<f64>, right: Option<f64>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => (left - right).abs() <= 0.000_001,
        (None, None) => true,
        _ => false,
    }
}

fn read_capped(path: &Path) -> Result<String, FixtureLoadError> {
    let metadata = fs::metadata(path).map_err(|error| FixtureLoadError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    if !metadata.is_file() || metadata.len() > M6_MAX_EXIT_EVIDENCE_BYTES {
        return Err(invalid(
            path,
            "$",
            format!(
                "M6 exit evidence must be a regular file no larger than {M6_MAX_EXIT_EVIDENCE_BYTES} bytes"
            ),
        ));
    }
    fs::read_to_string(path).map_err(|error| FixtureLoadError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}

fn invalid(path: &Path, field: impl Into<String>, message: impl Into<String>) -> FixtureLoadError {
    FixtureLoadError::InvalidSchema {
        path: path.to_path_buf(),
        field: field.into(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::M6ExitEvidence;
    use crate::m6_host_evidence::M6HostEvidenceRollup;

    fn command(id: &str, command: &str, status: &str) -> String {
        if status == "blocked" {
            format!(
                r#"{{"id":"{id}","command":"{command}","status":"blocked","exit_code":null,"duration_ms":null,"blocked_reason":"runner unavailable"}}"#
            )
        } else {
            format!(
                r#"{{"id":"{id}","command":"{command}","status":"pass","exit_code":0,"duration_ms":1,"blocked_reason":null}}"#
            )
        }
    }

    fn commands(status: &str) -> String {
        [
            ("default_check", "cargo check --workspace"),
            (
                "default_clippy",
                "cargo clippy --workspace --all-targets -- -D warnings",
            ),
            ("default_test", "cargo test --workspace"),
            (
                "hera_host_check",
                "cargo check --workspace --features hera-host",
            ),
            (
                "hera_host_clippy",
                "cargo clippy --workspace --all-targets --features hera-host -- -D warnings",
            ),
            (
                "hera_host_test",
                "cargo test --workspace --features hera-host",
            ),
            (
                "host_goldens",
                "cargo test -p paneflow-app --features hera-host terminal::",
            ),
        ]
        .into_iter()
        .map(|(id, value)| command(id, value, status))
        .collect::<Vec<_>>()
        .join(",")
    }

    fn platform(platform: &str, status: &str) -> String {
        format!(
            r#"{{"platform":"{platform}","status":"{status}","visual_status":"{status}","target_triple":"x86_64-unknown-{platform}","toolchain":"rustc 1.96.1","commands":[{}],"blocked_reason":{}}}"#,
            commands(status),
            if status == "blocked" {
                r#""runner unavailable""#
            } else {
                "null"
            }
        )
    }

    fn valid_json() -> String {
        format!(
            r#"{{
              "schema":"hera.m6_exit_evidence","version":1,
              "generated_at":"2026-07-10T00:00:00Z",
              "source_command":"terminal-cli validate-m6-exit-evidence",
              "hera_commit":"66c9229","paneflow_commit":"4129f8ac",
              "default_path_status":"pass",
              "windows_canary":{{
                "status":"pass","duration_seconds":3600,"completed_60_minute_target":true,
                "concurrent_panes":2,
                "workloads":["power_shell_echo_unicode","rapid_output","output100k_lines","alternate_screen","codex_cli","claude_code"],
                "p0_mismatches":0,"unsupported_checkpoints":0,"fallback_count":0,"dropped_bytes":0,"crash_count":0,
                "blank_frame_count":0,"lost_input_count":0,"orphan_process_count":0,
                "latency_samples":100,"latency_p95_ms":0.5,
                "memory":{{"source":"paired_control","hera_peak_bytes":1200,"control_peak_bytes":1000,"ratio_percent":120.0}},
                "blocked_reason":null
              }},
              "platforms":{{"windows":{},"linux":{},"macos":{}}},
              "interaction_coverage":[{{"interaction":"visible_render","status":"pass","proof":"host golden corpus"}}],
              "decision":"targeted_host_hardening",
              "blockers":[{{"id":"platform-runners","owner":"maintainer","reason":"linux and macos runners unavailable"}}],
              "artifacts":{{"baseline":"evidence/m6/m6-baseline.json","host_manifest":"evidence/m6/host-manifest.json","report":"docs/m6-paneflow-controlled-host-replacement-report.md"}}
            }}"#,
            platform("windows", "pass"),
            platform("linux", "blocked"),
            platform("macos", "blocked")
        )
    }

    #[test]
    fn exit_policy_accepts_windows_pass_with_blocked_platform_hardening() {
        let raw = valid_json();
        let path = std::path::Path::new("m6-exit.json");
        let public: serde_json::Value = serde_json::from_str(&raw).expect("JSON fixture");
        super::validate_public_payload(path, "$", &public).expect("public evidence");
        let evidence: M6ExitEvidence = serde_json::from_str(&raw).expect("typed fixture");
        evidence.validate(path).expect("exit evidence");
    }

    #[test]
    fn broader_canary_cannot_hide_blocked_platforms() {
        let raw = valid_json().replace("targeted_host_hardening", "broader_hera_canary");
        let evidence: M6ExitEvidence = serde_json::from_str(&raw).expect("typed fixture");
        let error = evidence
            .validate(std::path::Path::new("m6-exit.json"))
            .expect_err("blocked platforms must prohibit broader rollout")
            .to_string();
        assert!(error.contains("broader_hera_canary"), "{error}");
    }

    #[test]
    fn pass_canary_cannot_be_relabelled_when_thresholds_fail() {
        let raw = valid_json().replace("\"duration_seconds\":3600", "\"duration_seconds\":3599");
        let evidence: M6ExitEvidence = serde_json::from_str(&raw).expect("typed fixture");
        let error = evidence
            .validate(std::path::Path::new("m6-exit.json"))
            .expect_err("short canary must fail")
            .to_string();
        assert!(error.contains("windows_canary.status"), "{error}");
    }

    #[test]
    fn exit_counters_cannot_hide_failed_host_artifacts() {
        let raw = valid_json();
        let evidence: M6ExitEvidence = serde_json::from_str(&raw).expect("typed fixture");
        let rollup = M6HostEvidenceRollup {
            pane_count: 2,
            duration_seconds: 3_600,
            p0_mismatches: 1,
            unsupported_checkpoints: 0,
            fallback_count: 0,
            dropped_bytes: 0,
            latency_samples: 100,
            latency_p95_ms: Some(0.5),
            hera_peak_bytes: Some(1_200),
        };
        let error = evidence
            .windows_canary
            .validate_host_rollup(std::path::Path::new("m6-exit.json"), &rollup)
            .expect_err("failed host counters must not validate as zero")
            .to_string();
        assert!(error.contains("p0_mismatches"), "{error}");
    }
}
