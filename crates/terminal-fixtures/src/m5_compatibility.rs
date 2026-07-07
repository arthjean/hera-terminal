use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{FixtureLoadError, FixturePack, invalid_schema, recording_schema_error};

pub const M5_COMPATIBILITY_MATRIX_SCHEMA: &str = "hera.m5_compatibility_matrix";
pub const M5_COMPATIBILITY_MATRIX_VERSION: u32 = 1;
pub const M5_MAX_COMPATIBILITY_MATRIX_BYTES: u64 = 1024 * 1024;
const M5_MAX_COMPATIBILITY_ROWS: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5CompatibilityMatrix {
    pub schema: String,
    pub version: u32,
    pub generated_at: String,
    pub rows: Vec<M5CompatibilityRow>,
}

impl M5CompatibilityMatrix {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref();
        let raw = read_matrix_file_capped(path)?;
        Self::from_json_str(path, &raw)
    }

    pub fn from_json_str(path: impl AsRef<Path>, json: &str) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref().to_path_buf();
        if json.len() as u64 > M5_MAX_COMPATIBILITY_MATRIX_BYTES {
            return Err(invalid_schema(
                &path,
                "$",
                format!(
                    "M5 compatibility matrix JSON is {} bytes, maximum is {M5_MAX_COMPATIBILITY_MATRIX_BYTES}",
                    json.len()
                ),
            ));
        }

        let mut deserializer = serde_json::Deserializer::from_str(json);
        let matrix: Self =
            serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
                recording_schema_error(&path, &error.path().to_string(), &error.inner().to_string())
            })?;
        matrix.validate(&path)?;
        Ok(matrix)
    }

    pub fn validate_referenced_artifacts(
        &self,
        matrix_path: impl AsRef<Path>,
        repo_root: impl AsRef<Path>,
    ) -> Result<(), FixtureLoadError> {
        let matrix_path = matrix_path.as_ref();
        let repo_root = repo_root.as_ref();

        for (row_index, row) in self.rows.iter().enumerate() {
            for (artifact_index, artifact) in row.fixture_coverage.artifacts.iter().enumerate() {
                let path = repo_root.join(&artifact.path);
                if !path.is_file() {
                    return Err(invalid_schema(
                        matrix_path,
                        format!(
                            "rows[{row_index}].fixture_coverage.artifacts[{artifact_index}].path"
                        ),
                        format!("linked artifact does not exist: {}", artifact.path),
                    ));
                }

                if artifact.kind == M5CompatibilityArtifactKind::Fixture {
                    let Some(name) = artifact.name.as_deref() else {
                        return Err(invalid_schema(
                            matrix_path,
                            format!(
                                "rows[{row_index}].fixture_coverage.artifacts[{artifact_index}].name"
                            ),
                            "fixture links must include the fixture name",
                        ));
                    };
                    let pack = FixturePack::from_path(&path)?;
                    if !pack.fixtures().iter().any(|fixture| fixture.name() == name) {
                        return Err(invalid_schema(
                            matrix_path,
                            format!(
                                "rows[{row_index}].fixture_coverage.artifacts[{artifact_index}].name"
                            ),
                            format!("fixture {name:?} not found in {}", artifact.path),
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    pub fn rows(&self) -> &[M5CompatibilityRow] {
        &self.rows
    }

    pub fn pass_count(&self) -> usize {
        self.rows
            .iter()
            .filter(|row| row.m5_disposition == M5Disposition::Pass)
            .count()
    }

    pub fn measured_count(&self) -> usize {
        self.rows
            .iter()
            .filter(|row| {
                !matches!(
                    row.m5_disposition,
                    M5Disposition::Deferred | M5Disposition::OutOfScope
                )
            })
            .count()
    }

    fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        if self.schema != M5_COMPATIBILITY_MATRIX_SCHEMA {
            return Err(invalid_schema(
                path,
                "schema",
                format!("expected {M5_COMPATIBILITY_MATRIX_SCHEMA}"),
            ));
        }
        if self.version != M5_COMPATIBILITY_MATRIX_VERSION {
            return Err(invalid_schema(
                path,
                "version",
                format!("expected {M5_COMPATIBILITY_MATRIX_VERSION}"),
            ));
        }
        validate_timestamp(path, "generated_at", &self.generated_at)?;
        if self.rows.is_empty() {
            return Err(invalid_schema(path, "rows", "at least one row is required"));
        }
        if self.rows.len() > M5_MAX_COMPATIBILITY_ROWS {
            return Err(invalid_schema(
                path,
                "rows",
                format!(
                    "matrix has {} rows, maximum is {M5_MAX_COMPATIBILITY_ROWS}",
                    self.rows.len()
                ),
            ));
        }

        let mut ids = BTreeSet::new();
        for (index, row) in self.rows.iter().enumerate() {
            row.validate(path, index)?;
            if !ids.insert(row.behavior_id.as_str()) {
                return Err(invalid_schema(
                    path,
                    format!("rows[{index}].behavior_id"),
                    "behavior id must be unique",
                ));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5CompatibilityRow {
    pub behavior_id: String,
    pub category: String,
    pub behavior: String,
    pub priority: M5CompatibilityPriority,
    pub source_reference: M5SourceReference,
    pub fixture_coverage: M5FixtureCoverage,
    pub platform_measurements: M5PlatformMeasurements,
    pub m5_disposition: M5Disposition,
    pub owner: String,
    #[serde(default)]
    pub deferred: Option<M5DeferredPolicy>,
    #[serde(default)]
    pub notes: Vec<String>,
}

impl M5CompatibilityRow {
    fn validate(&self, path: &Path, index: usize) -> Result<(), FixtureLoadError> {
        validate_required_string(
            path,
            format!("rows[{index}].behavior_id"),
            &self.behavior_id,
        )?;
        validate_required_string(path, format!("rows[{index}].category"), &self.category)?;
        validate_required_string(path, format!("rows[{index}].behavior"), &self.behavior)?;
        validate_required_string(path, format!("rows[{index}].owner"), &self.owner)?;
        validate_string_list(path, format!("rows[{index}].notes"), &self.notes)?;
        self.source_reference
            .validate(path, &format!("rows[{index}].source_reference"))?;
        self.fixture_coverage
            .validate(path, &format!("rows[{index}].fixture_coverage"))?;
        self.platform_measurements
            .validate(path, &format!("rows[{index}].platform_measurements"))?;

        match self.m5_disposition {
            M5Disposition::Pass => {
                if !matches!(
                    self.fixture_coverage.status,
                    M5FixtureCoverageStatus::FixtureBacked | M5FixtureCoverageStatus::ReplayBacked
                ) {
                    return Err(invalid_schema(
                        path,
                        format!("rows[{index}].fixture_coverage.status"),
                        "pass rows must be fixture_backed or replay_backed",
                    ));
                }
                if self.fixture_coverage.artifacts.is_empty() {
                    return Err(invalid_schema(
                        path,
                        format!("rows[{index}].fixture_coverage.artifacts"),
                        "pass rows must link at least one fixture or replay artifact",
                    ));
                }
                if self.deferred.is_some() {
                    return Err(invalid_schema(
                        path,
                        format!("rows[{index}].deferred"),
                        "pass rows must not include deferred policy",
                    ));
                }
            }
            M5Disposition::Deferred => {
                let Some(deferred) = &self.deferred else {
                    return Err(invalid_schema(
                        path,
                        format!("rows[{index}].deferred"),
                        "deferred rows must include reason, owner and M6 follow-up",
                    ));
                };
                deferred.validate(path, &format!("rows[{index}].deferred"))?;
                if self.fixture_coverage.status != M5FixtureCoverageStatus::Deferred {
                    return Err(invalid_schema(
                        path,
                        format!("rows[{index}].fixture_coverage.status"),
                        "deferred rows must use deferred fixture coverage status",
                    ));
                }
            }
            M5Disposition::Failed | M5Disposition::NotImplemented | M5Disposition::OutOfScope => {
                if self.notes.is_empty() {
                    return Err(invalid_schema(
                        path,
                        format!("rows[{index}].notes"),
                        "non-pass rows must include at least one note",
                    ));
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5CompatibilityPriority {
    P0,
    P1,
    P2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5Disposition {
    Pass,
    Failed,
    Deferred,
    NotImplemented,
    OutOfScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5DeferredPolicy {
    pub reason: String,
    pub owner: String,
    pub m6_follow_up: String,
}

impl M5DeferredPolicy {
    fn validate(&self, path: &Path, field: &str) -> Result<(), FixtureLoadError> {
        validate_required_string(path, format!("{field}.reason"), &self.reason)?;
        validate_required_string(path, format!("{field}.owner"), &self.owner)?;
        validate_required_string(path, format!("{field}.m6_follow_up"), &self.m6_follow_up)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5FixtureCoverage {
    pub status: M5FixtureCoverageStatus,
    pub artifacts: Vec<M5CompatibilityArtifact>,
}

impl M5FixtureCoverage {
    fn validate(&self, path: &Path, field: &str) -> Result<(), FixtureLoadError> {
        if self.status.requires_artifact() && self.artifacts.is_empty() {
            return Err(invalid_schema(
                path,
                format!("{field}.artifacts"),
                "fixture-backed coverage must link at least one artifact",
            ));
        }
        for (index, artifact) in self.artifacts.iter().enumerate() {
            artifact.validate(path, &format!("{field}.artifacts[{index}]"))?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5FixtureCoverageStatus {
    FixtureBacked,
    ReplayBacked,
    NotMeasured,
    NotImplemented,
    Deferred,
    OutOfScope,
}

impl M5FixtureCoverageStatus {
    const fn requires_artifact(self) -> bool {
        matches!(self, Self::FixtureBacked | Self::ReplayBacked)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5CompatibilityArtifact {
    pub kind: M5CompatibilityArtifactKind,
    pub path: String,
    #[serde(default)]
    pub name: Option<String>,
}

impl M5CompatibilityArtifact {
    fn validate(&self, path: &Path, field: &str) -> Result<(), FixtureLoadError> {
        validate_repo_path(path, format!("{field}.path"), &self.path)?;
        if let Some(name) = self.name.as_deref() {
            validate_required_string(path, format!("{field}.name"), name)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5CompatibilityArtifactKind {
    Fixture,
    ReplayArtifact,
    EvidenceArtifact,
    Doc,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5SourceReference {
    pub kind: M5SourceReferenceKind,
    pub label: String,
    pub url: String,
}

impl M5SourceReference {
    fn validate(&self, matrix_path: &Path, field: &str) -> Result<(), FixtureLoadError> {
        validate_required_string(matrix_path, format!("{field}.label"), &self.label)?;
        match self.kind {
            M5SourceReferenceKind::Vttest => validate_source_url(
                matrix_path,
                format!("{field}.url"),
                &self.url,
                "https://invisible-island.net/vttest/",
            )?,
            M5SourceReferenceKind::Esctest2 => validate_source_url(
                matrix_path,
                format!("{field}.url"),
                &self.url,
                "https://github.com/ThomasDickey/esctest2",
            )?,
            M5SourceReferenceKind::XtermControlSequences => {
                if !self
                    .url
                    .starts_with("https://www.xfree86.org/current/ctlseqs.html")
                    && !self
                        .url
                        .starts_with("https://invisible-island.net/xterm/ctlseqs/")
                {
                    return Err(invalid_schema(
                        matrix_path,
                        format!("{field}.url"),
                        "xterm references must point to the public xterm control sequence docs",
                    ));
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5SourceReferenceKind {
    Vttest,
    Esctest2,
    XtermControlSequences,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5PlatformMeasurements {
    pub windows: M5PlatformMeasurementStatus,
    pub linux: M5PlatformMeasurementStatus,
    pub macos: M5PlatformMeasurementStatus,
    #[serde(default)]
    pub notes: Vec<String>,
}

impl M5PlatformMeasurements {
    fn validate(&self, path: &Path, field: &str) -> Result<(), FixtureLoadError> {
        validate_string_list(path, format!("{field}.notes"), &self.notes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5PlatformMeasurementStatus {
    Pass,
    Fail,
    Blocked,
    NotMeasured,
    NotApplicable,
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

fn validate_string_list(
    path: &Path,
    field: impl Into<String>,
    values: &[String],
) -> Result<(), FixtureLoadError> {
    let field = field.into();
    for (index, value) in values.iter().enumerate() {
        validate_required_string(path, format!("{field}[{index}]"), value)?;
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

fn validate_repo_path(
    matrix_path: &Path,
    field: impl Into<String>,
    value: &str,
) -> Result<(), FixtureLoadError> {
    let field = field.into();
    validate_required_string(matrix_path, field.clone(), value)?;
    if value.contains('\\') || value.contains(':') {
        return Err(invalid_schema(
            matrix_path,
            field,
            "paths must be relative slash-separated repo paths",
        ));
    }
    let path = Path::new(value);
    if path.is_absolute() {
        return Err(invalid_schema(
            matrix_path,
            field,
            "paths must not be absolute",
        ));
    }
    if value
        .split('/')
        .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(invalid_schema(
            matrix_path,
            field,
            "path contains an empty or parent segment",
        ));
    }

    Ok(())
}

fn validate_source_url(
    path: &Path,
    field: impl Into<String>,
    value: &str,
    prefix: &str,
) -> Result<(), FixtureLoadError> {
    let field = field.into();
    validate_required_string(path, field.clone(), value)?;
    if !value.starts_with(prefix) {
        return Err(invalid_schema(
            path,
            field,
            format!("source URL must start with {prefix}"),
        ));
    }

    Ok(())
}

fn read_matrix_file_capped(path: &Path) -> Result<String, FixtureLoadError> {
    let metadata = fs::metadata(path).map_err(|error| FixtureLoadError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    if !metadata.is_file() {
        return Err(invalid_schema(
            path,
            "$",
            "M5 compatibility matrix path must be a regular file",
        ));
    }
    if metadata.len() > M5_MAX_COMPATIBILITY_MATRIX_BYTES {
        return Err(invalid_schema(
            path,
            "$",
            format!(
                "M5 compatibility matrix file is {} bytes, maximum is {M5_MAX_COMPATIBILITY_MATRIX_BYTES}",
                metadata.len()
            ),
        ));
    }

    let file = fs::File::open(path).map_err(|error| FixtureLoadError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    let mut reader = file.take(M5_MAX_COMPATIBILITY_MATRIX_BYTES + 1);
    let mut raw = String::new();
    reader
        .read_to_string(&mut raw)
        .map_err(|error| FixtureLoadError::Io {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;

    if raw.len() as u64 > M5_MAX_COMPATIBILITY_MATRIX_BYTES {
        return Err(invalid_schema(
            path,
            "$",
            format!(
                "M5 compatibility matrix exceeded maximum of {M5_MAX_COMPATIBILITY_MATRIX_BYTES} bytes while reading"
            ),
        ));
    }

    Ok(raw)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{M5CompatibilityMatrix, M5Disposition};
    use crate::FixtureLoadError;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn valid_matrix_json() -> &'static str {
        r#"{
          "schema": "hera.m5_compatibility_matrix",
          "version": 1,
          "generated_at": "2026-07-04T00:00:00Z",
          "rows": [{
            "behavior_id": "vt.cursor.cup",
            "category": "cursor_movement",
            "behavior": "CUP places printable cells using one-based coordinates.",
            "priority": "p0",
            "source_reference": {
              "kind": "xterm_control_sequences",
              "label": "Xterm Control Sequences",
              "url": "https://www.xfree86.org/current/ctlseqs.html"
            },
            "fixture_coverage": {
              "status": "fixture_backed",
              "artifacts": [{
                "kind": "fixture",
                "path": "crates/terminal-fixtures/fixtures/m5-compatibility.json",
                "name": "csi-cup-hvp-positioning"
              }]
            },
            "platform_measurements": {
              "windows": "pass",
              "linux": "not_measured",
              "macos": "not_measured",
              "notes": ["Fixture validation was run locally on Windows only."]
            },
            "m5_disposition": "pass",
            "owner": "terminal-core",
            "notes": []
          }]
        }"#
    }

    #[test]
    fn m5_compatibility_matrix_validates_checked_in_matrix() {
        let root = workspace_root();
        let path = root.join("evidence/m5/compatibility-matrix.json");
        let matrix = M5CompatibilityMatrix::from_path(&path).expect("matrix should load");

        assert!(matrix.rows().len() >= 18);
        assert!(
            matrix
                .rows()
                .iter()
                .any(|row| row.behavior_id == "vt.cursor.csi_positioning")
        );
        assert!(
            matrix
                .rows()
                .iter()
                .any(|row| row.behavior_id == "vt.screen.ed_el_ech")
        );
        assert!(
            matrix
                .rows()
                .iter()
                .any(|row| row.behavior_id == "xterm.private_modes.47_1047_1048")
        );
        assert!(matrix.pass_count() >= 17);
        assert!(matrix.measured_count() >= matrix.pass_count());
        matrix
            .validate_referenced_artifacts(&path, &root)
            .expect("linked fixtures should exist");
    }

    #[test]
    fn pass_rows_require_fixture_artifacts() {
        let json = valid_matrix_json().replace(
            r#""artifacts": [{
                "kind": "fixture",
                "path": "crates/terminal-fixtures/fixtures/m5-compatibility.json",
                "name": "csi-cup-hvp-positioning"
              }]"#,
            r#""artifacts": []"#,
        );

        let error = M5CompatibilityMatrix::from_json_str("matrix.json", &json)
            .expect_err("pass row without artifact must fail");

        assert!(matches!(
            error,
            FixtureLoadError::InvalidSchema { field, .. }
                if field == "rows[0].fixture_coverage.artifacts"
        ));
    }

    #[test]
    fn core_rows_require_reference_source_urls() {
        let json = valid_matrix_json().replace(
            "https://www.xfree86.org/current/ctlseqs.html",
            "https://example.com/notes",
        );

        let error = M5CompatibilityMatrix::from_json_str("matrix.json", &json)
            .expect_err("wrong source URL must fail");

        assert!(matches!(
            error,
            FixtureLoadError::InvalidSchema { field, .. }
                if field == "rows[0].source_reference.url"
        ));
    }

    #[test]
    fn deferred_rows_are_not_counted_as_pass() {
        let json = valid_matrix_json()
            .replace(
                r#""m5_disposition": "pass""#,
                r#""m5_disposition": "deferred""#,
            )
            .replace(r#""status": "fixture_backed""#, r#""status": "deferred""#)
            .replace(
                r#""artifacts": [{
                "kind": "fixture",
                "path": "crates/terminal-fixtures/fixtures/m5-compatibility.json",
                "name": "csi-cup-hvp-positioning"
              }]"#,
                r#""artifacts": []"#,
            )
            .replace(
                r#""owner": "terminal-core",
            "notes": []"#,
                r#""owner": "terminal-core",
            "deferred": {
              "reason": "Policy deferred for M6.",
              "owner": "terminal-core",
              "m6_follow_up": "Add fixture-backed behavior."
            },
            "notes": ["Deferred rows are excluded from the pass denominator."]"#,
            );

        let matrix = M5CompatibilityMatrix::from_json_str("matrix.json", &json)
            .expect("deferred row should load with policy");

        assert_eq!(matrix.rows()[0].m5_disposition, M5Disposition::Deferred);
        assert_eq!(matrix.pass_count(), 0);
        assert_eq!(matrix.measured_count(), 0);
    }
}
