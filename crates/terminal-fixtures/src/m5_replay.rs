use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use terminal_core::{ScrollbackConfig, Terminal, TerminalConfig};

use super::{
    FixtureLoadError, SnapshotCodecError, invalid_schema, recording_schema_error,
    serialize_snapshot, snapshot_terminal,
};

pub const M5_PUBLIC_REPLAY_SCHEMA: &str = "hera.m5_public_replay";
pub const M5_PUBLIC_REPLAY_VERSION: u32 = 1;
pub const M5_REPLAY_VERIFICATION_SCHEMA: &str = "hera.m5_replay_verification";
pub const M5_REPLAY_VERIFICATION_VERSION: u32 = 1;
pub const M5_MAX_PUBLIC_REPLAY_FILE_BYTES: u64 = 1024 * 1024;
pub const M5_MAX_PUBLIC_REPLAY_EVENTS: usize = 128;
pub const M5_MAX_PUBLIC_REPLAY_EVENT_BYTES: usize = 128 * 1024;
pub const M5_MAX_PUBLIC_REPLAY_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
pub const M5_MAX_GENERATED_OUTPUT_LINES: usize = 1_000_000;
pub const M5_DEFAULT_REPLAY_TIMEOUT_MS: u128 = 5_000;
pub const M5_DEFAULT_MEMORY_BUDGET_BYTES: usize = 8 * 1024 * 1024;

const DEFAULT_REJECT_PATTERNS: &[&str] = &[
    "C:\\Users\\",
    "C:/Users/",
    "\\Users\\",
    "%USERPROFILE%",
    "/home/",
    "OPENAI_API_KEY",
    "ANTHROPIC_API_KEY",
    "BEGIN OPENSSH PRIVATE KEY",
    "BEGIN RSA PRIVATE KEY",
];

const TOKEN_LIKE_PATTERNS: &[&str] = &["sk-", "ghp_", "github_pat_", "xoxb-", "AKIA", "AIza"];

const RAW_TRANSCRIPT_FIELDS: &[&str] = &[
    "raw_transcript",
    "raw_bytes",
    "terminal_bytes",
    "prompt_text",
    "raw_input",
    "private_prompt",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5PublicReplayFixture {
    pub schema: String,
    pub version: u32,
    pub id: String,
    pub title: String,
    pub agent: M5ReplayAgent,
    pub source: M5ReplaySource,
    pub privacy: M5ReplayPrivacy,
    pub generated_at: String,
    pub redaction: M5ReplayRedaction,
    pub initial_size: M5ReplaySize,
    #[serde(default)]
    pub scrollback: Option<M5ReplayScrollback>,
    pub replay_policy: M5ReplayPolicy,
    pub events: Vec<M5ReplayEvent>,
    #[serde(default)]
    pub notes: Vec<String>,
}

impl M5PublicReplayFixture {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref();
        let raw = read_public_replay_file_capped(path)?;
        Self::from_json_str(path, &raw)
    }

    pub fn from_json_str(path: impl AsRef<Path>, json: &str) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref().to_path_buf();
        if json.len() as u64 > M5_MAX_PUBLIC_REPLAY_FILE_BYTES {
            return Err(invalid_schema(
                &path,
                "$",
                format!(
                    "M5 public replay JSON is {} bytes, maximum is {M5_MAX_PUBLIC_REPLAY_FILE_BYTES}",
                    json.len()
                ),
            ));
        }
        scan_raw_field_names(&path, json)?;

        let mut deserializer = serde_json::Deserializer::from_str(json);
        let fixture: Self =
            serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
                recording_schema_error(&path, &error.path().to_string(), &error.inner().to_string())
            })?;
        fixture.validate(&path)?;
        Ok(fixture)
    }

    #[must_use]
    pub fn filename(&self) -> String {
        format!("{}.json", self.id)
    }

    pub fn verify(&self) -> Result<M5ReplayVerificationReport, M5ReplayVerificationError> {
        let first = self.replay_once()?;
        let second = self.replay_once()?;
        if first.snapshot_bytes != second.snapshot_bytes {
            return Err(M5ReplayVerificationError::NondeterministicSnapshot {
                fixture_id: self.id.clone(),
                first_hash: stable_snapshot_hash(&first.snapshot_bytes),
                second_hash: stable_snapshot_hash(&second.snapshot_bytes),
            });
        }

        let mut status = M5ReplayVerificationStatus::Pass;
        let mut report_notes = self.notes.clone();
        if first.elapsed_ms > self.replay_policy.timeout_ms {
            status = M5ReplayVerificationStatus::Fail;
            report_notes.push(format!(
                "replay exceeded timeout: observed {} ms, budget {} ms",
                first.elapsed_ms, self.replay_policy.timeout_ms
            ));
        }
        if first.scrollback_byte_len > self.replay_policy.memory_budget_bytes {
            status = M5ReplayVerificationStatus::Fail;
            report_notes.push(format!(
                "replay exceeded memory budget: observed {} bytes, budget {} bytes",
                first.scrollback_byte_len, self.replay_policy.memory_budget_bytes
            ));
        }
        if let Some(min_lines) = self.replay_policy.expected_min_logical_lines {
            if first.logical_lines_processed < min_lines {
                status = M5ReplayVerificationStatus::Fail;
                report_notes.push(format!(
                    "replay processed {} logical lines, expected at least {min_lines}",
                    first.logical_lines_processed
                ));
            }
        }
        if self.replay_policy.require_discarded_rows && first.discarded_rows == 0 {
            status = M5ReplayVerificationStatus::Fail;
            report_notes.push("long-session replay did not record discarded rows".to_owned());
        }

        Ok(M5ReplayVerificationReport {
            fixture_id: self.id.clone(),
            title: self.title.clone(),
            agent: self.agent,
            status,
            event_count: self.events.len(),
            event_counts: self.event_counts(),
            logical_lines_processed: first.logical_lines_processed,
            output_bytes: first.output_bytes,
            elapsed_ms: first.elapsed_ms,
            timeout_budget_ms: self.replay_policy.timeout_ms,
            memory_budget_bytes: self.replay_policy.memory_budget_bytes,
            scrollback_rows: first.scrollback_rows,
            scrollback_byte_len: first.scrollback_byte_len,
            discarded_rows: first.discarded_rows,
            final_snapshot_hash: stable_snapshot_hash(&first.snapshot_bytes),
            final_snapshot_bytes: first.snapshot_bytes.len(),
            notes: report_notes,
        })
    }

    fn replay_once(&self) -> Result<M5ReplayOnce, M5ReplayVerificationError> {
        let config = if let Some(scrollback) = self.scrollback {
            TerminalConfig::with_scrollback(
                self.initial_size.columns,
                self.initial_size.rows,
                ScrollbackConfig::new(scrollback.max_lines, scrollback.max_bytes),
            )
        } else {
            TerminalConfig::new(self.initial_size.columns, self.initial_size.rows)
        }
        .map_err(|error| M5ReplayVerificationError::TerminalConfig {
            fixture_id: self.id.clone(),
            message: error.to_string(),
        })?;

        let mut terminal = Terminal::with_config(config);
        let mut output_bytes = 0usize;
        let mut logical_lines_processed = 0usize;
        let started = Instant::now();

        for event in &self.events {
            match event.kind.as_str() {
                "output" => {
                    let data = event.data.as_deref().unwrap_or_default();
                    output_bytes = output_bytes.saturating_add(data.len());
                    logical_lines_processed =
                        logical_lines_processed.saturating_add(count_logical_lines(data));
                    terminal.advance_bytes(data.as_bytes());
                }
                "generated_output" => {
                    let prefix = event.generated_line_prefix.as_deref().ok_or_else(|| {
                        M5ReplayVerificationError::UnsupportedEvent {
                            fixture_id: self.id.clone(),
                            kind: "generated_output_without_prefix".to_owned(),
                        }
                    })?;
                    let count = event.generated_line_count.ok_or_else(|| {
                        M5ReplayVerificationError::UnsupportedEvent {
                            fixture_id: self.id.clone(),
                            kind: "generated_output_without_count".to_owned(),
                        }
                    })?;
                    for line in 0..count {
                        let data = format!("{prefix}{line:06}\r\n");
                        output_bytes = output_bytes.saturating_add(data.len());
                        logical_lines_processed = logical_lines_processed.saturating_add(1);
                        terminal.advance_bytes(data.as_bytes());
                    }
                }
                "resize" => {
                    let columns = event.columns.unwrap_or(self.initial_size.columns);
                    let rows = event.rows.unwrap_or(self.initial_size.rows);
                    terminal.resize(columns, rows).map_err(|error| {
                        M5ReplayVerificationError::TerminalConfig {
                            fixture_id: self.id.clone(),
                            message: error.to_string(),
                        }
                    })?;
                }
                "marker" => {}
                other => {
                    return Err(M5ReplayVerificationError::UnsupportedEvent {
                        fixture_id: self.id.clone(),
                        kind: other.to_owned(),
                    });
                }
            }
        }

        let elapsed_ms = started.elapsed().as_millis();
        let scrollback_rows = terminal.scrollback_len();
        let scrollback_byte_len = terminal.scrollback_byte_len();
        let visible_rows = terminal.dimensions().rows();
        let discarded_rows =
            logical_lines_processed.saturating_sub(scrollback_rows.saturating_add(visible_rows));
        let snapshot = snapshot_terminal(&mut terminal);
        let snapshot_bytes = serialize_snapshot(&snapshot)?;

        Ok(M5ReplayOnce {
            snapshot_bytes,
            logical_lines_processed,
            output_bytes,
            elapsed_ms,
            scrollback_rows,
            scrollback_byte_len,
            discarded_rows,
        })
    }

    fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        if self.schema != M5_PUBLIC_REPLAY_SCHEMA {
            return Err(invalid_schema(
                path,
                "schema",
                format!("expected {M5_PUBLIC_REPLAY_SCHEMA}"),
            ));
        }
        if self.version != M5_PUBLIC_REPLAY_VERSION {
            return Err(invalid_schema(
                path,
                "version",
                format!("expected {M5_PUBLIC_REPLAY_VERSION}"),
            ));
        }
        validate_required_string(path, "id", &self.id)?;
        validate_required_string(path, "title", &self.title)?;
        validate_timestamp(path, "generated_at", &self.generated_at)?;
        self.redaction.validate(path)?;
        self.initial_size.validate(path, "initial_size")?;
        if let Some(scrollback) = self.scrollback {
            scrollback.validate(path, "scrollback")?;
        }
        self.replay_policy.validate(path, "replay_policy")?;
        if !self.privacy.is_public() {
            return Err(invalid_schema(
                path,
                "privacy",
                "M5 public replay fixtures must be scrubbed_public or synthetic_public",
            ));
        }
        match self.agent {
            M5ReplayAgent::Codex | M5ReplayAgent::ClaudeCode => {
                if self.source != M5ReplaySource::ScrubbedCaptureDerivative
                    || self.privacy != M5ReplayPrivacy::ScrubbedPublic
                {
                    return Err(invalid_schema(
                        path,
                        "source",
                        "agent session derivatives must be scrubbed capture derivatives",
                    ));
                }
            }
            M5ReplayAgent::RapidOutput | M5ReplayAgent::LongSession => {}
        }

        if self.events.is_empty() {
            return Err(invalid_schema(
                path,
                "events",
                "at least one event is required",
            ));
        }
        if self.events.len() > M5_MAX_PUBLIC_REPLAY_EVENTS {
            return Err(invalid_schema(
                path,
                "events",
                format!(
                    "replay has {} events, maximum is {M5_MAX_PUBLIC_REPLAY_EVENTS}",
                    self.events.len()
                ),
            ));
        }

        let mut output_bytes = 0usize;
        let mut logical_lines = 0usize;
        let mut saw_output = false;
        for (index, event) in self.events.iter().enumerate() {
            let field = format!("events[{index}]");
            event.validate(path, &field)?;
            match event.kind.as_str() {
                "output" => {
                    saw_output = true;
                    let data = event.data.as_deref().unwrap_or_default();
                    output_bytes = output_bytes.saturating_add(data.len());
                    logical_lines = logical_lines.saturating_add(count_logical_lines(data));
                }
                "generated_output" => {
                    saw_output = true;
                    let prefix = event.generated_line_prefix.as_deref().unwrap_or_default();
                    let count = event.generated_line_count.unwrap_or_default();
                    output_bytes =
                        output_bytes.saturating_add(prefix.len().saturating_add(8) * count);
                    logical_lines = logical_lines.saturating_add(count);
                }
                _ => {}
            }
        }
        if !saw_output {
            return Err(invalid_schema(
                path,
                "events",
                "at least one output event is required for replay proof",
            ));
        }
        if output_bytes > M5_MAX_PUBLIC_REPLAY_OUTPUT_BYTES {
            return Err(invalid_schema(
                path,
                "events",
                format!(
                    "replay output is {output_bytes} bytes, maximum is {M5_MAX_PUBLIC_REPLAY_OUTPUT_BYTES}"
                ),
            ));
        }
        if self.agent == M5ReplayAgent::LongSession && logical_lines < 100_000 {
            return Err(invalid_schema(
                path,
                "events",
                "long-session replay must declare at least 100000 logical lines",
            ));
        }

        self.scan_public_content(path)
    }

    fn scan_public_content(&self, path: &Path) -> Result<(), FixtureLoadError> {
        let mut patterns = DEFAULT_REJECT_PATTERNS
            .iter()
            .chain(TOKEN_LIKE_PATTERNS.iter())
            .map(|pattern| (*pattern).to_owned())
            .collect::<Vec<_>>();
        patterns.extend(self.redaction.reject_patterns.iter().cloned());

        let mut values = vec![
            ("id".to_owned(), self.id.as_str()),
            ("title".to_owned(), self.title.as_str()),
        ];
        for (index, note) in self.notes.iter().enumerate() {
            values.push((format!("notes[{index}]"), note.as_str()));
        }
        for (index, event) in self.events.iter().enumerate() {
            if let Some(data) = event.data.as_deref() {
                values.push((format!("events[{index}].data"), data));
            }
            if let Some(prefix) = event.generated_line_prefix.as_deref() {
                values.push((format!("events[{index}].generated_line_prefix"), prefix));
            }
            if let Some(label) = event.label.as_deref() {
                values.push((format!("events[{index}].label"), label));
            }
            for (key, value) in &event.metadata {
                values.push((format!("events[{index}].metadata.{key}"), value.as_str()));
            }
        }

        for (field, value) in values {
            let value_lower = value.to_ascii_lowercase();
            for pattern in &patterns {
                let pattern_lower = pattern.to_ascii_lowercase();
                if value.contains(pattern) || value_lower.contains(&pattern_lower) {
                    return Err(invalid_schema(
                        path,
                        field,
                        format!("public replay contains rejected redaction pattern {pattern:?}"),
                    ));
                }
            }
        }

        Ok(())
    }

    fn event_counts(&self) -> M5ReplayEventCounts {
        let mut counts = M5ReplayEventCounts::default();
        for event in &self.events {
            match event.kind.as_str() {
                "output" => counts.output += 1,
                "generated_output" => counts.generated_output += 1,
                "resize" => counts.resize += 1,
                "marker" => counts.marker += 1,
                _ => counts.unsupported += 1,
            }
        }
        counts
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5ReplayAgent {
    Codex,
    ClaudeCode,
    RapidOutput,
    LongSession,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5ReplaySource {
    ScrubbedCaptureDerivative,
    SyntheticStress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5ReplayPrivacy {
    ScrubbedPublic,
    SyntheticPublic,
    RawLocal,
}

impl M5ReplayPrivacy {
    const fn is_public(self) -> bool {
        matches!(self, Self::ScrubbedPublic | Self::SyntheticPublic)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5ReplayRedaction {
    pub status: M5ReplayRedactionStatus,
    pub policy_version: u32,
    pub checked_at: String,
    #[serde(default)]
    pub reject_patterns: Vec<String>,
    #[serde(default)]
    pub normalized_fields: Vec<String>,
}

impl M5ReplayRedaction {
    fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        if self.status != M5ReplayRedactionStatus::Passed {
            return Err(invalid_schema(
                path,
                "redaction.status",
                "public replay redaction must be passed",
            ));
        }
        if self.policy_version == 0 {
            return Err(invalid_schema(
                path,
                "redaction.policy_version",
                "redaction policy version must be greater than zero",
            ));
        }
        validate_timestamp(path, "redaction.checked_at", &self.checked_at)?;
        for (index, pattern) in self.reject_patterns.iter().enumerate() {
            validate_required_string(path, format!("redaction.reject_patterns[{index}]"), pattern)?;
        }
        for (index, field) in self.normalized_fields.iter().enumerate() {
            validate_required_string(path, format!("redaction.normalized_fields[{index}]"), field)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5ReplayRedactionStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5ReplaySize {
    pub columns: usize,
    pub rows: usize,
}

impl M5ReplaySize {
    fn validate(&self, path: &Path, field: impl Into<String>) -> Result<(), FixtureLoadError> {
        let field = field.into();
        TerminalConfig::new(self.columns, self.rows)
            .map(|_| ())
            .map_err(|error| invalid_schema(path, field, error.to_string()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5ReplayScrollback {
    pub max_lines: usize,
    pub max_bytes: usize,
}

impl M5ReplayScrollback {
    fn validate(&self, path: &Path, field: &str) -> Result<(), FixtureLoadError> {
        if self.max_lines == 0 {
            return Err(invalid_schema(
                path,
                format!("{field}.max_lines"),
                "scrollback max_lines must be non-zero",
            ));
        }
        if self.max_bytes == 0 {
            return Err(invalid_schema(
                path,
                format!("{field}.max_bytes"),
                "scrollback max_bytes must be non-zero",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5ReplayPolicy {
    pub timeout_ms: u128,
    pub memory_budget_bytes: usize,
    #[serde(default)]
    pub expected_min_logical_lines: Option<usize>,
    #[serde(default)]
    pub require_discarded_rows: bool,
}

impl M5ReplayPolicy {
    fn validate(&self, path: &Path, field: &str) -> Result<(), FixtureLoadError> {
        if self.timeout_ms == 0 {
            return Err(invalid_schema(
                path,
                format!("{field}.timeout_ms"),
                "timeout must be non-zero",
            ));
        }
        if self.memory_budget_bytes == 0 {
            return Err(invalid_schema(
                path,
                format!("{field}.memory_budget_bytes"),
                "memory budget must be non-zero",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5ReplayEvent {
    pub kind: String,
    #[serde(default)]
    pub elapsed_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_line_prefix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_line_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

impl M5ReplayEvent {
    fn validate(&self, path: &Path, field: &str) -> Result<(), FixtureLoadError> {
        validate_required_string(path, format!("{field}.kind"), &self.kind)?;
        match self.kind.as_str() {
            "output" => {
                let data = self.data.as_deref().ok_or_else(|| {
                    invalid_schema(path, format!("{field}.data"), "output event requires data")
                })?;
                if data.is_empty() {
                    return Err(invalid_schema(
                        path,
                        format!("{field}.data"),
                        "output data must not be empty",
                    ));
                }
                if data.len() > M5_MAX_PUBLIC_REPLAY_EVENT_BYTES {
                    return Err(invalid_schema(
                        path,
                        format!("{field}.data"),
                        format!(
                            "output event is {} bytes, maximum is {M5_MAX_PUBLIC_REPLAY_EVENT_BYTES}",
                            data.len()
                        ),
                    ));
                }
            }
            "generated_output" => {
                let prefix = self.generated_line_prefix.as_deref().ok_or_else(|| {
                    invalid_schema(
                        path,
                        format!("{field}.generated_line_prefix"),
                        "generated output requires a line prefix",
                    )
                })?;
                validate_required_string(path, format!("{field}.generated_line_prefix"), prefix)?;
                let count = self.generated_line_count.ok_or_else(|| {
                    invalid_schema(
                        path,
                        format!("{field}.generated_line_count"),
                        "generated output requires a line count",
                    )
                })?;
                if count == 0 || count > M5_MAX_GENERATED_OUTPUT_LINES {
                    return Err(invalid_schema(
                        path,
                        format!("{field}.generated_line_count"),
                        format!("generated line count must be 1..={M5_MAX_GENERATED_OUTPUT_LINES}"),
                    ));
                }
            }
            "resize" => {
                let columns = self.columns.ok_or_else(|| {
                    invalid_schema(path, format!("{field}.columns"), "resize requires columns")
                })?;
                let rows = self.rows.ok_or_else(|| {
                    invalid_schema(path, format!("{field}.rows"), "resize requires rows")
                })?;
                M5ReplaySize { columns, rows }.validate(path, field)?;
            }
            "marker" => {
                if let Some(label) = self.label.as_deref() {
                    validate_required_string(path, format!("{field}.label"), label)?;
                }
            }
            other => {
                return Err(invalid_schema(
                    path,
                    format!("{field}.kind"),
                    format!("unsupported event kind {other:?}"),
                ));
            }
        }

        for (key, value) in &self.metadata {
            validate_required_string(path, format!("{field}.metadata.{key}"), value)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct M5ReplayOnce {
    snapshot_bytes: Vec<u8>,
    logical_lines_processed: usize,
    output_bytes: usize,
    elapsed_ms: u128,
    scrollback_rows: usize,
    scrollback_byte_len: usize,
    discarded_rows: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5ReplayVerificationStatus {
    Pass,
    Fail,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5ReplayEventCounts {
    pub output: usize,
    pub generated_output: usize,
    pub resize: usize,
    pub marker: usize,
    pub unsupported: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5ReplayVerificationReport {
    pub fixture_id: String,
    pub title: String,
    pub agent: M5ReplayAgent,
    pub status: M5ReplayVerificationStatus,
    pub event_count: usize,
    pub event_counts: M5ReplayEventCounts,
    pub logical_lines_processed: usize,
    pub output_bytes: usize,
    pub elapsed_ms: u128,
    pub timeout_budget_ms: u128,
    pub memory_budget_bytes: usize,
    pub scrollback_rows: usize,
    pub scrollback_byte_len: usize,
    pub discarded_rows: usize,
    pub final_snapshot_hash: String,
    pub final_snapshot_bytes: usize,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5ReplayVerificationSummary {
    pub schema: String,
    pub version: u32,
    pub generated_at: String,
    pub command: String,
    pub status: M5ReplayVerificationStatus,
    pub fixtures: Vec<M5ReplayVerificationReport>,
}

impl M5ReplayVerificationSummary {
    #[must_use]
    pub fn from_reports(
        generated_at: String,
        command: String,
        fixtures: Vec<M5ReplayVerificationReport>,
    ) -> Self {
        let status = if fixtures
            .iter()
            .all(|fixture| fixture.status == M5ReplayVerificationStatus::Pass)
        {
            M5ReplayVerificationStatus::Pass
        } else {
            M5ReplayVerificationStatus::Fail
        };

        Self {
            schema: M5_REPLAY_VERIFICATION_SCHEMA.to_owned(),
            version: M5_REPLAY_VERIFICATION_VERSION,
            generated_at,
            command,
            status,
            fixtures,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum M5ReplayVerificationError {
    TerminalConfig {
        fixture_id: String,
        message: String,
    },
    UnsupportedEvent {
        fixture_id: String,
        kind: String,
    },
    NondeterministicSnapshot {
        fixture_id: String,
        first_hash: String,
        second_hash: String,
    },
    SnapshotCodec(SnapshotCodecError),
}

impl fmt::Display for M5ReplayVerificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TerminalConfig {
                fixture_id,
                message,
            } => write!(
                formatter,
                "M5 replay fixture {fixture_id} has invalid terminal config: {message}"
            ),
            Self::UnsupportedEvent { fixture_id, kind } => write!(
                formatter,
                "M5 replay fixture {fixture_id} contains unsupported event kind {kind:?}"
            ),
            Self::NondeterministicSnapshot {
                fixture_id,
                first_hash,
                second_hash,
            } => write!(
                formatter,
                "M5 replay fixture {fixture_id} was nondeterministic: first {first_hash}, second {second_hash}"
            ),
            Self::SnapshotCodec(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for M5ReplayVerificationError {}

impl From<SnapshotCodecError> for M5ReplayVerificationError {
    fn from(value: SnapshotCodecError) -> Self {
        Self::SnapshotCodec(value)
    }
}

#[must_use]
pub fn m5_default_replay_fixtures(generated_at: &str) -> Vec<M5PublicReplayFixture> {
    vec![
        m5_agent_fixture(
            "codex-session",
            "Scrubbed Codex CLI replay derivative",
            M5ReplayAgent::Codex,
            generated_at,
            "codex> task accepted\r\n[tool:shell] cargo test --workspace\r\nrunning 4 tests\r\ntest m5_replay_derivative ... ok\r\ncodex> final checks passed\r\n",
        ),
        m5_agent_fixture(
            "claude-code-session",
            "Scrubbed Claude Code replay derivative",
            M5ReplayAgent::ClaudeCode,
            generated_at,
            "claude-code> plan updated\r\n[edit] crates/terminal-core/src/state.rs\r\n[tool:cargo] cargo clippy --workspace --all-targets\r\nwarning: none\r\nclaude-code> ready for review\r\n",
        ),
        M5PublicReplayFixture {
            schema: M5_PUBLIC_REPLAY_SCHEMA.to_owned(),
            version: M5_PUBLIC_REPLAY_VERSION,
            id: "rapid-output".to_owned(),
            title: "Synthetic rapid-output replay coverage".to_owned(),
            agent: M5ReplayAgent::RapidOutput,
            source: M5ReplaySource::SyntheticStress,
            privacy: M5ReplayPrivacy::SyntheticPublic,
            generated_at: generated_at.to_owned(),
            redaction: passed_redaction(generated_at),
            initial_size: M5ReplaySize {
                columns: 80,
                rows: 24,
            },
            scrollback: Some(M5ReplayScrollback {
                max_lines: 10_000,
                max_bytes: 1024 * 1024,
            }),
            replay_policy: M5ReplayPolicy {
                timeout_ms: M5_DEFAULT_REPLAY_TIMEOUT_MS,
                memory_budget_bytes: M5_DEFAULT_MEMORY_BUDGET_BYTES,
                expected_min_logical_lines: Some(5_000),
                require_discarded_rows: false,
            },
            events: vec![
                M5ReplayEvent {
                    kind: "generated_output".to_owned(),
                    elapsed_ms: 0,
                    data: None,
                    generated_line_prefix: Some("rapid-frame-".to_owned()),
                    generated_line_count: Some(5_000),
                    columns: None,
                    rows: None,
                    label: None,
                    metadata: BTreeMap::new(),
                },
                marker("rapid-output-complete", 250),
            ],
            notes: vec![
                "Synthetic public rapid-output workload. It preserves volume and timing shape without private terminal content.".to_owned(),
            ],
        },
        M5PublicReplayFixture {
            schema: M5_PUBLIC_REPLAY_SCHEMA.to_owned(),
            version: M5_PUBLIC_REPLAY_VERSION,
            id: "long-session-100k".to_owned(),
            title: "Synthetic long-session replay coverage with 100k lines".to_owned(),
            agent: M5ReplayAgent::LongSession,
            source: M5ReplaySource::SyntheticStress,
            privacy: M5ReplayPrivacy::SyntheticPublic,
            generated_at: generated_at.to_owned(),
            redaction: passed_redaction(generated_at),
            initial_size: M5ReplaySize {
                columns: 80,
                rows: 24,
            },
            scrollback: Some(M5ReplayScrollback {
                max_lines: 10_000,
                max_bytes: 1024 * 1024,
            }),
            replay_policy: M5ReplayPolicy {
                timeout_ms: M5_DEFAULT_REPLAY_TIMEOUT_MS,
                memory_budget_bytes: M5_DEFAULT_MEMORY_BUDGET_BYTES,
                expected_min_logical_lines: Some(100_000),
                require_discarded_rows: true,
            },
            events: vec![
                M5ReplayEvent {
                    kind: "generated_output".to_owned(),
                    elapsed_ms: 0,
                    data: None,
                    generated_line_prefix: Some("m5-long-line-".to_owned()),
                    generated_line_count: Some(100_000),
                    columns: None,
                    rows: None,
                    label: None,
                    metadata: BTreeMap::new(),
                },
                marker("long-session-complete", 1_000),
            ],
            notes: vec![
                "Generated public workload records discarded-row counts and uses Hera-owned scrollback bytes as the memory budget metric.".to_owned(),
            ],
        },
    ]
}

fn m5_agent_fixture(
    id: &str,
    title: &str,
    agent: M5ReplayAgent,
    generated_at: &str,
    data: &str,
) -> M5PublicReplayFixture {
    M5PublicReplayFixture {
        schema: M5_PUBLIC_REPLAY_SCHEMA.to_owned(),
        version: M5_PUBLIC_REPLAY_VERSION,
        id: id.to_owned(),
        title: title.to_owned(),
        agent,
        source: M5ReplaySource::ScrubbedCaptureDerivative,
        privacy: M5ReplayPrivacy::ScrubbedPublic,
        generated_at: generated_at.to_owned(),
        redaction: passed_redaction(generated_at),
        initial_size: M5ReplaySize {
            columns: 100,
            rows: 30,
        },
        scrollback: Some(M5ReplayScrollback {
            max_lines: 10_000,
            max_bytes: M5_DEFAULT_MEMORY_BUDGET_BYTES,
        }),
        replay_policy: M5ReplayPolicy {
            timeout_ms: M5_DEFAULT_REPLAY_TIMEOUT_MS,
            memory_budget_bytes: M5_DEFAULT_MEMORY_BUDGET_BYTES,
            expected_min_logical_lines: Some(4),
            require_discarded_rows: false,
        },
        events: vec![
            M5ReplayEvent {
                kind: "output".to_owned(),
                elapsed_ms: 0,
                data: Some(data.to_owned()),
                generated_line_prefix: None,
                generated_line_count: None,
                columns: None,
                rows: None,
                label: None,
                metadata: BTreeMap::new(),
            },
            marker("agent-derivative-complete", 100),
        ],
        notes: vec![
            "Scrubbed derivative: command shape, tool labels and terminal volume are preserved; raw prompts, local paths and tokens are omitted.".to_owned(),
        ],
    }
}

fn passed_redaction(generated_at: &str) -> M5ReplayRedaction {
    M5ReplayRedaction {
        status: M5ReplayRedactionStatus::Passed,
        policy_version: 1,
        checked_at: generated_at.to_owned(),
        reject_patterns: Vec::new(),
        normalized_fields: vec![
            "elapsed_ms".to_owned(),
            "prompts".to_owned(),
            "local_paths".to_owned(),
            "tokens".to_owned(),
        ],
    }
}

fn marker(label: &str, elapsed_ms: u64) -> M5ReplayEvent {
    M5ReplayEvent {
        kind: "marker".to_owned(),
        elapsed_ms,
        data: None,
        generated_line_prefix: None,
        generated_line_count: None,
        columns: None,
        rows: None,
        label: Some(label.to_owned()),
        metadata: BTreeMap::new(),
    }
}

fn stable_snapshot_hash(bytes: &[u8]) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("fnv1a64:{hash:016x}")
}

fn count_logical_lines(data: &str) -> usize {
    let newline_count = data.bytes().filter(|byte| *byte == b'\n').count();
    if data.is_empty() || data.ends_with('\n') {
        newline_count
    } else {
        newline_count.saturating_add(1)
    }
}

fn validate_required_string(
    path: &Path,
    field: impl Into<String>,
    value: &str,
) -> Result<(), FixtureLoadError> {
    if value.trim().is_empty() {
        return Err(invalid_schema(path, field, "value is required"));
    }
    Ok(())
}

fn validate_timestamp(
    path: &Path,
    field: impl Into<String>,
    value: &str,
) -> Result<(), FixtureLoadError> {
    let field = field.into();
    validate_required_string(path, field.clone(), value)?;
    if !value.contains('T') || !value.ends_with('Z') {
        return Err(invalid_schema(
            path,
            field,
            "timestamp must use UTC ISO-8601 form ending in Z",
        ));
    }
    Ok(())
}

fn scan_raw_field_names(path: &Path, json: &str) -> Result<(), FixtureLoadError> {
    let json_lower = json.to_ascii_lowercase();
    for field in RAW_TRANSCRIPT_FIELDS {
        let quoted = format!("\"{field}\"");
        if json_lower.contains(&quoted) {
            return Err(invalid_schema(
                path,
                "$",
                format!("public replay contains raw transcript field {field:?}"),
            ));
        }
    }
    Ok(())
}

fn read_public_replay_file_capped(path: &Path) -> Result<String, FixtureLoadError> {
    let metadata = fs::metadata(path).map_err(|error| FixtureLoadError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    if !metadata.is_file() {
        return Err(invalid_schema(
            path,
            "$",
            "M5 public replay path must be a regular file",
        ));
    }
    if metadata.len() > M5_MAX_PUBLIC_REPLAY_FILE_BYTES {
        return Err(invalid_schema(
            path,
            "$",
            format!(
                "M5 public replay file is {} bytes, maximum is {M5_MAX_PUBLIC_REPLAY_FILE_BYTES}",
                metadata.len()
            ),
        ));
    }

    let file = fs::File::open(path).map_err(|error| FixtureLoadError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    let mut reader = file.take(M5_MAX_PUBLIC_REPLAY_FILE_BYTES + 1);
    let mut raw = String::new();
    reader
        .read_to_string(&mut raw)
        .map_err(|error| FixtureLoadError::Io {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    if raw.len() as u64 > M5_MAX_PUBLIC_REPLAY_FILE_BYTES {
        return Err(invalid_schema(
            path,
            "$",
            format!(
                "M5 public replay exceeded maximum of {M5_MAX_PUBLIC_REPLAY_FILE_BYTES} bytes while reading"
            ),
        ));
    }
    Ok(raw)
}

#[cfg(test)]
mod tests {
    use super::{
        M5PublicReplayFixture, M5ReplayAgent, M5ReplayVerificationStatus,
        m5_default_replay_fixtures,
    };

    #[test]
    fn default_replay_derivatives_verify_deterministically() {
        let fixtures = m5_default_replay_fixtures("2026-07-04T00:00:00Z");

        assert_eq!(fixtures.len(), 4);
        for fixture in fixtures {
            let report = fixture.verify().expect("M5 replay should verify");
            assert_eq!(report.status, M5ReplayVerificationStatus::Pass);
            assert!(report.final_snapshot_hash.starts_with("fnv1a64:"));
            if fixture.agent == M5ReplayAgent::LongSession {
                assert!(report.logical_lines_processed >= 100_000);
                assert!(report.discarded_rows > 0);
            }
        }
    }

    #[test]
    fn m5_public_replay_rejects_private_path_content() {
        let json = r#"{
          "schema": "hera.m5_public_replay",
          "version": 1,
          "id": "private-path",
          "title": "Private path leak",
          "agent": "codex",
          "source": "scrubbed_capture_derivative",
          "privacy": "scrubbed_public",
          "generated_at": "2026-07-04T00:00:00Z",
          "redaction": {
            "status": "passed",
            "policy_version": 1,
            "checked_at": "2026-07-04T00:00:00Z",
            "reject_patterns": [],
            "normalized_fields": []
          },
          "initial_size": { "columns": 80, "rows": 24 },
          "replay_policy": { "timeout_ms": 5000, "memory_budget_bytes": 8388608 },
          "events": [{ "kind": "output", "data": "C:\\Users\\Arthur\\secret\\n" }]
        }"#;

        let error = M5PublicReplayFixture::from_json_str("private.json", json)
            .expect_err("private path must be rejected");
        assert!(error.to_string().contains("rejected redaction pattern"));
    }

    #[test]
    fn m5_public_replay_rejects_token_like_content() {
        let json = r#"{
          "schema": "hera.m5_public_replay",
          "version": 1,
          "id": "token",
          "title": "Token leak",
          "agent": "claude_code",
          "source": "scrubbed_capture_derivative",
          "privacy": "scrubbed_public",
          "generated_at": "2026-07-04T00:00:00Z",
          "redaction": {
            "status": "passed",
            "policy_version": 1,
            "checked_at": "2026-07-04T00:00:00Z",
            "reject_patterns": [],
            "normalized_fields": []
          },
          "initial_size": { "columns": 80, "rows": 24 },
          "replay_policy": { "timeout_ms": 5000, "memory_budget_bytes": 8388608 },
          "events": [{ "kind": "output", "data": "secret sk-live-value\\n" }]
        }"#;

        let error = M5PublicReplayFixture::from_json_str("token.json", json)
            .expect_err("token-like value must be rejected");
        assert!(error.to_string().contains("rejected redaction pattern"));
    }

    #[test]
    fn m5_public_replay_rejects_raw_input_transcript_fields() {
        let json = r#"{
          "schema": "hera.m5_public_replay",
          "version": 1,
          "id": "raw-input",
          "title": "Raw input leak",
          "agent": "codex",
          "source": "scrubbed_capture_derivative",
          "privacy": "scrubbed_public",
          "generated_at": "2026-07-04T00:00:00Z",
          "redaction": {
            "status": "passed",
            "policy_version": 1,
            "checked_at": "2026-07-04T00:00:00Z",
            "reject_patterns": [],
            "normalized_fields": []
          },
          "initial_size": { "columns": 80, "rows": 24 },
          "replay_policy": { "timeout_ms": 5000, "memory_budget_bytes": 8388608 },
          "raw_input": "private prompt",
          "events": [{ "kind": "output", "data": "ok\\n" }]
        }"#;

        let error = M5PublicReplayFixture::from_json_str("raw-input.json", json)
            .expect_err("raw input field must be rejected");
        assert!(error.to_string().contains("raw transcript field"));
    }
}
