use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::json;
use terminal_core::{Terminal, TerminalConfig};

use super::{
    FixtureLoadError, SnapshotCodecError, TerminalSnapshot, invalid_schema, recording_schema_error,
    serialize_snapshot, snapshot_terminal,
};

pub const M4_PUBLIC_REPLAY_SCHEMA: &str = "hera.m4_public_replay";
pub const M4_PUBLIC_REPLAY_VERSION: u32 = 1;
pub const M4_EVENT_STREAM_SCHEMA: &str = "hera.m4_event_stream";
pub const M4_REPLAY_VERIFICATION_SCHEMA: &str = "hera.m4_replay_verification";
pub const M4_REPLAY_VERIFICATION_VERSION: u32 = 1;
pub const M4_MAX_PUBLIC_REPLAY_FILE_BYTES: u64 = 2 * 1024 * 1024;
pub const M4_MAX_PUBLIC_REPLAY_EVENTS: usize = 1024;
pub const M4_MAX_PUBLIC_REPLAY_OUTPUT_BYTES: usize = 1024 * 1024;
pub const M4_MAX_PUBLIC_REPLAY_EVENT_BYTES: usize = 64 * 1024;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M4PublicReplayFixture {
    pub schema: String,
    pub version: u32,
    pub id: String,
    pub title: String,
    pub source: M4ReplaySource,
    pub privacy: M4ReplayPrivacy,
    pub generated_at: String,
    pub redaction: M4ReplayRedaction,
    pub initial_size: M4ReplaySize,
    pub events: Vec<M4ReplayEvent>,
    #[serde(default)]
    pub notes: Vec<String>,
}

impl M4PublicReplayFixture {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref();
        let raw = read_public_replay_file_capped(path)?;
        Self::from_json_str(path, &raw)
    }

    pub fn from_json_str(path: impl AsRef<Path>, json: &str) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref().to_path_buf();
        if json.len() as u64 > M4_MAX_PUBLIC_REPLAY_FILE_BYTES {
            return Err(invalid_schema(
                &path,
                "$",
                format!(
                    "M4 public replay JSON is {} bytes, maximum is {M4_MAX_PUBLIC_REPLAY_FILE_BYTES}",
                    json.len()
                ),
            ));
        }

        let mut deserializer = serde_json::Deserializer::from_str(json);
        let fixture: Self =
            serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
                recording_schema_error(&path, &error.path().to_string(), &error.inner().to_string())
            })?;
        fixture.validate(&path)?;
        Ok(fixture)
    }

    pub fn verify(&self) -> Result<M4ReplayVerificationReport, M4ReplayVerificationError> {
        let first = self.replay_once()?;
        let second = self.replay_once()?;
        if first.snapshot_bytes != second.snapshot_bytes {
            return Err(M4ReplayVerificationError::NondeterministicSnapshot {
                fixture_id: self.id.clone(),
                first_hash: stable_snapshot_hash(&first.snapshot_bytes),
                second_hash: stable_snapshot_hash(&second.snapshot_bytes),
            });
        }

        Ok(M4ReplayVerificationReport {
            fixture_id: self.id.clone(),
            title: self.title.clone(),
            status: M4ReplayVerificationStatus::Pass,
            event_count: self.events.len(),
            event_counts: self.event_counts(),
            final_snapshot_hash: stable_snapshot_hash(&first.snapshot_bytes),
            final_snapshot_bytes: first.snapshot_bytes.len(),
            notes: self.notes.clone(),
        })
    }

    pub fn to_public_event_stream(&self) -> Result<String, M4ReplayExportError> {
        if !self.privacy.is_public() {
            return Err(M4ReplayExportError::PrivateFixture {
                fixture_id: self.id.clone(),
                privacy: self.privacy,
            });
        }

        let mut lines = Vec::with_capacity(self.events.len() + 1);
        lines.push(
            serde_json::to_string(&json!({
                "schema": M4_EVENT_STREAM_SCHEMA,
                "version": M4_PUBLIC_REPLAY_VERSION,
                "fixture_id": self.id,
                "width": self.initial_size.columns,
                "height": self.initial_size.rows,
                "public_mode": true,
                "format": "hera-jsonl",
                "asciicast_v2_reference": "https://docs.asciinema.org/manual/asciicast/v2/",
                "asciicast_v2_mapping": {
                    "shared_concepts": ["newline-delimited JSON", "header first line", "event stream"],
                    "hera_specific": ["object events", "millisecond offsets", "public redaction metadata"],
                    "incompatibility": "This is not an asciicast v2 file and should not be passed to an asciicast player."
                }
            }))
            .map_err(|error| M4ReplayExportError::Serialize {
                message: error.to_string(),
            })?,
        );

        for event in &self.events {
            lines.push(event.to_public_stream_line(self.privacy)?);
        }

        lines.push(String::new());
        Ok(lines.join("\n"))
    }

    fn replay_once(&self) -> Result<M4ReplayOnce, M4ReplayVerificationError> {
        let config = TerminalConfig::new(self.initial_size.columns, self.initial_size.rows)
            .map_err(|error| M4ReplayVerificationError::TerminalConfig {
                fixture_id: self.id.clone(),
                message: error.to_string(),
            })?;
        let mut terminal = Terminal::with_config(config);

        for event in &self.events {
            match event.kind.as_str() {
                "output" => {
                    let data = event.data.as_deref().unwrap_or_default();
                    terminal.advance_bytes(data.as_bytes());
                }
                "resize" => {
                    let columns = event.columns.unwrap_or(self.initial_size.columns);
                    let rows = event.rows.unwrap_or(self.initial_size.rows);
                    terminal.resize(columns, rows).map_err(|error| {
                        M4ReplayVerificationError::TerminalConfig {
                            fixture_id: self.id.clone(),
                            message: error.to_string(),
                        }
                    })?;
                }
                "marker" => {}
                other => {
                    return Err(M4ReplayVerificationError::UnsupportedEvent {
                        fixture_id: self.id.clone(),
                        kind: other.to_owned(),
                    });
                }
            }
        }

        let snapshot = snapshot_terminal(&mut terminal);
        let snapshot_bytes = serialize_snapshot(&snapshot)?;
        Ok(M4ReplayOnce {
            snapshot,
            snapshot_bytes,
        })
    }

    fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        if self.schema != M4_PUBLIC_REPLAY_SCHEMA {
            return Err(invalid_schema(
                path,
                "schema",
                format!("expected {M4_PUBLIC_REPLAY_SCHEMA}"),
            ));
        }
        if self.version != M4_PUBLIC_REPLAY_VERSION {
            return Err(invalid_schema(
                path,
                "version",
                format!("expected {M4_PUBLIC_REPLAY_VERSION}"),
            ));
        }
        validate_required_string(path, "id", &self.id)?;
        validate_required_string(path, "title", &self.title)?;
        validate_timestamp(path, "generated_at", &self.generated_at)?;
        self.redaction.validate(path)?;
        self.initial_size.validate(path, "initial_size")?;
        if !self.privacy.is_public() {
            return Err(invalid_schema(
                path,
                "privacy",
                "M4 public replay fixtures must be scrubbed_public or synthetic_public",
            ));
        }
        if self.events.is_empty() {
            return Err(invalid_schema(
                path,
                "events",
                "at least one event is required",
            ));
        }
        if self.events.len() > M4_MAX_PUBLIC_REPLAY_EVENTS {
            return Err(invalid_schema(
                path,
                "events",
                format!(
                    "replay has {} events, maximum is {M4_MAX_PUBLIC_REPLAY_EVENTS}",
                    self.events.len()
                ),
            ));
        }

        let mut output_bytes = 0usize;
        let mut saw_output = false;
        for (index, event) in self.events.iter().enumerate() {
            let field = format!("events[{index}]");
            event.validate(path, &field)?;
            if event.kind == "output" {
                saw_output = true;
                let bytes = event.data.as_deref().unwrap_or_default().len();
                output_bytes = output_bytes.checked_add(bytes).ok_or_else(|| {
                    invalid_schema(path, "events", "output byte count overflowed")
                })?;
                if output_bytes > M4_MAX_PUBLIC_REPLAY_OUTPUT_BYTES {
                    return Err(invalid_schema(
                        path,
                        "events",
                        format!(
                            "replay output is {output_bytes} bytes, maximum is {M4_MAX_PUBLIC_REPLAY_OUTPUT_BYTES}"
                        ),
                    ));
                }
            }
        }
        if !saw_output {
            return Err(invalid_schema(
                path,
                "events",
                "at least one output event is required for replay proof",
            ));
        }

        self.scan_public_content(path)
    }

    fn scan_public_content(&self, path: &Path) -> Result<(), FixtureLoadError> {
        let mut patterns = DEFAULT_REJECT_PATTERNS
            .iter()
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

    fn event_counts(&self) -> M4ReplayEventCounts {
        let mut counts = M4ReplayEventCounts::default();
        for event in &self.events {
            match event.kind.as_str() {
                "output" => counts.output += 1,
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
pub enum M4ReplaySource {
    Synthetic,
    ScrubbedCapture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M4ReplayPrivacy {
    SyntheticPublic,
    ScrubbedPublic,
    RawLocal,
}

impl M4ReplayPrivacy {
    const fn is_public(self) -> bool {
        matches!(self, Self::SyntheticPublic | Self::ScrubbedPublic)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M4ReplayRedaction {
    pub status: M4ReplayRedactionStatus,
    pub policy_version: u32,
    pub checked_at: String,
    #[serde(default)]
    pub reject_patterns: Vec<String>,
    #[serde(default)]
    pub normalized_fields: Vec<String>,
}

impl M4ReplayRedaction {
    fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        if self.status != M4ReplayRedactionStatus::Passed {
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
pub enum M4ReplayRedactionStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M4ReplaySize {
    pub columns: usize,
    pub rows: usize,
}

impl M4ReplaySize {
    fn validate(&self, path: &Path, field: impl Into<String>) -> Result<(), FixtureLoadError> {
        let field = field.into();
        TerminalConfig::new(self.columns, self.rows)
            .map(|_| ())
            .map_err(|error| invalid_schema(path, field, error.to_string()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M4ReplayEvent {
    pub kind: String,
    #[serde(default)]
    pub elapsed_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

impl M4ReplayEvent {
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
                if data.len() > M4_MAX_PUBLIC_REPLAY_EVENT_BYTES {
                    return Err(invalid_schema(
                        path,
                        format!("{field}.data"),
                        format!(
                            "output event is {} bytes, maximum is {M4_MAX_PUBLIC_REPLAY_EVENT_BYTES}",
                            data.len()
                        ),
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
                M4ReplaySize { columns, rows }.validate(path, field)?;
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

    fn to_public_stream_line(
        &self,
        privacy: M4ReplayPrivacy,
    ) -> Result<String, M4ReplayExportError> {
        let value = match self.kind.as_str() {
            "output" => json!({
                "kind": "output",
                "time_ms": self.elapsed_ms,
                "data": self.data.as_deref().unwrap_or_default(),
                "privacy": privacy,
            }),
            "resize" => json!({
                "kind": "resize",
                "time_ms": self.elapsed_ms,
                "columns": self.columns,
                "rows": self.rows,
                "privacy": privacy,
            }),
            "marker" => json!({
                "kind": "marker",
                "time_ms": self.elapsed_ms,
                "label": self.label,
                "privacy": privacy,
            }),
            other => {
                return Err(M4ReplayExportError::UnsupportedEvent {
                    kind: other.to_owned(),
                });
            }
        };

        serde_json::to_string(&value).map_err(|error| M4ReplayExportError::Serialize {
            message: error.to_string(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct M4ReplayOnce {
    snapshot: TerminalSnapshot,
    snapshot_bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M4ReplayVerificationStatus {
    Pass,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M4ReplayEventCounts {
    pub output: usize,
    pub resize: usize,
    pub marker: usize,
    pub unsupported: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M4ReplayVerificationReport {
    pub fixture_id: String,
    pub title: String,
    pub status: M4ReplayVerificationStatus,
    pub event_count: usize,
    pub event_counts: M4ReplayEventCounts,
    pub final_snapshot_hash: String,
    pub final_snapshot_bytes: usize,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M4ReplayVerificationSummary {
    pub schema: String,
    pub version: u32,
    pub generated_at: String,
    pub command: String,
    pub status: M4ReplayVerificationStatus,
    pub fixtures: Vec<M4ReplayVerificationReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum M4ReplayVerificationError {
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

impl fmt::Display for M4ReplayVerificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TerminalConfig {
                fixture_id,
                message,
            } => write!(
                formatter,
                "M4 replay fixture {fixture_id} has invalid terminal config: {message}"
            ),
            Self::UnsupportedEvent { fixture_id, kind } => write!(
                formatter,
                "M4 replay fixture {fixture_id} contains unsupported event kind {kind:?}"
            ),
            Self::NondeterministicSnapshot {
                fixture_id,
                first_hash,
                second_hash,
            } => write!(
                formatter,
                "M4 replay fixture {fixture_id} was nondeterministic: first {first_hash}, second {second_hash}"
            ),
            Self::SnapshotCodec(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for M4ReplayVerificationError {}

impl From<SnapshotCodecError> for M4ReplayVerificationError {
    fn from(value: SnapshotCodecError) -> Self {
        Self::SnapshotCodec(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum M4ReplayExportError {
    PrivateFixture {
        fixture_id: String,
        privacy: M4ReplayPrivacy,
    },
    UnsupportedEvent {
        kind: String,
    },
    Serialize {
        message: String,
    },
}

impl fmt::Display for M4ReplayExportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PrivateFixture {
                fixture_id,
                privacy,
            } => write!(
                formatter,
                "M4 replay fixture {fixture_id} cannot be exported in public mode with privacy {privacy:?}"
            ),
            Self::UnsupportedEvent { kind } => {
                write!(
                    formatter,
                    "M4 event stream export does not support event kind {kind:?}"
                )
            }
            Self::Serialize { message } => {
                write!(formatter, "M4 event stream serialization failed: {message}")
            }
        }
    }
}

impl std::error::Error for M4ReplayExportError {}

fn stable_snapshot_hash(bytes: &[u8]) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("fnv1a64:{hash:016x}")
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

fn read_public_replay_file_capped(path: &Path) -> Result<String, FixtureLoadError> {
    let metadata = fs::metadata(path).map_err(|error| FixtureLoadError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    if !metadata.is_file() {
        return Err(invalid_schema(
            path,
            "$",
            "M4 public replay path must be a regular file",
        ));
    }
    if metadata.len() > M4_MAX_PUBLIC_REPLAY_FILE_BYTES {
        return Err(invalid_schema(
            path,
            "$",
            format!(
                "M4 public replay file is {} bytes, maximum is {M4_MAX_PUBLIC_REPLAY_FILE_BYTES}",
                metadata.len()
            ),
        ));
    }

    let file = fs::File::open(path).map_err(|error| FixtureLoadError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    let mut reader = file.take(M4_MAX_PUBLIC_REPLAY_FILE_BYTES + 1);
    let mut raw = String::new();
    reader
        .read_to_string(&mut raw)
        .map_err(|error| FixtureLoadError::Io {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    if raw.len() as u64 > M4_MAX_PUBLIC_REPLAY_FILE_BYTES {
        return Err(invalid_schema(
            path,
            "$",
            format!(
                "M4 public replay exceeded maximum of {M4_MAX_PUBLIC_REPLAY_FILE_BYTES} bytes while reading"
            ),
        ));
    }
    Ok(raw)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{M4PublicReplayFixture, M4ReplayPrivacy};

    fn corpus_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/m4-replay")
    }

    #[test]
    fn m4_public_replay_corpus_replays_deterministically() {
        for name in [
            "basic-shell.json",
            "resize-and-wrap.json",
            "alternate-screen.json",
        ] {
            let fixture = M4PublicReplayFixture::from_path(corpus_dir().join(name))
                .expect("M4 public replay fixture should load");
            let report = fixture.verify().expect("M4 public replay should verify");

            assert_eq!(report.event_count, fixture.events.len());
            assert_eq!(report.event_counts.unsupported, 0);
            assert!(report.final_snapshot_hash.starts_with("fnv1a64:"));
        }
    }

    #[test]
    fn public_event_stream_export_uses_header_then_event_lines() {
        let fixture = M4PublicReplayFixture::from_path(corpus_dir().join("basic-shell.json"))
            .expect("M4 public replay fixture should load");
        let stream = fixture
            .to_public_event_stream()
            .expect("public stream should export");
        let mut lines = stream.lines();

        assert!(
            lines
                .next()
                .unwrap()
                .contains("\"schema\":\"hera.m4_event_stream\"")
        );
        assert!(lines.next().unwrap().contains("\"kind\":\"output\""));
        assert!(stream.contains("\"asciicast_v2_reference\""));
    }

    #[test]
    fn m4_public_replay_rejects_private_path_content() {
        let json = r#"{
          "schema": "hera.m4_public_replay",
          "version": 1,
          "id": "private-path",
          "title": "Private path leak",
          "source": "scrubbed_capture",
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
          "events": [{ "kind": "output", "data": "C:\\Users\\Arthur\\secret\\n" }]
        }"#;

        let error = M4PublicReplayFixture::from_json_str("private.json", json)
            .expect_err("private path must be rejected");
        assert!(error.to_string().contains("rejected redaction pattern"));
    }

    #[test]
    fn m4_public_replay_rejects_unsupported_future_event_kind() {
        let json = r#"{
          "schema": "hera.m4_public_replay",
          "version": 1,
          "id": "future-event",
          "title": "Future event",
          "source": "synthetic",
          "privacy": "synthetic_public",
          "generated_at": "2026-07-04T00:00:00Z",
          "redaction": {
            "status": "passed",
            "policy_version": 1,
            "checked_at": "2026-07-04T00:00:00Z",
            "reject_patterns": [],
            "normalized_fields": []
          },
          "initial_size": { "columns": 80, "rows": 24 },
          "events": [{ "kind": "process_id", "data": "1234" }]
        }"#;

        let error = M4PublicReplayFixture::from_json_str("future.json", json)
            .expect_err("unsupported events must fail clearly");
        assert!(error.to_string().contains("unsupported event kind"));
    }

    #[test]
    fn raw_local_fixture_cannot_be_public() {
        let json = r#"{
          "schema": "hera.m4_public_replay",
          "version": 1,
          "id": "raw-local",
          "title": "Raw local fixture",
          "source": "scrubbed_capture",
          "privacy": "raw_local",
          "generated_at": "2026-07-04T00:00:00Z",
          "redaction": {
            "status": "passed",
            "policy_version": 1,
            "checked_at": "2026-07-04T00:00:00Z",
            "reject_patterns": [],
            "normalized_fields": []
          },
          "initial_size": { "columns": 80, "rows": 24 },
          "events": [{ "kind": "output", "data": "ok\\n" }]
        }"#;

        let error = M4PublicReplayFixture::from_json_str("raw-local.json", json)
            .expect_err("raw local fixtures cannot be public");
        assert!(
            error
                .to_string()
                .contains("scrubbed_public or synthetic_public")
        );
        assert!(!M4ReplayPrivacy::RawLocal.is_public());
    }
}
