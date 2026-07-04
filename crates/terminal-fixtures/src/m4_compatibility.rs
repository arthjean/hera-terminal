use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{FixtureLoadError, FixturePack, invalid_schema, recording_schema_error};

pub const M4_COMPATIBILITY_MATRIX_SCHEMA: &str = "hera.m4_compatibility_matrix";
pub const M4_COMPATIBILITY_MATRIX_VERSION: u32 = 1;
pub const M4_MAX_COMPATIBILITY_MATRIX_BYTES: u64 = 1024 * 1024;
const M4_MAX_COMPATIBILITY_ROWS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M4CompatibilityMatrix {
    pub schema: String,
    pub version: u32,
    pub generated_at: String,
    pub rows: Vec<M4CompatibilityRow>,
}

impl M4CompatibilityMatrix {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref();
        let raw = read_matrix_file_capped(path)?;
        Self::from_json_str(path, &raw)
    }

    pub fn from_json_str(path: impl AsRef<Path>, json: &str) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref().to_path_buf();
        if json.len() as u64 > M4_MAX_COMPATIBILITY_MATRIX_BYTES {
            return Err(invalid_schema(
                &path,
                "$",
                format!(
                    "M4 compatibility matrix JSON is {} bytes, maximum is {M4_MAX_COMPATIBILITY_MATRIX_BYTES}",
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

                if artifact.kind == M4CompatibilityArtifactKind::Fixture {
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

    pub fn rows(&self) -> &[M4CompatibilityRow] {
        &self.rows
    }

    fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        if self.schema != M4_COMPATIBILITY_MATRIX_SCHEMA {
            return Err(invalid_schema(
                path,
                "schema",
                format!("expected {M4_COMPATIBILITY_MATRIX_SCHEMA}"),
            ));
        }
        if self.version != M4_COMPATIBILITY_MATRIX_VERSION {
            return Err(invalid_schema(
                path,
                "version",
                format!("expected {M4_COMPATIBILITY_MATRIX_VERSION}"),
            ));
        }
        validate_timestamp(path, "generated_at", &self.generated_at)?;
        if self.rows.is_empty() {
            return Err(invalid_schema(path, "rows", "at least one row is required"));
        }
        if self.rows.len() > M4_MAX_COMPATIBILITY_ROWS {
            return Err(invalid_schema(
                path,
                "rows",
                format!(
                    "matrix has {} rows, maximum is {M4_MAX_COMPATIBILITY_ROWS}",
                    self.rows.len()
                ),
            ));
        }

        let mut ids = BTreeSet::new();
        for (index, row) in self.rows.iter().enumerate() {
            row.validate(path, index)?;
            if !ids.insert(row.id.as_str()) {
                return Err(invalid_schema(
                    path,
                    format!("rows[{index}].id"),
                    "row id must be unique",
                ));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M4CompatibilityRow {
    pub id: String,
    pub category: String,
    pub behavior: String,
    pub status: M4CompatibilityStatus,
    pub fixture_coverage: M4FixtureCoverage,
    pub source_reference: M4SourceReference,
    pub platform_measurements: M4PlatformMeasurements,
    pub notes: Vec<String>,
    pub owner: String,
}

impl M4CompatibilityRow {
    fn validate(&self, path: &Path, index: usize) -> Result<(), FixtureLoadError> {
        validate_required_string(path, format!("rows[{index}].id"), &self.id)?;
        validate_required_string(path, format!("rows[{index}].category"), &self.category)?;
        validate_required_string(path, format!("rows[{index}].behavior"), &self.behavior)?;
        validate_required_string(path, format!("rows[{index}].owner"), &self.owner)?;
        validate_string_list(path, format!("rows[{index}].notes"), &self.notes)?;
        self.fixture_coverage
            .validate(path, &format!("rows[{index}].fixture_coverage"))?;
        self.source_reference
            .validate(path, &format!("rows[{index}].source_reference"))?;
        self.platform_measurements
            .validate(path, &format!("rows[{index}].platform_measurements"))?;

        if self.status.requires_fixture()
            && !matches!(
                self.fixture_coverage.status,
                M4FixtureCoverageStatus::FixtureBacked | M4FixtureCoverageStatus::ReplayBacked
            )
        {
            return Err(invalid_schema(
                path,
                format!("rows[{index}].fixture_coverage.status"),
                "implemented rows must be fixture_backed or replay_backed",
            ));
        }
        if self.status.requires_fixture() && self.fixture_coverage.artifacts.is_empty() {
            return Err(invalid_schema(
                path,
                format!("rows[{index}].fixture_coverage.artifacts"),
                "implemented rows must link at least one fixture or replay artifact",
            ));
        }
        if self.status.requires_gap_note() && self.notes.is_empty() {
            return Err(invalid_schema(
                path,
                format!("rows[{index}].notes"),
                "gap rows must include at least one note",
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M4CompatibilityStatus {
    Implemented,
    Partial,
    ManualOnly,
    NotMeasured,
    NotImplemented,
    OutOfScope,
}

impl M4CompatibilityStatus {
    const fn requires_fixture(self) -> bool {
        matches!(self, Self::Implemented)
    }

    const fn requires_gap_note(self) -> bool {
        matches!(
            self,
            Self::Partial
                | Self::ManualOnly
                | Self::NotMeasured
                | Self::NotImplemented
                | Self::OutOfScope
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M4FixtureCoverage {
    pub status: M4FixtureCoverageStatus,
    pub artifacts: Vec<M4CompatibilityArtifact>,
}

impl M4FixtureCoverage {
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
pub enum M4FixtureCoverageStatus {
    FixtureBacked,
    ReplayBacked,
    ManualOnly,
    NotMeasured,
    NotImplemented,
    OutOfScope,
}

impl M4FixtureCoverageStatus {
    const fn requires_artifact(self) -> bool {
        matches!(self, Self::FixtureBacked | Self::ReplayBacked)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M4CompatibilityArtifact {
    pub kind: M4CompatibilityArtifactKind,
    pub path: String,
    #[serde(default)]
    pub name: Option<String>,
}

impl M4CompatibilityArtifact {
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
pub enum M4CompatibilityArtifactKind {
    Fixture,
    ReplayArtifact,
    EvidenceArtifact,
    Doc,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M4SourceReference {
    pub kind: M4SourceReferenceKind,
    pub label: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
}

impl M4SourceReference {
    fn validate(&self, matrix_path: &Path, field: &str) -> Result<(), FixtureLoadError> {
        validate_required_string(matrix_path, format!("{field}.label"), &self.label)?;
        match self.kind {
            M4SourceReferenceKind::Vttest => {
                validate_source_url(
                    matrix_path,
                    format!("{field}.url"),
                    self.url.as_deref(),
                    "https://invisible-island.net/vttest/",
                )?;
            }
            M4SourceReferenceKind::Esctest2 => {
                validate_source_url(
                    matrix_path,
                    format!("{field}.url"),
                    self.url.as_deref(),
                    "https://github.com/ThomasDickey/esctest2",
                )?;
            }
            M4SourceReferenceKind::XtermControlSequences => {
                let url = self.url.as_deref().ok_or_else(|| {
                    invalid_schema(
                        matrix_path,
                        format!("{field}.url"),
                        "source URL is required",
                    )
                })?;
                if !url.starts_with("https://www.xfree86.org/current/ctlseqs.html")
                    && !url.starts_with("https://invisible-island.net/xterm/ctlseqs/")
                {
                    return Err(invalid_schema(
                        matrix_path,
                        format!("{field}.url"),
                        "xterm references must point to the public xterm control sequence docs",
                    ));
                }
            }
            M4SourceReferenceKind::LocalFixture | M4SourceReferenceKind::LocalDoc => {
                let path = self.path.as_deref().ok_or_else(|| {
                    invalid_schema(
                        matrix_path,
                        format!("{field}.path"),
                        "source path is required",
                    )
                })?;
                validate_repo_path(matrix_path, format!("{field}.path"), path)?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M4SourceReferenceKind {
    Vttest,
    Esctest2,
    XtermControlSequences,
    LocalFixture,
    LocalDoc,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M4PlatformMeasurements {
    pub windows: M4PlatformMeasurementStatus,
    pub linux: M4PlatformMeasurementStatus,
    pub macos: M4PlatformMeasurementStatus,
    #[serde(default)]
    pub notes: Vec<String>,
}

impl M4PlatformMeasurements {
    fn validate(&self, path: &Path, field: &str) -> Result<(), FixtureLoadError> {
        validate_string_list(path, format!("{field}.notes"), &self.notes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M4PlatformMeasurementStatus {
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
    value: Option<&str>,
    prefix: &str,
) -> Result<(), FixtureLoadError> {
    let field = field.into();
    let value =
        value.ok_or_else(|| invalid_schema(path, field.clone(), "source URL is required"))?;
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
            "M4 compatibility matrix path must be a regular file",
        ));
    }
    if metadata.len() > M4_MAX_COMPATIBILITY_MATRIX_BYTES {
        return Err(invalid_schema(
            path,
            "$",
            format!(
                "M4 compatibility matrix file is {} bytes, maximum is {M4_MAX_COMPATIBILITY_MATRIX_BYTES}",
                metadata.len()
            ),
        ));
    }

    let file = fs::File::open(path).map_err(|error| FixtureLoadError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    let mut reader = file.take(M4_MAX_COMPATIBILITY_MATRIX_BYTES + 1);
    let mut raw = String::new();
    reader
        .read_to_string(&mut raw)
        .map_err(|error| FixtureLoadError::Io {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;

    if raw.len() as u64 > M4_MAX_COMPATIBILITY_MATRIX_BYTES {
        return Err(invalid_schema(
            path,
            "$",
            format!(
                "M4 compatibility matrix exceeded maximum of {M4_MAX_COMPATIBILITY_MATRIX_BYTES} bytes while reading"
            ),
        ));
    }

    Ok(raw)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        M4CompatibilityMatrix, M4CompatibilityStatus, M4PlatformMeasurementStatus,
        M4SourceReferenceKind,
    };
    use crate::FixtureLoadError;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn valid_matrix_json() -> &'static str {
        r#"{
          "schema": "hera.m4_compatibility_matrix",
          "version": 1,
          "generated_at": "2026-07-04T00:00:00Z",
          "rows": [{
            "id": "vt.sgr.character_attributes",
            "category": "character_attributes",
            "behavior": "SGR reset and truecolor mutate rendered cell style.",
            "status": "implemented",
            "fixture_coverage": {
              "status": "fixture_backed",
              "artifacts": [{
                "kind": "fixture",
                "path": "crates/terminal-fixtures/fixtures/m1-golden.json",
                "name": "sgr-reset"
              }]
            },
            "source_reference": {
              "kind": "xterm_control_sequences",
              "label": "Xterm Control Sequences",
              "url": "https://www.xfree86.org/current/ctlseqs.html"
            },
            "platform_measurements": {
              "windows": "pass",
              "linux": "not_measured",
              "macos": "not_measured",
              "notes": ["Fixture validation was run locally on Windows only."]
            },
            "notes": [],
            "owner": "terminal-core"
          }]
        }"#
    }

    #[test]
    fn m4_compatibility_matrix_validates_checked_in_matrix() {
        let root = workspace_root();
        let path = root.join("evidence/m4/compatibility-matrix.json");
        let matrix = M4CompatibilityMatrix::from_path(&path).expect("matrix should load");

        assert!(matrix.rows().len() >= 8);
        assert!(
            matrix
                .rows()
                .iter()
                .any(|row| row.category == "cursor_movement")
        );
        assert!(
            matrix
                .rows()
                .iter()
                .any(|row| row.category == "screen_clearing")
        );
        assert!(matrix.rows().iter().any(|row| row.category == "scrolling"));
        assert!(
            matrix
                .rows()
                .iter()
                .any(|row| row.category == "character_attributes")
        );
        assert!(
            matrix
                .rows()
                .iter()
                .any(|row| row.category == "alternate_screen")
        );
        assert!(
            matrix
                .rows()
                .iter()
                .any(|row| row.category == "resize_reflow")
        );
        assert!(
            matrix
                .rows()
                .iter()
                .any(|row| row.source_reference.kind == M4SourceReferenceKind::Esctest2)
        );
        assert!(
            matrix
                .rows()
                .iter()
                .filter(|row| row.status == M4CompatibilityStatus::Implemented)
                .all(|row| row.platform_measurements.linux
                    == M4PlatformMeasurementStatus::NotMeasured
                    && row.platform_measurements.macos == M4PlatformMeasurementStatus::NotMeasured)
        );
        matrix
            .validate_referenced_artifacts(&path, &root)
            .expect("linked fixtures should exist");
    }

    #[test]
    fn implemented_rows_require_fixture_artifacts() {
        let json = valid_matrix_json().replace(
            r#""artifacts": [{
                "kind": "fixture",
                "path": "crates/terminal-fixtures/fixtures/m1-golden.json",
                "name": "sgr-reset"
              }]"#,
            r#""artifacts": []"#,
        );

        let error = M4CompatibilityMatrix::from_json_str("matrix.json", &json)
            .expect_err("implemented row without artifact must fail");

        assert!(matches!(
            error,
            FixtureLoadError::InvalidSchema { field, .. }
                if field == "rows[0].fixture_coverage.artifacts"
        ));
    }

    #[test]
    fn omitted_platform_field_fails_schema_validation() {
        let json = valid_matrix_json().replace(
            r#""linux": "not_measured",
              "#,
            "",
        );

        let error = M4CompatibilityMatrix::from_json_str("matrix.json", &json)
            .expect_err("missing platform field must fail");

        assert!(matches!(
            error,
            FixtureLoadError::InvalidSchema { field, .. }
                if field == "rows[0].platform_measurements.linux"
        ));
    }

    #[test]
    fn linked_fixture_name_must_exist() {
        let root = workspace_root();
        let json = valid_matrix_json().replace("sgr-reset", "missing-fixture");
        let matrix = M4CompatibilityMatrix::from_json_str("matrix.json", &json)
            .expect("schema should load before artifact inspection");

        let error = matrix
            .validate_referenced_artifacts("matrix.json", &root)
            .expect_err("missing fixture name must fail");

        assert!(matches!(
            error,
            FixtureLoadError::InvalidSchema { field, .. }
                if field == "rows[0].fixture_coverage.artifacts[0].name"
        ));
    }
}
