//! Fixture and replay test utility boundary for Hera M1.

#![forbid(unsafe_code)]

use std::fmt;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use terminal_core::{
    CellStyle, Color, CursorState, DamageRegion, Dimensions, ImagePlaceholder, ImageProtocol,
    M1_MAX_COLUMNS, M1_MAX_ROWS, PayloadStatus, RenderCell, RenderSnapshot, RowHandle,
    ScreenIdentity, ScrollbackConfig, ScrollbackRow, Terminal, TerminalAction, TerminalConfig,
    TerminalError, UnsupportedSequenceKind, ViewportRow,
};

mod m4_compatibility;
mod m4_evidence;
mod m4_performance;
mod m4_replay;
mod m5_baseline;
mod m5_compatibility;
mod m5_dogfood;
mod m5_evidence;
mod m5_go_no_go;
mod m5_platform;
mod m5_release;
mod m5_replay;
mod m5_security;

pub use m4_compatibility::{
    M4_COMPATIBILITY_MATRIX_SCHEMA, M4_COMPATIBILITY_MATRIX_VERSION,
    M4_MAX_COMPATIBILITY_MATRIX_BYTES, M4CompatibilityArtifact, M4CompatibilityArtifactKind,
    M4CompatibilityMatrix, M4CompatibilityRow, M4CompatibilityStatus, M4FixtureCoverage,
    M4FixtureCoverageStatus, M4PlatformMeasurementStatus, M4PlatformMeasurements,
    M4SourceReference, M4SourceReferenceKind,
};
pub use m4_evidence::{
    M4_EVIDENCE_MANIFEST_SCHEMA, M4_EVIDENCE_MANIFEST_VERSION, M4_MAX_EVIDENCE_MANIFEST_BYTES,
    M4_MAX_PUBLIC_ARTIFACT_BYTES, M4ArtifactPrivacy, M4EvidenceArtifact, M4EvidenceManifest,
    M4RedactionPolicy,
};
pub use m4_performance::{
    M4_BENCHMARK_EVIDENCE_SCHEMA, M4_MEMORY_PROFILE_SCHEMA, M4_PERFORMANCE_REPORT_SCHEMA,
    M4_PERFORMANCE_THRESHOLDS_SCHEMA, M4_PERFORMANCE_VERSION, M4BenchmarkEvidence,
    M4BenchmarkMeasurement, M4BenchmarkOperation, M4MachineMetadata, M4MemoryProfileEvidence,
    M4MemoryScenario, M4MetricEvaluation, M4MetricSource, M4MetricThreshold, M4PerformanceReport,
    M4PerformanceStatus, M4PerformanceThresholds, m4_rollup_status, m4_synthetic_workload,
};
pub use m4_replay::{
    M4_EVENT_STREAM_SCHEMA, M4_MAX_PUBLIC_REPLAY_EVENTS, M4_MAX_PUBLIC_REPLAY_FILE_BYTES,
    M4_MAX_PUBLIC_REPLAY_OUTPUT_BYTES, M4_PUBLIC_REPLAY_SCHEMA, M4_PUBLIC_REPLAY_VERSION,
    M4_REPLAY_VERIFICATION_SCHEMA, M4_REPLAY_VERIFICATION_VERSION, M4PublicReplayFixture,
    M4ReplayEvent, M4ReplayEventCounts, M4ReplayExportError, M4ReplayPrivacy, M4ReplayRedaction,
    M4ReplayRedactionStatus, M4ReplaySize, M4ReplaySource, M4ReplayVerificationError,
    M4ReplayVerificationReport, M4ReplayVerificationStatus, M4ReplayVerificationSummary,
};
pub use m5_baseline::{
    M5_BASELINE_SCHEMA, M5_BASELINE_VERSION, M5_MAX_BASELINE_BYTES, M5Baseline, M5BaselineBlocker,
    M5BaselineDependency, M5BaselineDependencyStatus, M5BaselineDisposition, M5BaselineStatus,
    M5CurrentEvidence, M5SourceMilestone,
};
pub use m5_compatibility::{
    M5_COMPATIBILITY_MATRIX_SCHEMA, M5_COMPATIBILITY_MATRIX_VERSION,
    M5_MAX_COMPATIBILITY_MATRIX_BYTES, M5CompatibilityArtifact, M5CompatibilityArtifactKind,
    M5CompatibilityMatrix, M5CompatibilityPriority, M5CompatibilityRow, M5DeferredPolicy,
    M5Disposition, M5FixtureCoverage, M5FixtureCoverageStatus, M5PlatformMeasurementStatus,
    M5PlatformMeasurements, M5SourceReference, M5SourceReferenceKind,
};
pub use m5_dogfood::{
    M5_MAX_DOGFOOD_REPORT_BYTES, M5_PANEFLOW_DOGFOOD_SCHEMA, M5_PANEFLOW_DOGFOOD_VERSION,
    M5DogfoodMismatch, M5DogfoodMismatchSeverity, M5DogfoodMismatchSummary, M5DogfoodMode,
    M5DogfoodRetention, M5DogfoodScenario, M5DogfoodStatus, M5PaneflowDogfoodReport,
    m5_default_paneflow_dogfood_report,
};
pub use m5_evidence::{
    M5_EVIDENCE_MANIFEST_SCHEMA, M5_EVIDENCE_MANIFEST_VERSION, M5_MAX_EVIDENCE_MANIFEST_BYTES,
    M5_MAX_PUBLIC_ARTIFACT_BYTES, M5ArtifactPlatform, M5ArtifactPrivacy, M5EvidenceArtifact,
    M5EvidenceManifest, M5RedactionPolicy, M5ReproducibilityStatus,
};
pub use m5_go_no_go::{
    M5_GO_NO_GO_SCHEMA, M5_GO_NO_GO_VERSION, M5_MAX_GO_NO_GO_BYTES, M5GoNoGoCriterion,
    M5GoNoGoPolicy, M5Outcome, M5OutcomePolicy, M5OutcomeStatus,
};
pub use m5_platform::{
    M5_MAX_PLATFORM_RUNTIME_EVIDENCE_BYTES, M5_PLATFORM_RUNTIME_EVIDENCE_SCHEMA,
    M5_PLATFORM_RUNTIME_EVIDENCE_VERSION, M5PlatformCommandId, M5PlatformCommandResult,
    M5PlatformCommandStatus, M5PlatformEvidenceStatus, M5PlatformFreshnessPolicy, M5PlatformRow,
    M5PlatformRows, M5PlatformRuntimeEvidence, M5RuntimePlatform, m5_required_platform_commands,
};
pub use m5_release::{
    M5_API_AUDIT_SCHEMA, M5_MAX_RELEASE_EVIDENCE_BYTES, M5_PACKAGE_READINESS_SCHEMA,
    M5_RELEASE_EVIDENCE_VERSION, M5_RELEASE_PLAN_SCHEMA, M5ActionStatus, M5ApiAudit,
    M5ApiBoundaryCheck, M5ApiFinding, M5ApiPublicCrate, M5AuditSource, M5CheckStatus,
    M5DocsRsPolicy, M5DryRunStatus, M5FindingSeverity, M5FindingStatus, M5MetadataOmission,
    M5PackageCrate, M5PackageDryRun, M5PackageReadiness, M5PublishAction, M5PublishIntent,
    M5ReadinessStatus, M5ReleasePlan, M5ReleasePlanCrate, M5SemverBaseline, M5ToolStatus,
};
pub use m5_replay::{
    M5_DEFAULT_MEMORY_BUDGET_BYTES, M5_DEFAULT_REPLAY_TIMEOUT_MS, M5_MAX_GENERATED_OUTPUT_LINES,
    M5_MAX_PUBLIC_REPLAY_EVENT_BYTES, M5_MAX_PUBLIC_REPLAY_EVENTS, M5_MAX_PUBLIC_REPLAY_FILE_BYTES,
    M5_MAX_PUBLIC_REPLAY_OUTPUT_BYTES, M5_PUBLIC_REPLAY_SCHEMA, M5_PUBLIC_REPLAY_VERSION,
    M5_REPLAY_VERIFICATION_SCHEMA, M5_REPLAY_VERIFICATION_VERSION, M5PublicReplayFixture,
    M5ReplayAgent, M5ReplayEvent, M5ReplayEventCounts, M5ReplayPolicy, M5ReplayPrivacy,
    M5ReplayRedaction, M5ReplayRedactionStatus, M5ReplayScrollback, M5ReplaySize, M5ReplaySource,
    M5ReplayVerificationError, M5ReplayVerificationReport, M5ReplayVerificationStatus,
    M5ReplayVerificationSummary, m5_default_replay_fixtures,
};
pub use m5_security::{
    M5_MAX_SECURITY_BASELINE_BYTES, M5_SECURITY_BASELINE_SCHEMA, M5_SECURITY_BASELINE_VERSION,
    M5SecurityBaseline, M5SecurityBaselineStatus, M5SecurityCheckStatus, M5SecurityCoverage,
    M5SecurityCoverageCategory, M5SecurityCoverageStatus, M5SecurityFinding,
    M5SecurityFindingSeverity, M5SecurityInstallStatus, M5SecuritySummary, M5SecurityToolCheck,
    M5SecurityToolId,
};

pub const M1_MAX_FIXTURE_FILE_BYTES: u64 = 2 * 1024 * 1024;
pub const M1_MAX_FIXTURES_PER_PACK: usize = 128;
pub const M1_MAX_CHUNKS_PER_FIXTURE: usize = 4096;
pub const M1_MAX_FIXTURE_INPUT_BYTES: usize = 1024 * 1024;
pub const M1_MAX_FIXTURE_VIEWPORT_CELLS: usize = 262_144;
pub const M1_MAX_SPLIT_BOUNDARY_BYTES: usize = 4096;
pub const M1_MAX_SNAPSHOT_BYTES: usize = 4 * 1024 * 1024;
pub const M1_MAX_SNAPSHOT_ROWS: usize = 100_000;
pub const M1_MAX_SNAPSHOT_CELLS: usize = 1_048_576;
pub const M1_MAX_SNAPSHOT_EVENTS: usize = 8192;
pub const M1_MAX_SNAPSHOT_STRING_BYTES: usize = 8192;
pub const M1_MAX_CHECKPOINTS_PER_FIXTURE: usize = 4096;
pub const M2_PTY_RECORDING_SCHEMA: &str = "hera.pty_recording";
pub const M2_PTY_RECORDING_VERSION: u32 = 1;
pub const M2_MAX_PTY_RECORDING_FILE_BYTES: u64 = 8 * 1024 * 1024;
pub const M2_MAX_PTY_RECORDING_EVENTS: usize = 8192;
pub const M2_MAX_PTY_RECORDING_OUTPUT_BYTES: usize = 4 * 1024 * 1024;
pub const M2_MAX_PTY_RECORDING_EVENT_BYTES: usize = 1024 * 1024;
pub const M3_DOGFOOD_RECORDING_SCHEMA: &str = "hera.m3_dogfood_recording";
pub const M3_DOGFOOD_RECORDING_VERSION: u32 = 1;
pub const M3_MAX_DOGFOOD_RECORDING_FILE_BYTES: u64 = 80 * 1024 * 1024;
pub const M3_MAX_DOGFOOD_RECORDING_EVENTS: usize = 65_536;
pub const M3_MAX_DOGFOOD_RECORDING_OUTPUT_BYTES: usize = 64 * 1024 * 1024;
pub const M3_DOGFOOD_METRICS_SCHEMA: &str = "hera.m3_dogfood_metrics";
pub const M3_DOGFOOD_METRICS_VERSION: u32 = 1;
pub const M3_MAX_DOGFOOD_METRICS_FILE_BYTES: u64 = 1024 * 1024;
pub const M3_LONG_SESSION_MIN_LOGICAL_LINES: usize = 10_000;
pub const M3_BATCH_P95_BUDGET_MS: f64 = 2.0;
pub const M3_10K_MEMORY_DELTA_BUDGET_BYTES: i64 = 64 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct FixturePack {
    fixtures: Vec<Fixture>,
}

impl FixturePack {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref();
        let raw = read_fixture_file_capped(path)?;

        if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("toml"))
        {
            Self::from_toml_str(path, &raw)
        } else {
            Self::from_json_str(path, &raw)
        }
    }

    pub fn from_json_str(path: impl AsRef<Path>, json: &str) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref().to_path_buf();
        if json.len() as u64 > M1_MAX_FIXTURE_FILE_BYTES {
            return Err(invalid_schema(
                &path,
                "$",
                format!(
                    "fixture JSON is {} bytes, maximum is {M1_MAX_FIXTURE_FILE_BYTES}",
                    json.len()
                ),
            ));
        }

        let mut deserializer = serde_json::Deserializer::from_str(json);
        let raw: RawFixturePack =
            serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
                FixtureLoadError::InvalidSchema {
                    path: path.clone(),
                    field: error.path().to_string(),
                    message: error.inner().to_string(),
                }
            })?;

        Self::from_raw_pack(path, raw)
    }

    pub fn from_toml_str(path: impl AsRef<Path>, toml: &str) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref().to_path_buf();
        if toml.len() as u64 > M1_MAX_FIXTURE_FILE_BYTES {
            return Err(invalid_schema(
                &path,
                "$",
                format!(
                    "fixture TOML is {} bytes, maximum is {M1_MAX_FIXTURE_FILE_BYTES}",
                    toml.len()
                ),
            ));
        }

        let value: toml::Value =
            toml::from_str(toml).map_err(|error| FixtureLoadError::InvalidSchema {
                path: path.clone(),
                field: "$".to_owned(),
                message: error.to_string(),
            })?;
        let raw: RawFixturePack = serde_path_to_error::deserialize(value).map_err(|error| {
            FixtureLoadError::InvalidSchema {
                path: path.clone(),
                field: error.path().to_string(),
                message: error.inner().to_string(),
            }
        })?;

        Self::from_raw_pack(path, raw)
    }

    fn from_raw_pack(path: PathBuf, raw: RawFixturePack) -> Result<Self, FixtureLoadError> {
        let raw_fixtures = raw
            .fixtures
            .ok_or_else(|| invalid_schema(&path, "fixtures", "missing fixture list"))?;

        if raw_fixtures.is_empty() {
            return Err(invalid_schema(&path, "fixtures", "fixture list is empty"));
        }
        if raw_fixtures.len() > M1_MAX_FIXTURES_PER_PACK {
            return Err(invalid_schema(
                &path,
                "fixtures",
                format!(
                    "fixture pack has {} fixtures, maximum is {M1_MAX_FIXTURES_PER_PACK}",
                    raw_fixtures.len()
                ),
            ));
        }

        let fixtures = raw_fixtures
            .into_iter()
            .enumerate()
            .map(|(index, fixture)| {
                Fixture::from_raw(&path, fixture, &format!("fixtures[{index}]"))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self { fixtures })
    }

    #[must_use]
    pub fn fixtures(&self) -> &[Fixture] {
        &self.fixtures
    }
}

#[derive(Debug, Clone)]
pub struct Fixture {
    name: String,
    terminal: TerminalSpec,
    chunks: Vec<InputChunk>,
    expected: ExpectedSnapshot,
    checkpoints: Vec<ExpectedCheckpoint>,
    split_every_boundary: bool,
    deterministic: bool,
}

impl Fixture {
    fn from_raw(path: &Path, raw: RawFixture, prefix: &str) -> Result<Self, FixtureLoadError> {
        let name = required(path, prefix, "name", raw.name)?;
        let terminal = raw.terminal.unwrap_or_default().validate(path, prefix)?;
        let raw_chunks = required(path, prefix, "chunks", raw.chunks)?;

        if raw_chunks.is_empty() {
            return Err(invalid_schema(
                path,
                format!("{prefix}.chunks"),
                "at least one input chunk is required",
            ));
        }
        if raw_chunks.len() > M1_MAX_CHUNKS_PER_FIXTURE {
            return Err(invalid_schema(
                path,
                format!("{prefix}.chunks"),
                format!(
                    "fixture has {} chunks, maximum is {M1_MAX_CHUNKS_PER_FIXTURE}",
                    raw_chunks.len()
                ),
            ));
        }

        let mut total_input_bytes = 0usize;
        let mut chunks = Vec::with_capacity(raw_chunks.len());
        for (index, raw_chunk) in raw_chunks.into_iter().enumerate() {
            let field = format!("{prefix}.chunks[{index}]");
            let expanded_len = raw_chunk.expanded_len(path, &field)?;
            let Some(next_total_input_bytes) = total_input_bytes.checked_add(expanded_len) else {
                return Err(invalid_schema(
                    path,
                    format!("{prefix}.chunks"),
                    "expanded input byte count overflowed",
                ));
            };
            if next_total_input_bytes > M1_MAX_FIXTURE_INPUT_BYTES {
                return Err(invalid_schema(
                    path,
                    format!("{prefix}.chunks"),
                    format!(
                        "expanded input is {next_total_input_bytes} bytes, maximum is {M1_MAX_FIXTURE_INPUT_BYTES}"
                    ),
                ));
            }

            let chunk = InputChunk::from_raw(path, raw_chunk, &field)?;
            total_input_bytes = next_total_input_bytes;
            chunks.push(chunk);
        }

        let expected = required(path, prefix, "expected", raw.expected)?;
        if !expected.has_assertion() {
            return Err(invalid_schema(
                path,
                format!("{prefix}.expected"),
                "expected block must contain at least one assertion",
            ));
        }

        let checkpoints = raw.checkpoints.unwrap_or_default();
        if checkpoints.len() > M1_MAX_CHECKPOINTS_PER_FIXTURE {
            return Err(invalid_schema(
                path,
                format!("{prefix}.checkpoints"),
                format!(
                    "fixture has {} checkpoints, maximum is {M1_MAX_CHECKPOINTS_PER_FIXTURE}",
                    checkpoints.len()
                ),
            ));
        }

        for (index, checkpoint) in checkpoints.iter().enumerate() {
            if checkpoint.after_chunk == 0 || checkpoint.after_chunk > chunks.len() {
                return Err(invalid_schema(
                    path,
                    format!("{prefix}.checkpoints[{index}].after_chunk"),
                    "checkpoint must point to a completed chunk",
                ));
            }
            if !checkpoint.expected.has_assertion() {
                return Err(invalid_schema(
                    path,
                    format!("{prefix}.checkpoints[{index}].expected"),
                    "expected block must contain at least one assertion",
                ));
            }
        }

        let split_every_boundary = raw.split_every_boundary.unwrap_or(false);
        if split_every_boundary && chunks.iter().any(InputChunk::is_resize) {
            return Err(invalid_schema(
                path,
                format!("{prefix}.split_every_boundary"),
                "split-boundary replay cannot include resize chunks",
            ));
        }
        if split_every_boundary && total_input_bytes > M1_MAX_SPLIT_BOUNDARY_BYTES {
            return Err(invalid_schema(
                path,
                format!("{prefix}.split_every_boundary"),
                format!(
                    "split-boundary replay input is {total_input_bytes} bytes, maximum is {M1_MAX_SPLIT_BOUNDARY_BYTES}"
                ),
            ));
        }

        Ok(Self {
            name,
            terminal,
            chunks,
            expected,
            checkpoints,
            split_every_boundary,
            deterministic: raw.deterministic.unwrap_or(true),
        })
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Clone, Copy)]
struct TerminalSpec {
    columns: usize,
    rows: usize,
    scrollback: ScrollbackSpec,
}

impl TerminalSpec {
    fn to_config(self) -> Result<TerminalConfig, TerminalError> {
        if self
            .columns
            .checked_mul(self.rows)
            .is_none_or(|cells| cells > M1_MAX_FIXTURE_VIEWPORT_CELLS)
        {
            return Err(TerminalError::InvalidDimensions {
                columns: self.columns,
                rows: self.rows,
                max_columns: M1_MAX_COLUMNS,
                max_rows: M1_MAX_ROWS,
                max_cells: M1_MAX_FIXTURE_VIEWPORT_CELLS,
            });
        }

        TerminalConfig::with_scrollback(
            self.columns,
            self.rows,
            ScrollbackConfig::new(self.scrollback.max_lines, self.scrollback.max_bytes),
        )
    }
}

impl Default for TerminalSpec {
    fn default() -> Self {
        Self {
            columns: 80,
            rows: 24,
            scrollback: ScrollbackSpec::default(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ScrollbackSpec {
    max_lines: usize,
    max_bytes: usize,
}

impl Default for ScrollbackSpec {
    fn default() -> Self {
        Self {
            max_lines: 10_000,
            max_bytes: 8 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InputChunk {
    bytes: Vec<u8>,
    resize: Option<ResizeOperation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResizeOperation {
    columns: usize,
    rows: usize,
}

impl InputChunk {
    fn from_raw(path: &Path, raw: RawInputChunk, field: &str) -> Result<Self, FixtureLoadError> {
        match (raw.bytes, raw.repeat, raw.resize) {
            (Some(bytes), None, None) => {
                if bytes.is_empty() {
                    return Err(invalid_schema(path, field, "chunk bytes must not be empty"));
                }
                if bytes.len() > M1_MAX_FIXTURE_INPUT_BYTES {
                    return Err(invalid_schema(
                        path,
                        field,
                        format!(
                            "chunk has {} bytes, maximum is {M1_MAX_FIXTURE_INPUT_BYTES}",
                            bytes.len()
                        ),
                    ));
                }
                Ok(Self {
                    bytes,
                    resize: None,
                })
            }
            (None, Some(repeat), None) => {
                if repeat.count == 0 {
                    return Err(invalid_schema(path, field, "repeat count must not be zero"));
                }
                if repeat.count > M1_MAX_FIXTURE_INPUT_BYTES {
                    return Err(invalid_schema(
                        path,
                        field,
                        format!(
                            "repeat expands to {} bytes, maximum is {M1_MAX_FIXTURE_INPUT_BYTES}",
                            repeat.count
                        ),
                    ));
                }
                Ok(Self {
                    bytes: vec![repeat.byte; repeat.count],
                    resize: None,
                })
            }
            (None, None, Some(resize)) => {
                Dimensions::new(resize.columns, resize.rows).map_err(|error| {
                    invalid_schema(path, field, format!("invalid resize operation: {error}"))
                })?;
                Ok(Self {
                    bytes: Vec::new(),
                    resize: Some(ResizeOperation {
                        columns: resize.columns,
                        rows: resize.rows,
                    }),
                })
            }
            _ => Err(invalid_schema(
                path,
                field,
                "chunk must contain exactly one of bytes, repeat or resize",
            )),
        }
    }

    fn is_resize(&self) -> bool {
        self.resize.is_some()
    }
}

#[derive(Debug, Clone)]
pub struct FixtureRunner;

impl FixtureRunner {
    pub fn run_pack_path(path: impl AsRef<Path>) -> Result<Vec<FixtureReport>, FixtureError> {
        let pack = FixturePack::from_path(path).map_err(FixtureError::Load)?;
        Self.run_pack(&pack).map_err(FixtureError::Run)
    }

    pub fn run_pack(&self, pack: &FixturePack) -> Result<Vec<FixtureReport>, FixtureRunError> {
        pack.fixtures
            .iter()
            .map(|fixture| self.run_fixture(fixture))
            .collect()
    }

    pub fn run_fixture(&self, fixture: &Fixture) -> Result<FixtureReport, FixtureRunError> {
        let replay = replay_fixture(fixture, &fixture.chunks, true)?;
        let snapshot_bytes = serialize_snapshot(&replay.snapshot)?;

        verify_expected(
            &fixture.name,
            "expected",
            &fixture.expected,
            &replay.snapshot,
        )?;

        if fixture.deterministic {
            let second = replay_fixture(fixture, &fixture.chunks, true)?;
            let second_bytes = serialize_snapshot(&second.snapshot)?;
            if snapshot_bytes != second_bytes {
                return Err(FixtureRunError::Assertion(FixtureAssertionFailure::new(
                    &fixture.name,
                    "deterministic_snapshot_bytes",
                    format!("{} bytes", snapshot_bytes.len()),
                    format!("{} bytes", second_bytes.len()),
                )));
            }
        }

        if fixture.split_every_boundary {
            verify_split_boundaries(fixture, &replay.snapshot)?;
        }

        Ok(FixtureReport {
            name: fixture.name.clone(),
            snapshot: replay.snapshot,
            snapshot_bytes,
        })
    }
}

#[derive(Debug, Clone)]
pub struct FixtureReport {
    name: String,
    snapshot: TerminalSnapshot,
    snapshot_bytes: Vec<u8>,
}

impl FixtureReport {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn snapshot(&self) -> &TerminalSnapshot {
        &self.snapshot
    }

    #[must_use]
    pub fn snapshot_bytes(&self) -> &[u8] {
        &self.snapshot_bytes
    }
}

pub fn snapshot_terminal(terminal: &mut Terminal) -> TerminalSnapshot {
    let actions = terminal.actions().to_vec();
    TerminalSnapshot::from_render(terminal.render_snapshot(), &actions)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalSnapshot {
    columns: usize,
    rows: usize,
    active_screen: SnapshotScreen,
    #[serde(default)]
    modes: SnapshotModes,
    cursor: SnapshotCursor,
    viewport_rows: Vec<SnapshotRow>,
    scrollback_rows: Vec<SnapshotRow>,
    damage: Vec<SnapshotDamage>,
    events: Vec<SnapshotEvent>,
    #[serde(default)]
    semantic_events: Vec<SemanticEvent>,
}

impl TerminalSnapshot {
    fn from_render(snapshot: RenderSnapshot, actions: &[TerminalAction]) -> Self {
        Self {
            columns: snapshot.columns(),
            rows: snapshot.rows(),
            active_screen: SnapshotScreen::from(snapshot.active_screen()),
            modes: snapshot_modes(actions),
            cursor: SnapshotCursor::from(snapshot.cursor()),
            viewport_rows: snapshot
                .viewport_rows()
                .iter()
                .map(SnapshotRow::from_viewport)
                .collect(),
            scrollback_rows: snapshot
                .scrollback_rows()
                .iter()
                .map(SnapshotRow::from_scrollback)
                .collect(),
            damage: snapshot.damage().iter().copied().map(Into::into).collect(),
            events: snapshot_events(actions),
            semantic_events: Vec::new(),
        }
    }

    #[must_use]
    pub const fn columns(&self) -> usize {
        self.columns
    }

    #[must_use]
    pub const fn rows(&self) -> usize {
        self.rows
    }

    #[must_use]
    pub const fn active_screen(&self) -> SnapshotScreen {
        self.active_screen
    }

    #[must_use]
    pub const fn modes(&self) -> SnapshotModes {
        self.modes
    }

    #[must_use]
    pub const fn cursor(&self) -> SnapshotCursor {
        self.cursor
    }

    #[must_use]
    pub fn viewport_rows(&self) -> &[SnapshotRow] {
        &self.viewport_rows
    }

    #[must_use]
    pub fn scrollback_rows(&self) -> &[SnapshotRow] {
        &self.scrollback_rows
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotScreen {
    Primary,
    Alternate,
}

impl From<ScreenIdentity> for SnapshotScreen {
    fn from(value: ScreenIdentity) -> Self {
        match value {
            ScreenIdentity::Primary => Self::Primary,
            ScreenIdentity::Alternate => Self::Alternate,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotModes {
    bracketed_paste: bool,
}

impl SnapshotModes {
    #[must_use]
    pub const fn new(bracketed_paste: bool) -> Self {
        Self { bracketed_paste }
    }

    #[must_use]
    pub const fn bracketed_paste(self) -> bool {
        self.bracketed_paste
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotCursor {
    row: usize,
    column: usize,
    visible: bool,
}

impl SnapshotCursor {
    #[must_use]
    pub const fn new(row: usize, column: usize, visible: bool) -> Self {
        Self {
            row,
            column,
            visible,
        }
    }

    #[must_use]
    pub const fn row(self) -> usize {
        self.row
    }

    #[must_use]
    pub const fn column(self) -> usize {
        self.column
    }

    #[must_use]
    pub const fn visible(self) -> bool {
        self.visible
    }
}

impl From<CursorState> for SnapshotCursor {
    fn from(value: CursorState) -> Self {
        Self::new(value.row(), value.column(), value.visible())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotRow {
    handle: SnapshotRowHandle,
    cells: Vec<SnapshotCell>,
    wrapped: bool,
}

impl SnapshotRow {
    fn from_viewport(row: &ViewportRow) -> Self {
        Self::from_parts(row.handle(), row.cells(), row.wrapped())
    }

    fn from_scrollback(row: &ScrollbackRow) -> Self {
        Self::from_parts(row.handle(), row.cells(), row.wrapped())
    }

    fn from_parts(handle: RowHandle, cells: &[RenderCell], wrapped: bool) -> Self {
        Self {
            handle: SnapshotRowHandle::from(handle),
            cells: cells.iter().map(SnapshotCell::from).collect(),
            wrapped,
        }
    }

    #[must_use]
    pub fn text(&self) -> String {
        self.cells.iter().map(|cell| cell.ch).collect()
    }

    #[must_use]
    pub const fn wrapped(&self) -> bool {
        self.wrapped
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotRowHandle {
    id: u64,
    generation: u64,
}

impl From<RowHandle> for SnapshotRowHandle {
    fn from(value: RowHandle) -> Self {
        Self {
            id: value.id(),
            generation: value.generation(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotCell {
    ch: char,
    width: u8,
    style: SnapshotStyle,
    image: Option<SnapshotImage>,
}

impl From<&RenderCell> for SnapshotCell {
    fn from(value: &RenderCell) -> Self {
        Self {
            ch: value.ch(),
            width: value.width(),
            style: SnapshotStyle::from(value.style()),
            image: value.image().map(SnapshotImage::from),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotStyle {
    foreground: Option<SnapshotColor>,
    background: Option<SnapshotColor>,
    bold: bool,
    italic: bool,
    underline: bool,
    inverse: bool,
}

impl From<CellStyle> for SnapshotStyle {
    fn from(value: CellStyle) -> Self {
        Self {
            foreground: value.foreground().map(Into::into),
            background: value.background().map(Into::into),
            bold: value.bold(),
            italic: value.italic(),
            underline: value.underline(),
            inverse: value.inverse(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotColor {
    Indexed(u8),
    Rgb { red: u8, green: u8, blue: u8 },
}

impl From<Color> for SnapshotColor {
    fn from(value: Color) -> Self {
        match value {
            Color::Indexed(index) => Self::Indexed(index),
            Color::Rgb { red, green, blue } => Self::Rgb { red, green, blue },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotImage {
    protocol: SnapshotImageProtocol,
    id: Option<String>,
    byte_len: usize,
    diagnostic: String,
}

impl From<&ImagePlaceholder> for SnapshotImage {
    fn from(value: &ImagePlaceholder) -> Self {
        Self {
            protocol: SnapshotImageProtocol::from(value.protocol()),
            id: value.id().map(ToOwned::to_owned),
            byte_len: value.byte_len(),
            diagnostic: value.diagnostic().to_owned(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotImageProtocol {
    Kitty,
    Iterm2,
    Sixel,
    Unknown,
}

impl From<ImageProtocol> for SnapshotImageProtocol {
    fn from(value: ImageProtocol) -> Self {
        match value {
            ImageProtocol::Kitty => Self::Kitty,
            ImageProtocol::Iterm2 => Self::Iterm2,
            ImageProtocol::Sixel => Self::Sixel,
            ImageProtocol::Unknown => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotDamage {
    row: usize,
    start_column: usize,
    end_column: usize,
}

impl From<DamageRegion> for SnapshotDamage {
    fn from(value: DamageRegion) -> Self {
        Self {
            row: value.row(),
            start_column: value.start_column(),
            end_column: value.end_column(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotEvent {
    kind: SnapshotEventKind,
    diagnostic: String,
}

impl SnapshotEvent {
    #[must_use]
    pub fn kind(&self) -> SnapshotEventKind {
        self.kind
    }

    #[must_use]
    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotEventKind {
    Unsupported,
    PayloadLimitExceeded,
    Csi,
    Osc,
    Dcs,
    Apc,
    Pm,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticEvent {
    kind: String,
    summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotCodecError {
    Serialize { message: String },
    Deserialize { message: String },
}

impl fmt::Display for SnapshotCodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serialize { message } => {
                write!(formatter, "snapshot serialize failed: {message}")
            }
            Self::Deserialize { message } => {
                write!(formatter, "snapshot deserialize failed: {message}")
            }
        }
    }
}

impl std::error::Error for SnapshotCodecError {}

pub fn serialize_snapshot(snapshot: &TerminalSnapshot) -> Result<Vec<u8>, SnapshotCodecError> {
    let bytes = serde_json::to_vec(snapshot).map_err(|error| SnapshotCodecError::Serialize {
        message: error.to_string(),
    })?;
    if bytes.len() > M1_MAX_SNAPSHOT_BYTES {
        return Err(SnapshotCodecError::Serialize {
            message: format!(
                "snapshot is {} bytes, maximum is {M1_MAX_SNAPSHOT_BYTES}",
                bytes.len()
            ),
        });
    }
    Ok(bytes)
}

pub fn serialize_snapshot_pretty(
    snapshot: &TerminalSnapshot,
) -> Result<String, SnapshotCodecError> {
    let text =
        serde_json::to_string_pretty(snapshot).map_err(|error| SnapshotCodecError::Serialize {
            message: error.to_string(),
        })?;
    if text.len() > M1_MAX_SNAPSHOT_BYTES {
        return Err(SnapshotCodecError::Serialize {
            message: format!(
                "snapshot is {} bytes, maximum is {M1_MAX_SNAPSHOT_BYTES}",
                text.len()
            ),
        });
    }
    Ok(text)
}

pub fn deserialize_snapshot(bytes: &[u8]) -> Result<TerminalSnapshot, SnapshotCodecError> {
    if bytes.len() > M1_MAX_SNAPSHOT_BYTES {
        return Err(SnapshotCodecError::Deserialize {
            message: format!(
                "snapshot is {} bytes, maximum is {M1_MAX_SNAPSHOT_BYTES}",
                bytes.len()
            ),
        });
    }

    let snapshot =
        serde_json::from_slice(bytes).map_err(|error| SnapshotCodecError::Deserialize {
            message: error.to_string(),
        })?;
    validate_snapshot(&snapshot)?;
    Ok(snapshot)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PtyRecording {
    pub schema: String,
    pub version: u32,
    pub metadata: PtyRecordingMetadata,
    pub events: Vec<PtyRecordingEvent>,
    pub final_snapshot: TerminalSnapshot,
}

impl PtyRecording {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref();
        let raw = read_pty_recording_file_capped(path)?;
        Self::from_json_str(path, &raw)
    }

    pub fn from_json_str(path: impl AsRef<Path>, json: &str) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref().to_path_buf();
        if json.len() as u64 > M2_MAX_PTY_RECORDING_FILE_BYTES {
            return Err(invalid_schema(
                &path,
                "$",
                format!(
                    "PTY recording JSON is {} bytes, maximum is {M2_MAX_PTY_RECORDING_FILE_BYTES}",
                    json.len()
                ),
            ));
        }

        let mut deserializer = serde_json::Deserializer::from_str(json);
        let recording: Self =
            serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
                recording_schema_error(&path, &error.path().to_string(), &error.inner().to_string())
            })?;
        recording.validate(&path)?;
        Ok(recording)
    }

    pub fn replay(&self) -> Result<PtyRecordingReplayReport, PtyRecordingReplayError> {
        let first = self.replay_once()?;
        if let Some(difference) = first_snapshot_difference(&self.final_snapshot, &first.snapshot) {
            return Err(PtyRecordingReplayError::SnapshotMismatch(difference));
        }

        let second = self.replay_once()?;
        if first.snapshot_bytes != second.snapshot_bytes {
            return Err(PtyRecordingReplayError::NondeterministicSnapshot {
                first_bytes: first.snapshot_bytes.len(),
                second_bytes: second.snapshot_bytes.len(),
            });
        }

        Ok(first)
    }

    fn replay_once(&self) -> Result<PtyRecordingReplayReport, PtyRecordingReplayError> {
        let initial = self.metadata.initial_size;
        let config = TerminalConfig::new(usize::from(initial.columns), usize::from(initial.rows))
            .map_err(|error| PtyRecordingReplayError::TerminalConfig {
            message: error.to_string(),
        })?;
        let mut terminal = Terminal::with_config(config);

        for event in &self.events {
            match event {
                PtyRecordingEvent::Output { bytes, .. } => terminal.advance_bytes(bytes),
                PtyRecordingEvent::Resize { columns, rows, .. } => terminal
                    .resize(usize::from(*columns), usize::from(*rows))
                    .map_err(|error| PtyRecordingReplayError::TerminalConfig {
                        message: error.to_string(),
                    })?,
                PtyRecordingEvent::Input { .. }
                | PtyRecordingEvent::Eof { .. }
                | PtyRecordingEvent::Exit { .. } => {}
            }
        }

        let snapshot = snapshot_terminal(&mut terminal);
        let snapshot_bytes = serialize_snapshot(&snapshot)?;
        Ok(PtyRecordingReplayReport {
            snapshot,
            snapshot_bytes,
        })
    }

    fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        if self.schema != M2_PTY_RECORDING_SCHEMA {
            return Err(invalid_schema(
                path,
                "schema",
                format!("expected {M2_PTY_RECORDING_SCHEMA}"),
            ));
        }
        if self.version != M2_PTY_RECORDING_VERSION {
            return Err(invalid_schema(
                path,
                "version",
                format!("expected {M2_PTY_RECORDING_VERSION}"),
            ));
        }
        if self.events.len() > M2_MAX_PTY_RECORDING_EVENTS {
            return Err(invalid_schema(
                path,
                "events",
                format!(
                    "recording has {} events, maximum is {M2_MAX_PTY_RECORDING_EVENTS}",
                    self.events.len()
                ),
            ));
        }

        validate_recording_size(path, "metadata.initial_size", self.metadata.initial_size)?;
        validate_recording_size(
            path,
            "metadata.platform.initial_size",
            self.metadata.platform.initial_size,
        )?;
        for (index, size) in self.metadata.resizes.iter().copied().enumerate() {
            validate_recording_size(path, format!("metadata.resizes[{index}]"), size)?;
        }
        validate_exit(path, "metadata.exit", &self.metadata.exit)?;
        validate_snapshot(&self.final_snapshot)
            .map_err(|error| invalid_schema(path, "final_snapshot", error.to_string()))?;

        let mut output_bytes = 0usize;
        let mut saw_exit = false;
        for (index, event) in self.events.iter().enumerate() {
            let field = format!("events[{index}]");
            match event {
                PtyRecordingEvent::Output { bytes, .. } => {
                    if bytes.is_empty() {
                        return Err(invalid_schema(
                            path,
                            field,
                            "output bytes must not be empty",
                        ));
                    }
                    if bytes.len() > M2_MAX_PTY_RECORDING_EVENT_BYTES {
                        return Err(invalid_schema(
                            path,
                            field,
                            format!(
                                "output chunk is {} bytes, maximum is {M2_MAX_PTY_RECORDING_EVENT_BYTES}",
                                bytes.len()
                            ),
                        ));
                    }
                    output_bytes = output_bytes.checked_add(bytes.len()).ok_or_else(|| {
                        invalid_schema(path, "events", "output byte count overflowed")
                    })?;
                    if output_bytes > M2_MAX_PTY_RECORDING_OUTPUT_BYTES {
                        return Err(invalid_schema(
                            path,
                            "events",
                            format!(
                                "recording output is {output_bytes} bytes, maximum is {M2_MAX_PTY_RECORDING_OUTPUT_BYTES}"
                            ),
                        ));
                    }
                }
                PtyRecordingEvent::Input { bytes, .. } => {
                    if bytes.len() > M2_MAX_PTY_RECORDING_EVENT_BYTES {
                        return Err(invalid_schema(
                            path,
                            field,
                            format!(
                                "input chunk is {} bytes, maximum is {M2_MAX_PTY_RECORDING_EVENT_BYTES}",
                                bytes.len()
                            ),
                        ));
                    }
                }
                PtyRecordingEvent::Resize { columns, rows, .. } => {
                    validate_recording_size(path, field, PtyRecordingSize::new(*columns, *rows))?;
                }
                PtyRecordingEvent::Eof { .. } => {}
                PtyRecordingEvent::Exit { exit, .. } => {
                    validate_exit(path, format!("{field}.exit"), exit)?;
                    saw_exit = true;
                }
            }
        }

        if !saw_exit {
            return Err(invalid_schema(
                path,
                "events",
                "recording must include an exit event",
            ));
        }
        if self.metadata.recording.output_bytes != output_bytes {
            return Err(invalid_schema(
                path,
                "metadata.recording.output_bytes",
                format!("expected recorded output byte count {output_bytes}"),
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PtyRecordingMetadata {
    pub command: PtyRecordingCommandMetadata,
    pub platform: PtyRecordingPlatformMetadata,
    pub exit: PtyRecordingExitMetadata,
    pub runtime: PtyRecordingRuntimeMetadata,
    pub initial_size: PtyRecordingSize,
    pub input: PtyRecordingInputMetadata,
    pub resizes: Vec<PtyRecordingSize>,
    pub recording: PtyRecordingStorageMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PtyRecordingCommandMetadata {
    pub mode: PtyRecordingCommandMode,
    pub program: String,
    pub args: Vec<String>,
    pub shell_command: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PtyRecordingCommandMode {
    Direct,
    Shell,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PtyRecordingPlatformMetadata {
    pub pty_backend: String,
    pub process_id: Option<u32>,
    pub initial_size: PtyRecordingSize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PtyRecordingExitMetadata {
    pub code: u32,
    pub signal: Option<String>,
    pub success: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PtyRecordingRuntimeMetadata {
    pub output_bytes: usize,
    pub output_chunks: usize,
    pub saw_eof: bool,
    pub drain_timed_out: bool,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PtyRecordingSize {
    pub columns: u16,
    pub rows: u16,
}

impl PtyRecordingSize {
    #[must_use]
    pub const fn new(columns: u16, rows: u16) -> Self {
        Self { columns, rows }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PtyRecordingInputMetadata {
    pub path: Option<String>,
    pub bytes: usize,
    pub max_bytes: usize,
    pub stdin_closed_after_input: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PtyRecordingStorageMetadata {
    pub enabled: bool,
    pub output_bytes: usize,
    pub output_truncated: bool,
    pub max_output_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PtyRecordingEvent {
    Output {
        elapsed_ms: u64,
        bytes: Vec<u8>,
    },
    Input {
        elapsed_ms: u64,
        bytes: Vec<u8>,
        source: Option<String>,
    },
    Resize {
        elapsed_ms: u64,
        columns: u16,
        rows: u16,
    },
    Eof {
        elapsed_ms: u64,
    },
    Exit {
        elapsed_ms: u64,
        exit: PtyRecordingExitMetadata,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M3DogfoodRecording {
    pub schema: String,
    pub version: u32,
    pub metadata: M3DogfoodMetadata,
    pub events: Vec<M3DogfoodEvent>,
    pub final_snapshot: M3DogfoodFinalSnapshot,
}

impl M3DogfoodRecording {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref();
        let raw = read_m3_dogfood_recording_file_capped(path)?;
        Self::from_json_str(path, &raw)
    }

    pub fn from_json_str(path: impl AsRef<Path>, json: &str) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref().to_path_buf();
        if json.len() as u64 > M3_MAX_DOGFOOD_RECORDING_FILE_BYTES {
            return Err(invalid_schema(
                &path,
                "$",
                format!(
                    "M3 dogfood recording JSON is {} bytes, maximum is {M3_MAX_DOGFOOD_RECORDING_FILE_BYTES}",
                    json.len()
                ),
            ));
        }

        let mut deserializer = serde_json::Deserializer::from_str(json);
        let recording: Self =
            serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
                recording_schema_error(&path, &error.path().to_string(), &error.inner().to_string())
            })?;
        recording.validate(&path)?;
        Ok(recording)
    }

    pub fn replay(&self) -> Result<M3DogfoodReplayReport, M3DogfoodReplayError> {
        let first = self.replay_once()?;
        if let Some(difference) = self
            .final_snapshot
            .first_difference(&M3DogfoodFinalSnapshot::from_snapshot(&first.snapshot))
        {
            return Err(M3DogfoodReplayError::SnapshotMismatch(difference));
        }

        let second = self.replay_once()?;
        if first.snapshot_bytes != second.snapshot_bytes {
            return Err(M3DogfoodReplayError::NondeterministicSnapshot {
                first_bytes: first.snapshot_bytes.len(),
                second_bytes: second.snapshot_bytes.len(),
            });
        }

        Ok(first)
    }

    fn replay_once(&self) -> Result<M3DogfoodReplayReport, M3DogfoodReplayError> {
        let initial = self.metadata.initial_dimensions;
        let config = TerminalConfig::new(initial.columns, initial.rows).map_err(|error| {
            M3DogfoodReplayError::TerminalConfig {
                message: error.to_string(),
            }
        })?;
        let mut terminal = Terminal::with_config(config);

        for event in &self.events {
            match event {
                M3DogfoodEvent::Output { bytes, .. } => terminal.advance_bytes(bytes),
                M3DogfoodEvent::Resize { columns, rows, .. } => terminal
                    .resize(*columns, *rows)
                    .map_err(|error| M3DogfoodReplayError::TerminalConfig {
                        message: error.to_string(),
                    })?,
                M3DogfoodEvent::Input { .. } | M3DogfoodEvent::Lifecycle { .. } => {}
            }
        }

        let snapshot = snapshot_terminal(&mut terminal);
        let snapshot_bytes = serialize_snapshot(&snapshot)?;
        Ok(M3DogfoodReplayReport {
            snapshot,
            snapshot_bytes,
        })
    }

    fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        if self.schema != M3_DOGFOOD_RECORDING_SCHEMA {
            return Err(invalid_schema(
                path,
                "schema",
                format!("expected {M3_DOGFOOD_RECORDING_SCHEMA}"),
            ));
        }
        if self.version != M3_DOGFOOD_RECORDING_VERSION {
            return Err(invalid_schema(
                path,
                "version",
                format!("expected {M3_DOGFOOD_RECORDING_VERSION}"),
            ));
        }
        validate_m3_dimensions(
            path,
            "metadata.initial_dimensions",
            self.metadata.initial_dimensions,
        )?;
        validate_m3_dimensions(
            path,
            "final_snapshot.dimensions",
            self.final_snapshot.dimensions,
        )?;
        if self.metadata.source.trim().is_empty() {
            return Err(invalid_schema(
                path,
                "metadata.source",
                "source is required",
            ));
        }
        if self.events.len() > M3_MAX_DOGFOOD_RECORDING_EVENTS {
            return Err(invalid_schema(
                path,
                "events",
                format!(
                    "recording has {} events, maximum is {M3_MAX_DOGFOOD_RECORDING_EVENTS}",
                    self.events.len()
                ),
            ));
        }
        if self.metadata.max_output_bytes > M3_MAX_DOGFOOD_RECORDING_OUTPUT_BYTES {
            return Err(invalid_schema(
                path,
                "metadata.max_output_bytes",
                format!("maximum is {M3_MAX_DOGFOOD_RECORDING_OUTPUT_BYTES}"),
            ));
        }
        if self.final_snapshot.viewport_lines.len() != self.final_snapshot.dimensions.rows {
            return Err(invalid_schema(
                path,
                "final_snapshot.viewport_lines",
                format!("expected {} rows", self.final_snapshot.dimensions.rows),
            ));
        }

        let mut counts = M3DogfoodEventCounts::default();
        let mut output_bytes = 0usize;
        for (index, event) in self.events.iter().enumerate() {
            match event {
                M3DogfoodEvent::Output { bytes, .. } => {
                    counts.output = counts.output.saturating_add(1);
                    output_bytes = output_bytes.checked_add(bytes.len()).ok_or_else(|| {
                        invalid_schema(path, "events", "output byte count overflowed")
                    })?;
                }
                M3DogfoodEvent::Input { .. } => {
                    counts.input = counts.input.saturating_add(1);
                }
                M3DogfoodEvent::Resize { columns, rows, .. } => {
                    counts.resize = counts.resize.saturating_add(1);
                    validate_m3_dimensions(
                        path,
                        format!("events[{index}]"),
                        M3DogfoodDimensions::new(*columns, *rows),
                    )?;
                }
                M3DogfoodEvent::Lifecycle { .. } => {
                    counts.lifecycle = counts.lifecycle.saturating_add(1);
                }
            }
        }

        if counts != self.metadata.event_counts {
            return Err(invalid_schema(
                path,
                "metadata.event_counts",
                "event counts do not match events",
            ));
        }
        if output_bytes != self.metadata.output_bytes {
            return Err(invalid_schema(
                path,
                "metadata.output_bytes",
                format!("expected recorded output byte count {output_bytes}"),
            ));
        }
        if output_bytes > self.metadata.max_output_bytes {
            return Err(invalid_schema(
                path,
                "events",
                format!(
                    "recording output is {output_bytes} bytes, maximum is {}",
                    self.metadata.max_output_bytes
                ),
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M3DogfoodMetadata {
    pub source: String,
    pub initial_dimensions: M3DogfoodDimensions,
    pub event_counts: M3DogfoodEventCounts,
    pub redaction_status: M3DogfoodRedactionStatus,
    pub output_bytes: usize,
    pub output_truncated: bool,
    pub max_output_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M3DogfoodDimensions {
    pub columns: usize,
    pub rows: usize,
}

impl M3DogfoodDimensions {
    #[must_use]
    pub const fn new(columns: usize, rows: usize) -> Self {
        Self { columns, rows }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M3DogfoodEventCounts {
    pub output: u64,
    pub input: u64,
    pub resize: u64,
    pub lifecycle: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M3DogfoodRedactionStatus {
    RawLocal,
    Scrubbed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum M3DogfoodEvent {
    Output {
        elapsed_ms: u64,
        bytes: Vec<u8>,
    },
    Input {
        elapsed_ms: u64,
        byte_count: usize,
        escaped_summary: String,
        truncated: bool,
    },
    Resize {
        elapsed_ms: u64,
        columns: usize,
        rows: usize,
    },
    Lifecycle {
        elapsed_ms: u64,
        state: String,
        exit_code: Option<i32>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M3DogfoodFinalSnapshot {
    pub dimensions: M3DogfoodDimensions,
    pub active_screen: SnapshotScreen,
    pub cursor: SnapshotCursor,
    pub viewport_lines: Vec<String>,
    pub scrollback_line_count: usize,
}

impl M3DogfoodFinalSnapshot {
    fn from_snapshot(snapshot: &TerminalSnapshot) -> Self {
        Self {
            dimensions: M3DogfoodDimensions::new(snapshot.columns, snapshot.rows),
            active_screen: snapshot.active_screen,
            cursor: snapshot.cursor,
            viewport_lines: snapshot
                .viewport_rows
                .iter()
                .map(SnapshotRow::text)
                .collect(),
            scrollback_line_count: snapshot.scrollback_rows.len(),
        }
    }

    fn first_difference(&self, right: &Self) -> Option<SnapshotDifference> {
        diff_value("$.dimensions", self.dimensions, right.dimensions)
            .or_else(|| diff_value("$.active_screen", self.active_screen, right.active_screen))
            .or_else(|| diff_value("$.cursor", self.cursor, right.cursor))
            .or_else(|| {
                diff_text_lines(
                    "$.viewport_lines",
                    &self.viewport_lines,
                    &right.viewport_lines,
                )
            })
            .or_else(|| {
                diff_value(
                    "$.scrollback_line_count",
                    self.scrollback_line_count,
                    right.scrollback_line_count,
                )
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct M3DogfoodReplayReport {
    snapshot: TerminalSnapshot,
    snapshot_bytes: Vec<u8>,
}

impl M3DogfoodReplayReport {
    #[must_use]
    pub const fn snapshot(&self) -> &TerminalSnapshot {
        &self.snapshot
    }

    #[must_use]
    pub fn snapshot_bytes(&self) -> &[u8] {
        &self.snapshot_bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum M3DogfoodReplayError {
    TerminalConfig {
        message: String,
    },
    SnapshotMismatch(SnapshotDifference),
    NondeterministicSnapshot {
        first_bytes: usize,
        second_bytes: usize,
    },
    SnapshotCodec(SnapshotCodecError),
}

impl fmt::Display for M3DogfoodReplayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TerminalConfig { message } => {
                write!(
                    formatter,
                    "M3 dogfood replay has invalid terminal config: {message}"
                )
            }
            Self::SnapshotMismatch(difference) => {
                write!(formatter, "M3 dogfood replay mismatch: {difference}")
            }
            Self::NondeterministicSnapshot {
                first_bytes,
                second_bytes,
            } => write!(
                formatter,
                "M3 dogfood replay was nondeterministic: first {first_bytes} bytes, second {second_bytes} bytes"
            ),
            Self::SnapshotCodec(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for M3DogfoodReplayError {}

impl From<SnapshotCodecError> for M3DogfoodReplayError {
    fn from(value: SnapshotCodecError) -> Self {
        Self::SnapshotCodec(value)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M3DogfoodMetricsReport {
    pub schema: String,
    pub version: u32,
    pub pane_id: String,
    pub timestamp_ms: u128,
    pub source: String,
    pub redaction_status: M3DogfoodRedactionStatus,
    pub session: M3DogfoodSessionMetrics,
    pub memory: M3DogfoodMemoryMetrics,
    pub latency: M3DogfoodLatencyMetrics,
    pub diff_counters: M3DogfoodDiffCounters,
    pub decision: M3DogfoodMetricsDecision,
}

impl M3DogfoodMetricsReport {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref();
        let raw = read_m3_dogfood_metrics_file_capped(path)?;
        Self::from_json_str(path, &raw)
    }

    pub fn from_json_str(path: impl AsRef<Path>, json: &str) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref().to_path_buf();
        if json.len() as u64 > M3_MAX_DOGFOOD_METRICS_FILE_BYTES {
            return Err(invalid_schema(
                &path,
                "$",
                format!(
                    "M3 dogfood metrics JSON is {} bytes, maximum is {M3_MAX_DOGFOOD_METRICS_FILE_BYTES}",
                    json.len()
                ),
            ));
        }

        let mut deserializer = serde_json::Deserializer::from_str(json);
        let report: Self =
            serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
                recording_schema_error(&path, &error.path().to_string(), &error.inner().to_string())
            })?;
        report.validate(&path)?;
        Ok(report)
    }

    fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        if self.schema != M3_DOGFOOD_METRICS_SCHEMA {
            return Err(invalid_schema(
                path,
                "schema",
                format!("expected {M3_DOGFOOD_METRICS_SCHEMA}"),
            ));
        }
        if self.version != M3_DOGFOOD_METRICS_VERSION {
            return Err(invalid_schema(
                path,
                "version",
                format!("expected {M3_DOGFOOD_METRICS_VERSION}"),
            ));
        }
        if self.pane_id.trim().is_empty() {
            return Err(invalid_schema(path, "pane_id", "pane id is required"));
        }
        if self.source.trim().is_empty() {
            return Err(invalid_schema(path, "source", "source is required"));
        }
        validate_m3_dimensions(
            path,
            "session.initial_dimensions",
            self.session.initial_dimensions,
        )?;
        if let Some(dimensions) = self.session.final_dimensions {
            validate_m3_dimensions(path, "session.final_dimensions", dimensions)?;
        }
        if self.session.logical_output_lines < M3_LONG_SESSION_MIN_LOGICAL_LINES {
            return Err(invalid_schema(
                path,
                "session.logical_output_lines",
                format!("expected at least {M3_LONG_SESSION_MIN_LOGICAL_LINES} logical lines"),
            ));
        }
        if self.session.recording_output_bytes > self.session.max_output_bytes {
            return Err(invalid_schema(
                path,
                "session.recording_output_bytes",
                "stored recording bytes exceed the configured cap",
            ));
        }
        if self.session.output_truncated
            && self.session.observed_output_bytes < self.session.recording_output_bytes
        {
            return Err(invalid_schema(
                path,
                "session.observed_output_bytes",
                "observed bytes must not be smaller than stored recording bytes",
            ));
        }
        if !self.session.output_truncated
            && self.session.observed_output_bytes != self.session.recording_output_bytes
        {
            return Err(invalid_schema(
                path,
                "session.recording_output_bytes",
                "untruncated metrics must store every observed output byte",
            ));
        }
        if self.latency.pty_batch_samples == 0
            && !matches!(
                self.latency.pty_batch_p95_ms,
                M3MetricMeasurement::NotMeasured { .. }
            )
        {
            return Err(invalid_schema(
                path,
                "latency.pty_batch_p95_ms",
                "P95 must be not_measured when there are no samples",
            ));
        }

        let replacement_blockers = self.replacement_blockers();
        if replacement_blockers.is_empty() {
            if self.decision.replacement_blocked {
                return Err(invalid_schema(
                    path,
                    "decision.replacement_blocked",
                    "replacement is blocked without a measured blocker",
                ));
            }
        } else if !self.decision.replacement_blocked || self.decision.blocked_reasons.is_empty() {
            return Err(invalid_schema(
                path,
                "decision",
                "replacement blockers must be explicit",
            ));
        }

        Ok(())
    }

    fn replacement_blockers(&self) -> Vec<String> {
        let mut blockers = Vec::new();
        match self.memory.dogfood_rss_delta_bytes.value() {
            Some(delta) if *delta > M3_10K_MEMORY_DELTA_BUDGET_BYTES => {
                blockers.push("memory".to_owned());
            }
            Some(_) => {}
            None => blockers.push("memory_not_measured".to_owned()),
        }

        match self.latency.pty_batch_p95_ms.value() {
            Some(p95) if *p95 > M3_BATCH_P95_BUDGET_MS => {
                blockers.push("latency".to_owned());
            }
            Some(_) => {}
            None => blockers.push("latency_not_measured".to_owned()),
        }

        blockers
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M3DogfoodSessionMetrics {
    pub initial_dimensions: M3DogfoodDimensions,
    pub final_dimensions: Option<M3DogfoodDimensions>,
    pub event_counts: M3DogfoodEventCounts,
    pub logical_output_lines: usize,
    pub observed_output_bytes: usize,
    pub recording_output_bytes: usize,
    pub output_truncated: bool,
    pub max_output_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M3DogfoodMemoryMetrics {
    pub paneflow_rss_baseline_bytes: M3MetricMeasurement<u64>,
    pub paneflow_rss_after_bytes: M3MetricMeasurement<u64>,
    pub dogfood_rss_delta_bytes: M3MetricMeasurement<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M3DogfoodLatencyMetrics {
    pub pty_batch_samples: usize,
    pub pty_batch_status_counts: M3DogfoodBatchStatusCounts,
    pub pty_batch_p50_ms: M3MetricMeasurement<f64>,
    pub pty_batch_p95_ms: M3MetricMeasurement<f64>,
    pub pty_batch_p99_ms: M3MetricMeasurement<f64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M3DogfoodBatchStatusCounts {
    pub rendered: u64,
    pub skipped: u64,
    pub errored: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M3DogfoodDiffCounters {
    pub equal: u64,
    pub mismatch: u64,
    pub unsupported: u64,
    pub shadow_disabled: u64,
    pub side_by_side_skipped: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M3DogfoodMetricsDecision {
    pub replacement_blocked: bool,
    pub blocked_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum M3MetricMeasurement<T> {
    Measured {
        value: T,
    },
    NotMeasured {
        os: String,
        command: String,
        reason: String,
    },
}

impl<T> M3MetricMeasurement<T> {
    fn value(&self) -> Option<&T> {
        match self {
            Self::Measured { value } => Some(value),
            Self::NotMeasured { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PtyRecordingReplayReport {
    snapshot: TerminalSnapshot,
    snapshot_bytes: Vec<u8>,
}

impl PtyRecordingReplayReport {
    #[must_use]
    pub const fn snapshot(&self) -> &TerminalSnapshot {
        &self.snapshot
    }

    #[must_use]
    pub fn snapshot_bytes(&self) -> &[u8] {
        &self.snapshot_bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PtyRecordingReplayError {
    TerminalConfig {
        message: String,
    },
    SnapshotMismatch(SnapshotDifference),
    NondeterministicSnapshot {
        first_bytes: usize,
        second_bytes: usize,
    },
    SnapshotCodec(SnapshotCodecError),
}

impl fmt::Display for PtyRecordingReplayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TerminalConfig { message } => {
                write!(
                    formatter,
                    "PTY recording replay has invalid terminal config: {message}"
                )
            }
            Self::SnapshotMismatch(difference) => {
                write!(formatter, "PTY recording replay mismatch: {difference}")
            }
            Self::NondeterministicSnapshot {
                first_bytes,
                second_bytes,
            } => write!(
                formatter,
                "PTY recording replay was nondeterministic: first {first_bytes} bytes, second {second_bytes} bytes"
            ),
            Self::SnapshotCodec(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for PtyRecordingReplayError {}

impl From<SnapshotCodecError> for PtyRecordingReplayError {
    fn from(value: SnapshotCodecError) -> Self {
        Self::SnapshotCodec(value)
    }
}

pub fn serialize_pty_recording_pretty(recording: &PtyRecording) -> Result<String, String> {
    let text = serde_json::to_string_pretty(recording).map_err(|error| error.to_string())?;
    if text.len() as u64 > M2_MAX_PTY_RECORDING_FILE_BYTES {
        return Err(format!(
            "PTY recording is {} bytes, maximum is {M2_MAX_PTY_RECORDING_FILE_BYTES}",
            text.len()
        ));
    }
    Ok(text)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotDifference {
    field: String,
    left: String,
    right: String,
}

impl SnapshotDifference {
    fn new(field: impl Into<String>, left: impl fmt::Debug, right: impl fmt::Debug) -> Self {
        Self {
            field: field.into(),
            left: format!("{left:?}"),
            right: format!("{right:?}"),
        }
    }

    #[must_use]
    pub fn field(&self) -> &str {
        &self.field
    }
}

impl fmt::Display for SnapshotDifference {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "snapshot difference at {}: left {}, right {}",
            self.field, self.left, self.right
        )
    }
}

#[must_use]
pub fn first_snapshot_difference(
    left: &TerminalSnapshot,
    right: &TerminalSnapshot,
) -> Option<SnapshotDifference> {
    diff_value("$.columns", left.columns, right.columns)
        .or_else(|| diff_value("$.rows", left.rows, right.rows))
        .or_else(|| diff_value("$.active_screen", left.active_screen, right.active_screen))
        .or_else(|| diff_value("$.modes", left.modes, right.modes))
        .or_else(|| diff_value("$.cursor", left.cursor, right.cursor))
        .or_else(|| diff_rows("$.viewport_rows", &left.viewport_rows, &right.viewport_rows))
        .or_else(|| {
            diff_rows(
                "$.scrollback_rows",
                &left.scrollback_rows,
                &right.scrollback_rows,
            )
        })
        .or_else(|| diff_value("$.events", &left.events, &right.events))
        .or_else(|| {
            diff_value(
                "$.semantic_events",
                &left.semantic_events,
                &right.semantic_events,
            )
        })
}

fn diff_value<T>(field: impl Into<String>, left: T, right: T) -> Option<SnapshotDifference>
where
    T: fmt::Debug + PartialEq,
{
    (left != right).then(|| SnapshotDifference::new(field, left, right))
}

fn diff_rows(
    field: &str,
    left: &[SnapshotRow],
    right: &[SnapshotRow],
) -> Option<SnapshotDifference> {
    if left.len() != right.len() {
        return Some(SnapshotDifference::new(
            format!("{field}.len"),
            left.len(),
            right.len(),
        ));
    }

    for (index, (left, right)) in left.iter().zip(right.iter()).enumerate() {
        let prefix = format!("{field}[{index}]");
        if left.handle != right.handle {
            return Some(SnapshotDifference::new(
                format!("{prefix}.handle"),
                left.handle,
                right.handle,
            ));
        }
        if left.wrapped != right.wrapped {
            return Some(SnapshotDifference::new(
                format!("{prefix}.wrapped"),
                left.wrapped,
                right.wrapped,
            ));
        }
        if left.cells.len() != right.cells.len() {
            return Some(SnapshotDifference::new(
                format!("{prefix}.cells.len"),
                left.cells.len(),
                right.cells.len(),
            ));
        }

        for (cell_index, (left_cell, right_cell)) in
            left.cells.iter().zip(right.cells.iter()).enumerate()
        {
            if let Some(difference) = diff_cell(
                &format!("{prefix}.cells[{cell_index}]"),
                left_cell,
                right_cell,
            ) {
                return Some(difference);
            }
        }
    }

    None
}

fn diff_text_lines(field: &str, left: &[String], right: &[String]) -> Option<SnapshotDifference> {
    if left.len() != right.len() {
        return Some(SnapshotDifference::new(
            format!("{field}.len"),
            left.len(),
            right.len(),
        ));
    }

    for (index, (left, right)) in left.iter().zip(right.iter()).enumerate() {
        if left != right {
            return Some(SnapshotDifference::new(
                format!("{field}[{index}]"),
                left,
                right,
            ));
        }
    }

    None
}

fn diff_cell(field: &str, left: &SnapshotCell, right: &SnapshotCell) -> Option<SnapshotDifference> {
    diff_value(format!("{field}.ch"), left.ch, right.ch)
        .or_else(|| diff_value(format!("{field}.width"), left.width, right.width))
        .or_else(|| diff_style(&format!("{field}.style"), &left.style, &right.style))
        .or_else(|| diff_image(&format!("{field}.image"), &left.image, &right.image))
}

fn diff_style(
    field: &str,
    left: &SnapshotStyle,
    right: &SnapshotStyle,
) -> Option<SnapshotDifference> {
    diff_value(
        format!("{field}.foreground"),
        left.foreground,
        right.foreground,
    )
    .or_else(|| {
        diff_value(
            format!("{field}.background"),
            left.background,
            right.background,
        )
    })
    .or_else(|| diff_value(format!("{field}.bold"), left.bold, right.bold))
    .or_else(|| diff_value(format!("{field}.italic"), left.italic, right.italic))
    .or_else(|| {
        diff_value(
            format!("{field}.underline"),
            left.underline,
            right.underline,
        )
    })
    .or_else(|| diff_value(format!("{field}.inverse"), left.inverse, right.inverse))
}

fn diff_image(
    field: &str,
    left: &Option<SnapshotImage>,
    right: &Option<SnapshotImage>,
) -> Option<SnapshotDifference> {
    match (left, right) {
        (Some(left), Some(right)) => {
            diff_value(format!("{field}.protocol"), left.protocol, right.protocol)
                .or_else(|| diff_value(format!("{field}.id"), &left.id, &right.id))
                .or_else(|| diff_value(format!("{field}.byte_len"), left.byte_len, right.byte_len))
                .or_else(|| {
                    diff_value(
                        format!("{field}.diagnostic"),
                        &left.diagnostic,
                        &right.diagnostic,
                    )
                })
        }
        _ => diff_value(field, left, right),
    }
}

fn validate_snapshot(snapshot: &TerminalSnapshot) -> Result<(), SnapshotCodecError> {
    Dimensions::new(snapshot.columns, snapshot.rows).map_err(|error| {
        snapshot_validation_error(format!("invalid snapshot dimensions: {error}"))
    })?;

    if snapshot.cursor.row >= snapshot.rows || snapshot.cursor.column >= snapshot.columns {
        return Err(snapshot_validation_error(format!(
            "cursor is outside snapshot dimensions: row={}, column={}, rows={}, columns={}",
            snapshot.cursor.row, snapshot.cursor.column, snapshot.rows, snapshot.columns
        )));
    }

    if snapshot.viewport_rows.len() != snapshot.rows {
        return Err(snapshot_validation_error(format!(
            "viewport has {} rows, terminal requires {} rows",
            snapshot.viewport_rows.len(),
            snapshot.rows
        )));
    }

    let row_count = snapshot
        .viewport_rows
        .len()
        .saturating_add(snapshot.scrollback_rows.len());
    if row_count > M1_MAX_SNAPSHOT_ROWS {
        return Err(snapshot_validation_error(format!(
            "snapshot has {row_count} rows, maximum is {M1_MAX_SNAPSHOT_ROWS}"
        )));
    }

    let mut cell_count = 0usize;
    for (index, row) in snapshot.viewport_rows.iter().enumerate() {
        if row.cells.len() != snapshot.columns {
            return Err(snapshot_validation_error(format!(
                "viewport row {index} has {} cells, terminal requires {} columns",
                row.cells.len(),
                snapshot.columns
            )));
        }
        cell_count = cell_count.saturating_add(row.cells.len());
        validate_snapshot_cells(&row.cells)?;
    }
    for row in &snapshot.scrollback_rows {
        if row.cells.len() != snapshot.columns {
            return Err(snapshot_validation_error(format!(
                "scrollback row has {} cells, terminal requires {} columns",
                row.cells.len(),
                snapshot.columns
            )));
        }
        cell_count = cell_count.saturating_add(row.cells.len());
        validate_snapshot_cells(&row.cells)?;
    }

    if cell_count > M1_MAX_SNAPSHOT_CELLS {
        return Err(snapshot_validation_error(format!(
            "snapshot has {cell_count} cells, maximum is {M1_MAX_SNAPSHOT_CELLS}"
        )));
    }

    if snapshot.damage.len() > snapshot.rows {
        return Err(snapshot_validation_error(format!(
            "snapshot has {} damage regions, maximum is {}",
            snapshot.damage.len(),
            snapshot.rows
        )));
    }
    for damage in &snapshot.damage {
        if damage.row >= snapshot.rows
            || damage.start_column > damage.end_column
            || damage.end_column > snapshot.columns
        {
            return Err(snapshot_validation_error(format!(
                "damage region is outside snapshot dimensions: row={}, start_column={}, end_column={}",
                damage.row, damage.start_column, damage.end_column
            )));
        }
    }

    if snapshot.events.len() > M1_MAX_SNAPSHOT_EVENTS {
        return Err(snapshot_validation_error(format!(
            "snapshot has {} events, maximum is {M1_MAX_SNAPSHOT_EVENTS}",
            snapshot.events.len()
        )));
    }
    if snapshot.semantic_events.len() > M1_MAX_SNAPSHOT_EVENTS {
        return Err(snapshot_validation_error(format!(
            "snapshot has {} semantic events, maximum is {M1_MAX_SNAPSHOT_EVENTS}",
            snapshot.semantic_events.len()
        )));
    }
    for event in &snapshot.events {
        validate_snapshot_string("event diagnostic", &event.diagnostic)?;
    }
    for event in &snapshot.semantic_events {
        validate_snapshot_string("semantic kind", &event.kind)?;
        validate_snapshot_string("semantic summary", &event.summary)?;
    }

    Ok(())
}

fn validate_snapshot_cells(cells: &[SnapshotCell]) -> Result<(), SnapshotCodecError> {
    for cell in cells {
        if let Some(image) = &cell.image {
            if let Some(id) = &image.id {
                validate_snapshot_string("image id", id)?;
            }
            validate_snapshot_string("image diagnostic", &image.diagnostic)?;
        }
    }

    Ok(())
}

fn validate_snapshot_string(name: &str, value: &str) -> Result<(), SnapshotCodecError> {
    if value.len() > M1_MAX_SNAPSHOT_STRING_BYTES {
        return Err(snapshot_validation_error(format!(
            "{name} is {} bytes, maximum is {M1_MAX_SNAPSHOT_STRING_BYTES}",
            value.len()
        )));
    }

    Ok(())
}

fn snapshot_validation_error(message: String) -> SnapshotCodecError {
    SnapshotCodecError::Deserialize { message }
}

#[derive(Debug)]
pub enum FixtureError {
    Load(FixtureLoadError),
    Run(FixtureRunError),
}

impl fmt::Display for FixtureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Load(error) => write!(formatter, "{error}"),
            Self::Run(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for FixtureError {}

#[derive(Debug)]
pub enum FixtureLoadError {
    Io {
        path: PathBuf,
        message: String,
    },
    InvalidSchema {
        path: PathBuf,
        field: String,
        message: String,
    },
}

impl fmt::Display for FixtureLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, message } => {
                write!(
                    formatter,
                    "fixture load failed at {}: {message}",
                    path.display()
                )
            }
            Self::InvalidSchema {
                path,
                field,
                message,
            } => write!(
                formatter,
                "invalid fixture schema at {} field {field}: {message}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for FixtureLoadError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixtureRunError {
    TerminalConfig { fixture: String, message: String },
    Assertion(FixtureAssertionFailure),
    SnapshotCodec(SnapshotCodecError),
}

impl fmt::Display for FixtureRunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TerminalConfig { fixture, message } => {
                write!(
                    formatter,
                    "fixture {fixture} has invalid terminal config: {message}"
                )
            }
            Self::Assertion(failure) => write!(formatter, "{failure}"),
            Self::SnapshotCodec(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for FixtureRunError {}

impl From<SnapshotCodecError> for FixtureRunError {
    fn from(value: SnapshotCodecError) -> Self {
        Self::SnapshotCodec(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureAssertionFailure {
    fixture_name: String,
    field: String,
    expected: String,
    actual: String,
}

impl FixtureAssertionFailure {
    fn new(
        fixture_name: &str,
        field: impl Into<String>,
        expected: impl Into<String>,
        actual: impl Into<String>,
    ) -> Self {
        Self {
            fixture_name: fixture_name.to_owned(),
            field: field.into(),
            expected: expected.into(),
            actual: actual.into(),
        }
    }

    #[must_use]
    pub fn fixture_name(&self) -> &str {
        &self.fixture_name
    }

    #[must_use]
    pub fn field(&self) -> &str {
        &self.field
    }
}

impl fmt::Display for FixtureAssertionFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "snapshot mismatch in fixture {} at {}: expected {}, actual {}",
            self.fixture_name, self.field, self.expected, self.actual
        )
    }
}

impl std::error::Error for FixtureAssertionFailure {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFixturePack {
    fixtures: Option<Vec<RawFixture>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFixture {
    name: Option<String>,
    terminal: Option<RawTerminalSpec>,
    chunks: Option<Vec<RawInputChunk>>,
    expected: Option<ExpectedSnapshot>,
    #[serde(default)]
    checkpoints: Option<Vec<ExpectedCheckpoint>>,
    split_every_boundary: Option<bool>,
    deterministic: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTerminalSpec {
    columns: Option<usize>,
    rows: Option<usize>,
    scrollback: Option<RawScrollbackSpec>,
}

impl RawTerminalSpec {
    fn validate(self, path: &Path, prefix: &str) -> Result<TerminalSpec, FixtureLoadError> {
        let spec = TerminalSpec {
            columns: self.columns.unwrap_or(80),
            rows: self.rows.unwrap_or(24),
            scrollback: self.scrollback.unwrap_or_default().into(),
        };

        spec.to_config().map_err(|error| {
            invalid_schema(path, format!("{prefix}.terminal"), error.to_string())
        })?;

        Ok(spec)
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawScrollbackSpec {
    max_lines: Option<usize>,
    max_bytes: Option<usize>,
}

impl Default for RawScrollbackSpec {
    fn default() -> Self {
        Self {
            max_lines: Some(10_000),
            max_bytes: Some(8 * 1024 * 1024),
        }
    }
}

impl From<RawScrollbackSpec> for ScrollbackSpec {
    fn from(value: RawScrollbackSpec) -> Self {
        Self {
            max_lines: value.max_lines.unwrap_or(10_000),
            max_bytes: value.max_bytes.unwrap_or(8 * 1024 * 1024),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawInputChunk {
    bytes: Option<Vec<u8>>,
    repeat: Option<RawRepeat>,
    resize: Option<RawResize>,
}

impl RawInputChunk {
    fn expanded_len(&self, path: &Path, field: &str) -> Result<usize, FixtureLoadError> {
        match (&self.bytes, &self.repeat, &self.resize) {
            (Some(bytes), None, None) => Ok(bytes.len()),
            (None, Some(repeat), None) => Ok(repeat.count),
            (None, None, Some(_)) => Ok(0),
            _ => Err(invalid_schema(
                path,
                field,
                "chunk must contain exactly one of bytes, repeat or resize",
            )),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRepeat {
    byte: u8,
    count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawResize {
    columns: usize,
    rows: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedCheckpoint {
    after_chunk: usize,
    expected: ExpectedSnapshot,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedSnapshot {
    columns: Option<usize>,
    rows: Option<usize>,
    active_screen: Option<SnapshotScreen>,
    modes: Option<SnapshotModes>,
    cursor: Option<SnapshotCursor>,
    viewport_lines: Option<Vec<String>>,
    scrollback_lines: Option<Vec<String>>,
    #[serde(default)]
    cell_styles: Vec<ExpectedCellStyle>,
    #[serde(default)]
    wrapped_viewport_rows: Vec<usize>,
    #[serde(default)]
    wrapped_scrollback_rows: Vec<usize>,
    #[serde(default)]
    events: Vec<ExpectedEvent>,
}

impl ExpectedSnapshot {
    fn has_assertion(&self) -> bool {
        self.columns.is_some()
            || self.rows.is_some()
            || self.active_screen.is_some()
            || self.modes.is_some()
            || self.cursor.is_some()
            || self.viewport_lines.is_some()
            || self.scrollback_lines.is_some()
            || !self.cell_styles.is_empty()
            || !self.wrapped_viewport_rows.is_empty()
            || !self.wrapped_scrollback_rows.is_empty()
            || !self.events.is_empty()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedCellStyle {
    row: usize,
    column: usize,
    default: Option<bool>,
    foreground: Option<SnapshotColor>,
    background: Option<SnapshotColor>,
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
    inverse: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedEvent {
    kind: SnapshotEventKind,
    diagnostic_contains: String,
}

struct ReplayOutput {
    snapshot: TerminalSnapshot,
}

fn replay_fixture(
    fixture: &Fixture,
    chunks: &[InputChunk],
    check_checkpoints: bool,
) -> Result<ReplayOutput, FixtureRunError> {
    let config = fixture
        .terminal
        .to_config()
        .map_err(|error| FixtureRunError::TerminalConfig {
            fixture: fixture.name.clone(),
            message: error.to_string(),
        })?;
    let mut terminal = Terminal::with_config(config);

    for (index, chunk) in chunks.iter().enumerate() {
        if let Some(resize) = chunk.resize {
            terminal
                .resize(resize.columns, resize.rows)
                .map_err(|error| FixtureRunError::TerminalConfig {
                    fixture: fixture.name.clone(),
                    message: error.to_string(),
                })?;
        } else {
            terminal.advance_bytes(&chunk.bytes);
        }

        if check_checkpoints {
            for checkpoint in fixture
                .checkpoints
                .iter()
                .filter(|checkpoint| checkpoint.after_chunk == index + 1)
            {
                let actions = terminal.actions().to_vec();
                let snapshot = TerminalSnapshot::from_render(terminal.render_snapshot(), &actions);
                verify_expected(
                    &fixture.name,
                    &format!("checkpoints[{}]", index + 1),
                    &checkpoint.expected,
                    &snapshot,
                )?;
            }
        }
    }

    let actions = terminal.actions().to_vec();
    let snapshot = TerminalSnapshot::from_render(terminal.render_snapshot(), &actions);
    Ok(ReplayOutput { snapshot })
}

fn verify_split_boundaries(
    fixture: &Fixture,
    expected_snapshot: &TerminalSnapshot,
) -> Result<(), FixtureRunError> {
    let expected_bytes = serialize_snapshot_without_damage(expected_snapshot)?;
    let bytes = fixture
        .chunks
        .iter()
        .flat_map(|chunk| chunk.bytes.iter().copied())
        .collect::<Vec<_>>();

    for split in 1..bytes.len() {
        let chunks = vec![
            InputChunk {
                bytes: bytes[..split].to_vec(),
                resize: None,
            },
            InputChunk {
                bytes: bytes[split..].to_vec(),
                resize: None,
            },
        ];
        let snapshot = replay_fixture(fixture, &chunks, false)?.snapshot;
        let snapshot_bytes = serialize_snapshot_without_damage(&snapshot)?;

        if snapshot_bytes != expected_bytes {
            return Err(FixtureRunError::Assertion(FixtureAssertionFailure::new(
                &fixture.name,
                format!("split_every_boundary[{split}]"),
                format!("{} bytes", expected_bytes.len()),
                format!("{} bytes", snapshot_bytes.len()),
            )));
        }
    }

    Ok(())
}

fn serialize_snapshot_without_damage(
    snapshot: &TerminalSnapshot,
) -> Result<Vec<u8>, SnapshotCodecError> {
    let mut snapshot = snapshot.clone();
    snapshot.damage.clear();
    serialize_snapshot(&snapshot)
}

fn verify_expected(
    fixture_name: &str,
    prefix: &str,
    expected: &ExpectedSnapshot,
    snapshot: &TerminalSnapshot,
) -> Result<(), FixtureRunError> {
    assert_field(
        fixture_name,
        prefix,
        "columns",
        expected.columns,
        snapshot.columns,
    )?;
    assert_field(fixture_name, prefix, "rows", expected.rows, snapshot.rows)?;
    assert_field(
        fixture_name,
        prefix,
        "active_screen",
        expected.active_screen,
        snapshot.active_screen,
    )?;
    assert_field(
        fixture_name,
        prefix,
        "modes",
        expected.modes,
        snapshot.modes,
    )?;
    assert_field(
        fixture_name,
        prefix,
        "cursor",
        expected.cursor,
        snapshot.cursor,
    )?;

    if let Some(lines) = &expected.viewport_lines {
        compare_lines(
            fixture_name,
            &format!("{prefix}.viewport_rows"),
            lines,
            &snapshot.viewport_rows,
        )?;
    }

    if let Some(lines) = &expected.scrollback_lines {
        compare_lines(
            fixture_name,
            &format!("{prefix}.scrollback_rows"),
            lines,
            &snapshot.scrollback_rows,
        )?;
    }

    compare_wrapped_rows(
        fixture_name,
        &format!("{prefix}.wrapped_viewport_rows"),
        &expected.wrapped_viewport_rows,
        &snapshot.viewport_rows,
    )?;
    compare_wrapped_rows(
        fixture_name,
        &format!("{prefix}.wrapped_scrollback_rows"),
        &expected.wrapped_scrollback_rows,
        &snapshot.scrollback_rows,
    )?;

    compare_cell_styles(
        fixture_name,
        &format!("{prefix}.cell_styles"),
        &expected.cell_styles,
        snapshot,
    )?;

    for event in &expected.events {
        let found = snapshot.events.iter().any(|observed| {
            observed.kind == event.kind && observed.diagnostic.contains(&event.diagnostic_contains)
        });
        if !found {
            return Err(FixtureRunError::Assertion(FixtureAssertionFailure::new(
                fixture_name,
                format!("{prefix}.events"),
                format!("{:?} containing {}", event.kind, event.diagnostic_contains),
                format!("{:?}", snapshot.events),
            )));
        }
    }

    Ok(())
}

fn assert_field<T>(
    fixture_name: &str,
    prefix: &str,
    field: &str,
    expected: Option<T>,
    actual: T,
) -> Result<(), FixtureRunError>
where
    T: fmt::Debug + PartialEq,
{
    if let Some(expected) = expected {
        if expected != actual {
            return Err(FixtureRunError::Assertion(FixtureAssertionFailure::new(
                fixture_name,
                format!("{prefix}.{field}"),
                format!("{expected:?}"),
                format!("{actual:?}"),
            )));
        }
    }

    Ok(())
}

fn compare_lines(
    fixture_name: &str,
    field: &str,
    expected: &[String],
    actual: &[SnapshotRow],
) -> Result<(), FixtureRunError> {
    if expected.len() != actual.len() {
        return Err(FixtureRunError::Assertion(FixtureAssertionFailure::new(
            fixture_name,
            format!("{field}.len"),
            expected.len().to_string(),
            actual.len().to_string(),
        )));
    }

    for (row_index, (expected, actual)) in expected.iter().zip(actual.iter()).enumerate() {
        let actual_text = actual.text();
        let expected_text = pad_to_width(expected, actual_text.chars().count());

        if expected_text != actual_text {
            let field = first_cell_diff(&expected_text, &actual_text).map_or_else(
                || format!("{field}[{row_index}].text"),
                |column| format!("{field}[{row_index}].cells[{column}].ch"),
            );
            return Err(FixtureRunError::Assertion(FixtureAssertionFailure::new(
                fixture_name,
                field,
                expected_text,
                actual_text,
            )));
        }
    }

    Ok(())
}

fn compare_wrapped_rows(
    fixture_name: &str,
    field: &str,
    expected: &[usize],
    actual: &[SnapshotRow],
) -> Result<(), FixtureRunError> {
    let actual_wrapped = actual
        .iter()
        .enumerate()
        .filter_map(|(index, row)| row.wrapped().then_some(index))
        .collect::<Vec<_>>();

    if expected != actual_wrapped {
        return Err(FixtureRunError::Assertion(FixtureAssertionFailure::new(
            fixture_name,
            field,
            format!("{expected:?}"),
            format!("{actual_wrapped:?}"),
        )));
    }

    Ok(())
}

fn compare_cell_styles(
    fixture_name: &str,
    field: &str,
    expected: &[ExpectedCellStyle],
    snapshot: &TerminalSnapshot,
) -> Result<(), FixtureRunError> {
    for (index, expected_style) in expected.iter().enumerate() {
        let Some(row) = snapshot.viewport_rows.get(expected_style.row) else {
            return Err(FixtureRunError::Assertion(FixtureAssertionFailure::new(
                fixture_name,
                format!("{field}[{index}].row"),
                expected_style.row.to_string(),
                "missing row",
            )));
        };
        let Some(cell) = row.cells.get(expected_style.column) else {
            return Err(FixtureRunError::Assertion(FixtureAssertionFailure::new(
                fixture_name,
                format!("{field}[{index}].column"),
                expected_style.column.to_string(),
                "missing cell",
            )));
        };

        if expected_style.default == Some(true) && cell.style != SnapshotStyle::default() {
            return Err(FixtureRunError::Assertion(FixtureAssertionFailure::new(
                fixture_name,
                format!("{field}[{index}].default"),
                "default style",
                format!("{:?}", cell.style),
            )));
        }
        assert_style_field(
            fixture_name,
            field,
            index,
            "foreground",
            expected_style.foreground,
            cell.style.foreground,
        )?;
        assert_style_field(
            fixture_name,
            field,
            index,
            "background",
            expected_style.background,
            cell.style.background,
        )?;
        assert_bool_style_field(
            fixture_name,
            field,
            index,
            "bold",
            expected_style.bold,
            cell.style.bold,
        )?;
        assert_bool_style_field(
            fixture_name,
            field,
            index,
            "italic",
            expected_style.italic,
            cell.style.italic,
        )?;
        assert_bool_style_field(
            fixture_name,
            field,
            index,
            "underline",
            expected_style.underline,
            cell.style.underline,
        )?;
        assert_bool_style_field(
            fixture_name,
            field,
            index,
            "inverse",
            expected_style.inverse,
            cell.style.inverse,
        )?;
    }

    Ok(())
}

fn assert_style_field(
    fixture_name: &str,
    field: &str,
    index: usize,
    name: &str,
    expected: Option<SnapshotColor>,
    actual: Option<SnapshotColor>,
) -> Result<(), FixtureRunError> {
    if let Some(expected) = expected {
        if Some(expected) != actual {
            return Err(FixtureRunError::Assertion(FixtureAssertionFailure::new(
                fixture_name,
                format!("{field}[{index}].{name}"),
                format!("{expected:?}"),
                format!("{actual:?}"),
            )));
        }
    }

    Ok(())
}

fn assert_bool_style_field(
    fixture_name: &str,
    field: &str,
    index: usize,
    name: &str,
    expected: Option<bool>,
    actual: bool,
) -> Result<(), FixtureRunError> {
    if let Some(expected) = expected {
        if expected != actual {
            return Err(FixtureRunError::Assertion(FixtureAssertionFailure::new(
                fixture_name,
                format!("{field}[{index}].{name}"),
                expected.to_string(),
                actual.to_string(),
            )));
        }
    }

    Ok(())
}

fn snapshot_events(actions: &[TerminalAction]) -> Vec<SnapshotEvent> {
    let mut events = Vec::new();

    for action in actions {
        match action {
            TerminalAction::Unsupported(sequence) => events.push(SnapshotEvent {
                kind: SnapshotEventKind::Unsupported,
                diagnostic: format!(
                    "{:?}: {}",
                    unsupported_kind(sequence.kind()),
                    sequence.diagnostic()
                ),
            }),
            TerminalAction::Csi(sequence) => events.push(SnapshotEvent {
                kind: SnapshotEventKind::Csi,
                diagnostic: format_csi(
                    sequence.params(),
                    sequence.intermediates(),
                    sequence.action(),
                ),
            }),
            TerminalAction::Osc(command) => {
                events.push(SnapshotEvent {
                    kind: SnapshotEventKind::Osc,
                    diagnostic: format!("OSC {}", payload_preview(command.payload().bytes())),
                });
                push_payload_limit(
                    &mut events,
                    "OSC",
                    command.payload().status(),
                    SnapshotEventKind::PayloadLimitExceeded,
                );
            }
            TerminalAction::Dcs(command) => {
                events.push(SnapshotEvent {
                    kind: SnapshotEventKind::Dcs,
                    diagnostic: format!(
                        "DCS {} {}",
                        format_params(command.params()),
                        payload_preview(command.payload().bytes())
                    ),
                });
                push_payload_limit(
                    &mut events,
                    "DCS",
                    command.payload().status(),
                    SnapshotEventKind::PayloadLimitExceeded,
                );
            }
            TerminalAction::Apc(control) => {
                events.push(SnapshotEvent {
                    kind: SnapshotEventKind::Apc,
                    diagnostic: format!("APC {}", payload_preview(control.payload().bytes())),
                });
                push_payload_limit(
                    &mut events,
                    "APC",
                    control.payload().status(),
                    SnapshotEventKind::PayloadLimitExceeded,
                );
            }
            TerminalAction::Pm(control) => {
                events.push(SnapshotEvent {
                    kind: SnapshotEventKind::Pm,
                    diagnostic: format!("PM {}", payload_preview(control.payload().bytes())),
                });
                push_payload_limit(
                    &mut events,
                    "PM",
                    control.payload().status(),
                    SnapshotEventKind::PayloadLimitExceeded,
                );
            }
            TerminalAction::Print(_) | TerminalAction::Control(_) | TerminalAction::Escape(_) => {}
        }
    }

    events
}

fn snapshot_modes(actions: &[TerminalAction]) -> SnapshotModes {
    let mut bracketed_paste = false;

    for action in actions {
        let TerminalAction::Csi(sequence) = action else {
            continue;
        };

        if sequence.intermediates() != b"?" || !matches!(sequence.action(), 'h' | 'l') {
            continue;
        }

        if sequence
            .params()
            .iter()
            .any(|param| param.subparameters().first() == Some(&2004))
        {
            bracketed_paste = sequence.action() == 'h';
        }
    }

    SnapshotModes::new(bracketed_paste)
}

fn push_payload_limit(
    events: &mut Vec<SnapshotEvent>,
    label: &str,
    status: &PayloadStatus,
    kind: SnapshotEventKind,
) {
    if let PayloadStatus::Truncated {
        original_len,
        retained_len,
        limit,
    } = status
    {
        events.push(SnapshotEvent {
            kind,
            diagnostic: format!(
                "{label} payload limit exceeded: original_len={original_len}, retained_len={retained_len}, limit={limit}"
            ),
        });
    }
}

fn format_csi(params: &[terminal_core::CsiParam], intermediates: &[u8], action: char) -> String {
    let intermediate = String::from_utf8_lossy(intermediates);
    format!("CSI {}{}{}", intermediate, format_params(params), action)
}

fn format_params(params: &[terminal_core::CsiParam]) -> String {
    params
        .iter()
        .map(|param| {
            param
                .subparameters()
                .iter()
                .map(u16::to_string)
                .collect::<Vec<_>>()
                .join(":")
        })
        .collect::<Vec<_>>()
        .join(";")
}

fn payload_preview(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).chars().take(80).collect()
}

fn unsupported_kind(kind: UnsupportedSequenceKind) -> &'static str {
    match kind {
        UnsupportedSequenceKind::PayloadLimitExceeded => "payload_limit_exceeded",
        UnsupportedSequenceKind::ParserIgnored => "parser_ignored",
        UnsupportedSequenceKind::UnterminatedDcs => "unterminated_dcs",
        UnsupportedSequenceKind::Other => "other",
    }
}

fn first_cell_diff(expected: &str, actual: &str) -> Option<usize> {
    expected
        .chars()
        .zip(actual.chars())
        .position(|(expected, actual)| expected != actual)
}

fn pad_to_width(value: &str, width: usize) -> String {
    let mut padded = value.to_owned();
    let current = padded.chars().count();
    if current < width {
        padded.extend(std::iter::repeat_n(' ', width - current));
    }
    padded
}

fn read_fixture_file_capped(path: &Path) -> Result<String, FixtureLoadError> {
    let metadata = fs::metadata(path).map_err(|error| FixtureLoadError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    if !metadata.is_file() {
        return Err(invalid_schema(
            path,
            "$",
            "fixture path must be a regular file",
        ));
    }
    if metadata.len() > M1_MAX_FIXTURE_FILE_BYTES {
        return Err(invalid_schema(
            path,
            "$",
            format!(
                "fixture file is {} bytes, maximum is {M1_MAX_FIXTURE_FILE_BYTES}",
                metadata.len()
            ),
        ));
    }

    let file = fs::File::open(path).map_err(|error| FixtureLoadError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    let mut reader = file.take(M1_MAX_FIXTURE_FILE_BYTES + 1);
    let mut raw = String::new();
    reader
        .read_to_string(&mut raw)
        .map_err(|error| FixtureLoadError::Io {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;

    if raw.len() as u64 > M1_MAX_FIXTURE_FILE_BYTES {
        return Err(invalid_schema(
            path,
            "$",
            format!(
                "fixture file exceeded maximum of {M1_MAX_FIXTURE_FILE_BYTES} bytes while reading"
            ),
        ));
    }

    Ok(raw)
}

fn read_pty_recording_file_capped(path: &Path) -> Result<String, FixtureLoadError> {
    let metadata = fs::metadata(path).map_err(|error| FixtureLoadError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    if !metadata.is_file() {
        return Err(invalid_schema(
            path,
            "$",
            "PTY recording path must be a regular file",
        ));
    }
    if metadata.len() > M2_MAX_PTY_RECORDING_FILE_BYTES {
        return Err(invalid_schema(
            path,
            "$",
            format!(
                "PTY recording file is {} bytes, maximum is {M2_MAX_PTY_RECORDING_FILE_BYTES}",
                metadata.len()
            ),
        ));
    }

    let file = fs::File::open(path).map_err(|error| FixtureLoadError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    let mut reader = file.take(M2_MAX_PTY_RECORDING_FILE_BYTES + 1);
    let mut raw = String::new();
    reader
        .read_to_string(&mut raw)
        .map_err(|error| FixtureLoadError::Io {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;

    if raw.len() as u64 > M2_MAX_PTY_RECORDING_FILE_BYTES {
        return Err(invalid_schema(
            path,
            "$",
            format!(
                "PTY recording file exceeded maximum of {M2_MAX_PTY_RECORDING_FILE_BYTES} bytes while reading"
            ),
        ));
    }

    Ok(raw)
}

fn read_m3_dogfood_recording_file_capped(path: &Path) -> Result<String, FixtureLoadError> {
    let metadata = fs::metadata(path).map_err(|error| FixtureLoadError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    if !metadata.is_file() {
        return Err(invalid_schema(
            path,
            "$",
            "M3 dogfood recording path must be a regular file",
        ));
    }
    if metadata.len() > M3_MAX_DOGFOOD_RECORDING_FILE_BYTES {
        return Err(invalid_schema(
            path,
            "$",
            format!(
                "M3 dogfood recording file is {} bytes, maximum is {M3_MAX_DOGFOOD_RECORDING_FILE_BYTES}",
                metadata.len()
            ),
        ));
    }

    let file = fs::File::open(path).map_err(|error| FixtureLoadError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    let mut reader = file.take(M3_MAX_DOGFOOD_RECORDING_FILE_BYTES + 1);
    let mut raw = String::new();
    reader
        .read_to_string(&mut raw)
        .map_err(|error| FixtureLoadError::Io {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;

    if raw.len() as u64 > M3_MAX_DOGFOOD_RECORDING_FILE_BYTES {
        return Err(invalid_schema(
            path,
            "$",
            format!(
                "M3 dogfood recording file exceeded maximum of {M3_MAX_DOGFOOD_RECORDING_FILE_BYTES} bytes while reading"
            ),
        ));
    }

    Ok(raw)
}

fn read_m3_dogfood_metrics_file_capped(path: &Path) -> Result<String, FixtureLoadError> {
    let metadata = fs::metadata(path).map_err(|error| FixtureLoadError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    if !metadata.is_file() {
        return Err(invalid_schema(
            path,
            "$",
            "M3 dogfood metrics path must be a regular file",
        ));
    }
    if metadata.len() > M3_MAX_DOGFOOD_METRICS_FILE_BYTES {
        return Err(invalid_schema(
            path,
            "$",
            format!(
                "M3 dogfood metrics file is {} bytes, maximum is {M3_MAX_DOGFOOD_METRICS_FILE_BYTES}",
                metadata.len()
            ),
        ));
    }

    let file = fs::File::open(path).map_err(|error| FixtureLoadError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    let mut reader = file.take(M3_MAX_DOGFOOD_METRICS_FILE_BYTES + 1);
    let mut raw = String::new();
    reader
        .read_to_string(&mut raw)
        .map_err(|error| FixtureLoadError::Io {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;

    if raw.len() as u64 > M3_MAX_DOGFOOD_METRICS_FILE_BYTES {
        return Err(invalid_schema(
            path,
            "$",
            format!(
                "M3 dogfood metrics file exceeded maximum of {M3_MAX_DOGFOOD_METRICS_FILE_BYTES} bytes while reading"
            ),
        ));
    }

    Ok(raw)
}

fn validate_recording_size(
    path: &Path,
    field: impl Into<String>,
    size: PtyRecordingSize,
) -> Result<(), FixtureLoadError> {
    Dimensions::new(usize::from(size.columns), usize::from(size.rows))
        .map(|_| ())
        .map_err(|error| invalid_schema(path, field, error.to_string()))
}

fn validate_m3_dimensions(
    path: &Path,
    field: impl Into<String>,
    size: M3DogfoodDimensions,
) -> Result<(), FixtureLoadError> {
    Dimensions::new(size.columns, size.rows)
        .map(|_| ())
        .map_err(|error| invalid_schema(path, field, error.to_string()))
}

fn validate_exit(
    path: &Path,
    field: impl Into<String>,
    exit: &PtyRecordingExitMetadata,
) -> Result<(), FixtureLoadError> {
    let expected_success = exit.code == 0 && exit.signal.is_none();
    if exit.success != expected_success {
        return Err(invalid_schema(
            path,
            field,
            format!(
                "exit success must be {expected_success} for code={} signal={:?}",
                exit.code, exit.signal
            ),
        ));
    }

    Ok(())
}

fn recording_schema_error(path: &Path, serde_path: &str, message: &str) -> FixtureLoadError {
    let field = missing_field(message)
        .map(|missing| {
            if serde_path == "." {
                missing
            } else {
                format!("{serde_path}.{missing}")
            }
        })
        .unwrap_or_else(|| {
            if serde_path == "." {
                "$".to_owned()
            } else {
                serde_path.to_owned()
            }
        });

    invalid_schema(path, field, message)
}

fn missing_field(message: &str) -> Option<String> {
    let rest = message.strip_prefix("missing field `")?;
    let (field, _) = rest.split_once('`')?;
    Some(field.to_owned())
}

fn required<T>(
    path: &Path,
    prefix: &str,
    field: &str,
    value: Option<T>,
) -> Result<T, FixtureLoadError> {
    value.ok_or_else(|| invalid_schema(path, format!("{prefix}.{field}"), "missing field"))
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
    use super::{
        FixtureAssertionFailure, FixtureLoadError, FixturePack, FixtureRunError, FixtureRunner,
        M1_MAX_FIXTURE_INPUT_BYTES, M1_MAX_FIXTURE_VIEWPORT_CELLS, M1_MAX_SNAPSHOT_BYTES,
        M2_MAX_PTY_RECORDING_FILE_BYTES, M3DogfoodMetricsReport, M3DogfoodRecording,
        M3DogfoodRedactionStatus, M3DogfoodReplayError, M3MetricMeasurement, PtyRecording,
        PtyRecordingEvent, PtyRecordingReplayError, SnapshotCodecError, SnapshotCursor,
        SnapshotScreen, deserialize_snapshot, first_snapshot_difference, serialize_snapshot,
    };
    use std::fs;
    use std::path::PathBuf;

    fn fixture_pack_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/m1-golden.json")
    }

    fn pty_recording_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/pty")
            .join(name)
    }

    fn m3_dogfood_recording_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/m3-dogfood")
            .join(name)
    }

    fn m3_dogfood_metrics_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/m3-dogfood")
            .join(name)
    }

    #[test]
    fn golden_pack_runs_all_m1_fixtures() {
        let pack = FixturePack::from_path(fixture_pack_path()).expect("fixture pack must load");
        let reports = FixtureRunner
            .run_pack(&pack)
            .expect("golden fixtures must pass");

        assert_eq!(reports.len(), 15);
        assert!(
            reports
                .iter()
                .all(|report| !report.snapshot_bytes().is_empty())
        );
    }

    #[test]
    fn malformed_fixture_schema_reports_path_and_field() {
        let path =
            std::env::temp_dir().join(format!("hera-invalid-fixture-{}.json", std::process::id()));
        fs::write(
            &path,
            r#"{"fixtures":[{"name":"bad","chunks":[{"bytes":[65]}]}]}"#,
        )
        .expect("temp fixture write should succeed");

        let error = FixturePack::from_path(&path).expect_err("schema must fail");
        let _ = fs::remove_file(&path);

        assert!(matches!(
            error,
            FixtureLoadError::InvalidSchema { field, .. } if field == "fixtures[0].expected"
        ));
    }

    #[test]
    fn malformed_toml_fixture_schema_reports_path_and_field() {
        let path =
            std::env::temp_dir().join(format!("hera-invalid-fixture-{}.toml", std::process::id()));
        fs::write(
            &path,
            r#"
                [[fixtures]]
                name = "bad"

                [[fixtures.chunks]]
                bytes = [65]
            "#,
        )
        .expect("temp TOML fixture write should succeed");

        let error = FixturePack::from_path(&path).expect_err("TOML schema must fail");
        let _ = fs::remove_file(&path);

        assert!(matches!(
            error,
            FixtureLoadError::InvalidSchema { field, .. } if field == "fixtures[0].expected"
        ));
    }

    #[test]
    fn wrong_expected_snapshot_identifies_first_differing_cell() {
        let json = r#"{
          "fixtures": [{
            "name": "bad-snapshot",
            "terminal": { "columns": 2, "rows": 1 },
            "chunks": [{ "bytes": [97, 98] }],
            "expected": { "viewport_lines": ["ax"] }
          }]
        }"#;
        let pack = FixturePack::from_json_str("inline.json", json).expect("schema should load");
        let error = FixtureRunner
            .run_pack(&pack)
            .expect_err("snapshot mismatch should fail");

        assert!(matches!(
            error,
            FixtureRunError::Assertion(FixtureAssertionFailure { .. })
        ));
        assert!(error.to_string().contains("viewport_rows[0].cells[1].ch"));
    }

    #[test]
    fn oversized_repeat_fixture_is_rejected_before_replay() {
        let json = format!(
            r#"{{
              "fixtures": [{{
                "name": "too-large",
                "chunks": [{{ "repeat": {{ "byte": 120, "count": {} }} }}],
                "expected": {{ "viewport_lines": [""] }}
              }}]
            }}"#,
            M1_MAX_FIXTURE_INPUT_BYTES + 1
        );

        let error = FixturePack::from_json_str("inline.json", &json).expect_err("schema must fail");

        assert!(matches!(
            error,
            FixtureLoadError::InvalidSchema { field, .. } if field == "fixtures[0].chunks"
        ));
    }

    #[test]
    fn cumulative_repeat_fixture_is_rejected_before_allocation() {
        let count = (M1_MAX_FIXTURE_INPUT_BYTES / 2) + 1;
        let json = format!(
            r#"{{
              "fixtures": [{{
                "name": "too-large-cumulative",
                "chunks": [
                  {{ "repeat": {{ "byte": 120, "count": {count} }} }},
                  {{ "repeat": {{ "byte": 121, "count": {count} }} }}
                ],
                "expected": {{ "viewport_lines": [""] }}
              }}]
            }}"#
        );

        let error = FixturePack::from_json_str("inline.json", &json).expect_err("schema must fail");

        assert!(matches!(
            error,
            FixtureLoadError::InvalidSchema { field, .. } if field == "fixtures[0].chunks"
        ));
    }

    #[test]
    fn oversized_fixture_viewport_is_rejected_before_replay() {
        let columns = 1024;
        let rows = (M1_MAX_FIXTURE_VIEWPORT_CELLS / columns) + 1;
        let json = format!(
            r#"{{
              "fixtures": [{{
                "name": "too-large-viewport",
                "terminal": {{ "columns": {columns}, "rows": {rows} }},
                "chunks": [{{ "bytes": [120] }}],
                "expected": {{ "viewport_lines": ["x"] }}
              }}]
            }}"#
        );

        let error = FixturePack::from_json_str("inline.json", &json).expect_err("schema must fail");

        assert!(matches!(
            error,
            FixtureLoadError::InvalidSchema { field, .. } if field == "fixtures[0].terminal"
        ));
    }

    #[test]
    fn snapshot_roundtrip_preserves_visual_state_and_defaults_semantics() {
        let pack = FixturePack::from_path(fixture_pack_path()).expect("fixture pack must load");
        let report = FixtureRunner
            .run_fixture(&pack.fixtures()[0])
            .expect("fixture should pass");
        let encoded = serialize_snapshot(report.snapshot()).expect("snapshot should serialize");
        let decoded = deserialize_snapshot(&encoded).expect("snapshot should deserialize");

        assert_eq!(decoded, *report.snapshot());

        let without_semantics = br#"{
          "columns":2,
          "rows":1,
          "active_screen":"primary",
          "cursor":{"row":0,"column":1,"visible":true},
            "viewport_rows":[{
            "handle":{"id":1,"generation":0},
            "cells":[{
              "ch":"a",
              "width":1,
              "style":{"foreground":null,"background":null,"bold":false,"italic":false,"underline":false,"inverse":false},
              "image":null
            },{
              "ch":" ",
              "width":1,
              "style":{"foreground":null,"background":null,"bold":false,"italic":false,"underline":false,"inverse":false},
              "image":null
            }],
            "wrapped":false
          }],
          "scrollback_rows":[],
          "damage":[],
          "events":[]
        }"#;
        let decoded =
            deserialize_snapshot(without_semantics).expect("semantic fields should be optional");
        assert_eq!(decoded.columns(), 2);
        assert_eq!(decoded.active_screen(), SnapshotScreen::Primary);
        assert_eq!(decoded.cursor(), SnapshotCursor::new(0, 1, true));
    }

    #[test]
    fn incomplete_snapshot_grid_returns_typed_error() {
        let incomplete = br#"{
          "columns":2,
          "rows":1,
          "active_screen":"primary",
          "cursor":{"row":0,"column":1,"visible":true},
          "viewport_rows":[{
            "handle":{"id":1,"generation":0},
            "cells":[{
              "ch":"a",
              "width":1,
              "style":{"foreground":null,"background":null,"bold":false,"italic":false,"underline":false,"inverse":false},
              "image":null
            }],
            "wrapped":false
          }],
          "scrollback_rows":[],
          "damage":[],
          "events":[]
        }"#;
        let error = deserialize_snapshot(incomplete).expect_err("incomplete grid must fail");

        assert!(matches!(error, SnapshotCodecError::Deserialize { .. }));
    }

    #[test]
    fn semantic_snapshot_difference_ignores_damage() {
        let pack = FixturePack::from_path(fixture_pack_path()).expect("fixture pack must load");
        let report = FixtureRunner
            .run_fixture(&pack.fixtures()[0])
            .expect("fixture should pass");
        let mut without_damage = report.snapshot().clone();
        without_damage.damage.clear();

        assert!(first_snapshot_difference(report.snapshot(), &without_damage).is_none());
    }

    #[test]
    fn corrupted_snapshot_input_returns_typed_error() {
        let error = deserialize_snapshot(br#"{"columns":"bad"}"#)
            .expect_err("corrupted snapshot must fail");

        assert!(matches!(error, SnapshotCodecError::Deserialize { .. }));
    }

    #[test]
    fn oversized_snapshot_input_returns_typed_error() {
        let bytes = vec![b' '; M1_MAX_SNAPSHOT_BYTES + 1];
        let error = deserialize_snapshot(&bytes).expect_err("oversized snapshot must fail");

        assert!(matches!(error, SnapshotCodecError::Deserialize { .. }));
    }

    #[test]
    fn pty_recording_pack_replays_checked_in_recordings() {
        for recording_name in [
            "plain-output.json",
            "non-zero-exit.json",
            "resize-aware.json",
        ] {
            let recording = PtyRecording::from_path(pty_recording_path(recording_name))
                .expect("PTY recording fixture should load");
            let report = recording.replay().expect("PTY recording should replay");

            assert!(!report.snapshot_bytes().is_empty());
            assert_eq!(
                report.snapshot().columns(),
                recording.final_snapshot.columns
            );
        }
    }

    #[test]
    fn m3_dogfood_recording_replays_checked_in_synthetic_fixture() {
        let recording =
            M3DogfoodRecording::from_path(m3_dogfood_recording_path("synthetic-shadow.json"))
                .expect("M3 dogfood fixture should load");

        assert_eq!(recording.metadata.source, "paneflow-synthetic");
        assert_eq!(
            recording.metadata.redaction_status,
            M3DogfoodRedactionStatus::Scrubbed
        );
        assert_eq!(recording.metadata.event_counts.output, 1);
        assert_eq!(recording.metadata.event_counts.input, 1);
        assert_eq!(recording.metadata.event_counts.resize, 1);
        assert_eq!(recording.metadata.event_counts.lifecycle, 1);

        let report = recording
            .replay()
            .expect("M3 dogfood fixture should replay");
        assert!(!report.snapshot_bytes().is_empty());
        assert_eq!(report.snapshot().columns(), 6);
        assert_eq!(report.snapshot().rows(), 2);
    }

    #[test]
    fn m3_dogfood_wrong_snapshot_identifies_first_differing_field() {
        let mut recording =
            M3DogfoodRecording::from_path(m3_dogfood_recording_path("synthetic-shadow.json"))
                .expect("M3 dogfood fixture should load");
        recording.final_snapshot.viewport_lines[1] = "foox  ".to_owned();

        let error = recording
            .replay()
            .expect_err("wrong M3 final snapshot should fail");

        assert!(matches!(
            error,
            M3DogfoodReplayError::SnapshotMismatch(difference)
                if difference.field() == "$.viewport_lines[1]"
        ));
    }

    #[test]
    fn m3_dogfood_long_session_metrics_validate_scrubbed_derivatives() {
        for metrics_name in [
            "codex-long-session-summary.json",
            "claude-code-long-session-summary.json",
        ] {
            let report = M3DogfoodMetricsReport::from_path(m3_dogfood_metrics_path(metrics_name))
                .expect("M3 dogfood metrics fixture should load");

            assert_eq!(report.redaction_status, M3DogfoodRedactionStatus::Scrubbed);
            assert!(report.session.logical_output_lines >= 10_000);
            assert!(report.decision.replacement_blocked);
            assert!(!report.decision.blocked_reasons.is_empty());
            assert!(matches!(
                report.memory.dogfood_rss_delta_bytes,
                M3MetricMeasurement::NotMeasured { .. }
            ));
            assert!(matches!(
                report.latency.pty_batch_p95_ms,
                M3MetricMeasurement::NotMeasured { .. }
            ));
        }
    }

    #[test]
    fn pty_recording_schema_reports_missing_field_path() {
        let json = r#"{
          "schema": "hera.pty_recording",
          "version": 1,
          "metadata": {
            "command": { "mode": "direct", "program": "fixture", "args": [], "shell_command": null },
            "platform": { "pty_backend": "recorded", "process_id": null, "initial_size": { "columns": 4, "rows": 1 } },
            "exit": { "code": 0, "signal": null, "success": true },
            "runtime": { "output_bytes": 2, "output_chunks": 1, "saw_eof": true, "drain_timed_out": false, "timeout_ms": 5000 },
            "initial_size": { "columns": 4, "rows": 1 },
            "input": { "path": null, "bytes": 0, "max_bytes": 65536, "stdin_closed_after_input": false },
            "resizes": [],
            "recording": { "enabled": true, "output_bytes": 2, "output_truncated": false, "max_output_bytes": 4194304 }
          },
          "events": [
            { "kind": "output", "elapsed_ms": 1, "bytes": [111, 107] },
            { "kind": "exit", "elapsed_ms": 2, "exit": { "code": 0, "signal": null, "success": true } }
          ]
        }"#;

        let error = PtyRecording::from_json_str("inline-recording.json", json)
            .expect_err("missing final snapshot must fail");

        assert!(matches!(
            error,
            FixtureLoadError::InvalidSchema { field, .. } if field == "final_snapshot"
        ));
    }

    #[test]
    fn pty_recording_rejects_oversized_file_before_schema_parse() {
        let json = " ".repeat(M2_MAX_PTY_RECORDING_FILE_BYTES as usize + 1);
        let error = PtyRecording::from_json_str("too-large-recording.json", &json)
            .expect_err("oversized recording must fail before deserialization");

        assert!(matches!(
            error,
            FixtureLoadError::InvalidSchema { field, .. } if field == "$"
        ));
    }

    #[test]
    fn pty_recording_timestamps_do_not_affect_replayed_snapshot() {
        let recording = PtyRecording::from_path(pty_recording_path("plain-output.json"))
            .expect("recording should load");
        let baseline = recording
            .replay()
            .expect("baseline recording should replay")
            .snapshot_bytes()
            .to_vec();
        let mut changed = recording.clone();

        for event in &mut changed.events {
            match event {
                PtyRecordingEvent::Output { elapsed_ms, .. }
                | PtyRecordingEvent::Input { elapsed_ms, .. }
                | PtyRecordingEvent::Resize { elapsed_ms, .. }
                | PtyRecordingEvent::Eof { elapsed_ms }
                | PtyRecordingEvent::Exit { elapsed_ms, .. } => {
                    *elapsed_ms = elapsed_ms.saturating_add(10_000);
                }
            }
        }

        let changed = changed
            .replay()
            .expect("timestamp-only change should replay")
            .snapshot_bytes()
            .to_vec();

        assert_eq!(baseline, changed);
    }

    #[test]
    fn pty_recording_wrong_snapshot_identifies_first_differing_field() {
        let mut recording = PtyRecording::from_path(pty_recording_path("plain-output.json"))
            .expect("recording should load");
        recording.final_snapshot.viewport_rows[0].cells[1].ch = 'x';

        let error = recording
            .replay()
            .expect_err("wrong final snapshot should fail");

        assert!(matches!(
            error,
            PtyRecordingReplayError::SnapshotMismatch(difference)
                if difference.field() == "$.viewport_rows[0].cells[1].ch"
        ));
    }
}
