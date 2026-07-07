use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{FixtureLoadError, invalid_schema, recording_schema_error};

pub const M5_BASELINE_SCHEMA: &str = "hera.m5_baseline";
pub const M5_BASELINE_VERSION: u32 = 1;
pub const M5_MAX_BASELINE_BYTES: u64 = 1024 * 1024;

const REQUIRED_BLOCKERS: [&str; 8] = [
    "vt.cursor.csi_positioning",
    "vt.screen.ed_el_ech",
    "xterm.private_modes.47_1047_1048",
    "replay.real_session_derivatives",
    "platform.linux_macos_measurement",
    "release.package_metadata",
    "release.publish_order",
    "security.openssf_scorecard",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5Baseline {
    pub schema: String,
    pub version: u32,
    pub generated_at: String,
    pub m5_status: M5BaselineStatus,
    pub source_milestone: M5SourceMilestone,
    pub dependencies: Vec<M5BaselineDependency>,
    pub blockers: Vec<M5BaselineBlocker>,
}

impl M5Baseline {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref();
        let raw = read_text_file_capped(path, M5_MAX_BASELINE_BYTES, "M5 baseline")?;
        Self::from_json_str(path, &raw)
    }

    pub fn from_json_str(path: impl AsRef<Path>, json: &str) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref().to_path_buf();
        if json.len() as u64 > M5_MAX_BASELINE_BYTES {
            return Err(invalid_schema(
                &path,
                "$",
                format!(
                    "M5 baseline JSON is {} bytes, maximum is {M5_MAX_BASELINE_BYTES}",
                    json.len()
                ),
            ));
        }

        let mut deserializer = serde_json::Deserializer::from_str(json);
        let baseline: Self =
            serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
                recording_schema_error(&path, &error.path().to_string(), &error.inner().to_string())
            })?;
        baseline.validate(&path)?;
        Ok(baseline)
    }

    pub fn blockers(&self) -> &[M5BaselineBlocker] {
        &self.blockers
    }

    pub fn dependencies(&self) -> &[M5BaselineDependency] {
        &self.dependencies
    }

    fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        if self.schema != M5_BASELINE_SCHEMA {
            return Err(invalid_schema(
                path,
                "schema",
                format!("expected {M5_BASELINE_SCHEMA}"),
            ));
        }
        if self.version != M5_BASELINE_VERSION {
            return Err(invalid_schema(
                path,
                "version",
                format!("expected {M5_BASELINE_VERSION}"),
            ));
        }
        validate_timestamp(path, "generated_at", &self.generated_at)?;
        if self.m5_status != M5BaselineStatus::Ready {
            return Err(invalid_schema(
                path,
                "m5_status",
                "EP-001 baseline generation must keep M5 status READY",
            ));
        }
        self.source_milestone.validate(path)?;

        if self.dependencies.is_empty() {
            return Err(invalid_schema(
                path,
                "dependencies",
                "at least one source dependency is required",
            ));
        }
        let mut dependency_ids = BTreeSet::new();
        for (index, dependency) in self.dependencies.iter().enumerate() {
            dependency.validate(path, index)?;
            if !dependency_ids.insert(dependency.id.as_str()) {
                return Err(invalid_schema(
                    path,
                    format!("dependencies[{index}].id"),
                    "dependency id must be unique",
                ));
            }
        }

        let mut blocker_ids = BTreeSet::new();
        for (index, blocker) in self.blockers.iter().enumerate() {
            blocker.validate(path, index)?;
            if !blocker_ids.insert(blocker.id.as_str()) {
                return Err(invalid_schema(
                    path,
                    format!("blockers[{index}].id"),
                    "blocker id must be unique",
                ));
            }
        }
        for required in REQUIRED_BLOCKERS {
            if !blocker_ids.contains(required) {
                return Err(invalid_schema(
                    path,
                    "blockers",
                    format!("missing required M5 baseline blocker {required}"),
                ));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum M5BaselineStatus {
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5SourceMilestone {
    pub id: String,
    pub status_path: String,
    pub prd_status: String,
    pub done_status_referenced: bool,
    pub history_modified: bool,
}

impl M5SourceMilestone {
    fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        validate_required_string(path, "source_milestone.id", &self.id)?;
        validate_repo_path(path, "source_milestone.status_path", &self.status_path)?;
        validate_required_string(path, "source_milestone.prd_status", &self.prd_status)?;
        if self.prd_status == "DONE" && !self.done_status_referenced {
            return Err(invalid_schema(
                path,
                "source_milestone.done_status_referenced",
                "M4 DONE status must be referenced",
            ));
        }
        if self.history_modified {
            return Err(invalid_schema(
                path,
                "source_milestone.history_modified",
                "M5 baseline must not modify M4 history",
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5BaselineDependency {
    pub id: String,
    pub path: String,
    pub status: M5BaselineDependencyStatus,
    #[serde(default)]
    pub reason: Option<String>,
}

impl M5BaselineDependency {
    fn validate(&self, path: &Path, index: usize) -> Result<(), FixtureLoadError> {
        validate_required_string(path, format!("dependencies[{index}].id"), &self.id)?;
        validate_repo_path(path, format!("dependencies[{index}].path"), &self.path)?;
        if self.status.requires_reason()
            && self
                .reason
                .as_deref()
                .is_none_or(|reason| reason.trim().is_empty())
        {
            return Err(invalid_schema(
                path,
                format!("dependencies[{index}].reason"),
                "blocked dependencies must include a reason",
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5BaselineDependencyStatus {
    Available,
    Missing,
    Malformed,
}

impl M5BaselineDependencyStatus {
    const fn requires_reason(self) -> bool {
        matches!(self, Self::Missing | Self::Malformed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5BaselineBlocker {
    pub id: String,
    pub title: String,
    pub category: String,
    pub source_path: String,
    pub source_status: String,
    pub m5_disposition: M5BaselineDisposition,
    pub owner: String,
    pub m5_story: String,
    #[serde(default)]
    pub current_evidence: Vec<M5CurrentEvidence>,
    #[serde(default)]
    pub blocked_reason: Option<String>,
    pub notes: Vec<String>,
}

impl M5BaselineBlocker {
    fn validate(&self, path: &Path, index: usize) -> Result<(), FixtureLoadError> {
        validate_required_string(path, format!("blockers[{index}].id"), &self.id)?;
        validate_required_string(path, format!("blockers[{index}].title"), &self.title)?;
        validate_required_string(path, format!("blockers[{index}].category"), &self.category)?;
        validate_repo_path(
            path,
            format!("blockers[{index}].source_path"),
            &self.source_path,
        )?;
        validate_required_string(
            path,
            format!("blockers[{index}].source_status"),
            &self.source_status,
        )?;
        validate_required_string(path, format!("blockers[{index}].owner"), &self.owner)?;
        validate_required_string(path, format!("blockers[{index}].m5_story"), &self.m5_story)?;
        validate_string_list(path, format!("blockers[{index}].notes"), &self.notes)?;

        if self.m5_disposition == M5BaselineDisposition::CurrentEvidence
            && self.current_evidence.is_empty()
        {
            return Err(invalid_schema(
                path,
                format!("blockers[{index}].current_evidence"),
                "current_evidence disposition must link evidence",
            ));
        }
        if self.m5_disposition == M5BaselineDisposition::BlockedDependency
            && self
                .blocked_reason
                .as_deref()
                .is_none_or(|reason| reason.trim().is_empty())
        {
            return Err(invalid_schema(
                path,
                format!("blockers[{index}].blocked_reason"),
                "blocked dependency disposition must include a reason",
            ));
        }
        if self.m5_disposition.requires_note() && self.notes.is_empty() {
            return Err(invalid_schema(
                path,
                format!("blockers[{index}].notes"),
                "carried or deferred blockers must explain the M5 treatment",
            ));
        }
        for (evidence_index, evidence) in self.current_evidence.iter().enumerate() {
            evidence.validate(
                path,
                &format!("blockers[{index}].current_evidence[{evidence_index}]"),
            )?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5BaselineDisposition {
    CurrentEvidence,
    CarriedForward,
    BlockedDependency,
    DeferredPolicy,
}

impl M5BaselineDisposition {
    const fn requires_note(self) -> bool {
        matches!(self, Self::CarriedForward | Self::DeferredPolicy)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5CurrentEvidence {
    pub path: String,
    pub status: String,
    pub summary: String,
}

impl M5CurrentEvidence {
    fn validate(&self, path: &Path, field: &str) -> Result<(), FixtureLoadError> {
        validate_repo_path(path, format!("{field}.path"), &self.path)?;
        validate_required_string(path, format!("{field}.status"), &self.status)?;
        validate_required_string(path, format!("{field}.summary"), &self.summary)
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
    manifest_path: &Path,
    field: impl Into<String>,
    value: &str,
) -> Result<(), FixtureLoadError> {
    let field = field.into();
    validate_required_string(manifest_path, field.clone(), value)?;
    if value.contains('\\') || value.contains(':') {
        return Err(invalid_schema(
            manifest_path,
            field,
            "paths must be relative slash-separated repo paths",
        ));
    }
    let path = Path::new(value);
    if path.is_absolute() {
        return Err(invalid_schema(
            manifest_path,
            field,
            "paths must not be absolute",
        ));
    }
    if value
        .split('/')
        .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(invalid_schema(
            manifest_path,
            field,
            "path contains an empty or parent segment",
        ));
    }

    Ok(())
}

fn read_text_file_capped(
    path: &Path,
    max_bytes: u64,
    label: &'static str,
) -> Result<String, FixtureLoadError> {
    let metadata = fs::metadata(path).map_err(|error| FixtureLoadError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    if !metadata.is_file() {
        return Err(invalid_schema(
            path,
            "$",
            format!("{label} path must be a regular file"),
        ));
    }
    if metadata.len() > max_bytes {
        return Err(invalid_schema(
            path,
            "$",
            format!(
                "{label} file is {} bytes, maximum is {max_bytes}",
                metadata.len()
            ),
        ));
    }

    let file = fs::File::open(path).map_err(|error| FixtureLoadError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    let mut reader = file.take(max_bytes + 1);
    let mut raw = String::new();
    reader
        .read_to_string(&mut raw)
        .map_err(|error| FixtureLoadError::Io {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;

    if raw.len() as u64 > max_bytes {
        return Err(invalid_schema(
            path,
            "$",
            format!("{label} file exceeded maximum of {max_bytes} bytes while reading"),
        ));
    }

    Ok(raw)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{M5Baseline, M5BaselineDisposition, M5BaselineStatus};
    use crate::FixtureLoadError;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn valid_baseline_json() -> &'static str {
        r#"{
          "schema": "hera.m5_baseline",
          "version": 1,
          "generated_at": "2026-07-04T00:00:00Z",
          "m5_status": "READY",
          "source_milestone": {
            "id": "M4",
            "status_path": "tasks/prd-m4-public-proof-status.json",
            "prd_status": "DONE",
            "done_status_referenced": true,
            "history_modified": false
          },
          "dependencies": [{
            "id": "m4_report",
            "path": "docs/m4-public-proof-report.md",
            "status": "available"
          }],
          "blockers": [
            {
              "id": "vt.cursor.csi_positioning",
              "title": "CSI cursor positioning",
              "category": "compatibility",
              "source_path": "evidence/m4/compatibility-matrix.json",
              "source_status": "not_implemented",
              "m5_disposition": "carried_forward",
              "owner": "terminal-core",
              "m5_story": "US-004",
              "notes": ["Needs M5 fixtures."]
            },
            {
              "id": "vt.screen.ed_el_ech",
              "title": "ED, EL and ECH erasure",
              "category": "compatibility",
              "source_path": "evidence/m4/compatibility-matrix.json",
              "source_status": "not_implemented",
              "m5_disposition": "carried_forward",
              "owner": "terminal-core",
              "m5_story": "US-005",
              "notes": ["Needs M5 fixtures."]
            },
            {
              "id": "xterm.private_modes.47_1047_1048",
              "title": "DEC private modes 47, 1047 and 1048",
              "category": "compatibility",
              "source_path": "evidence/m4/compatibility-matrix.json",
              "source_status": "not_implemented",
              "m5_disposition": "carried_forward",
              "owner": "terminal-core",
              "m5_story": "US-006",
              "notes": ["Needs policy or fixtures."]
            },
            {
              "id": "replay.real_session_derivatives",
              "title": "Real-session replay derivatives",
              "category": "replay",
              "source_path": "docs/m4-public-proof-report.md",
              "source_status": "blocked",
              "m5_disposition": "carried_forward",
              "owner": "terminal-fixtures",
              "m5_story": "US-008",
              "notes": ["Needs public-safe derivatives."]
            },
            {
              "id": "platform.linux_macos_measurement",
              "title": "Linux and macOS measurement",
              "category": "platform",
              "source_path": "docs/m4-public-proof-report.md",
              "source_status": "not_measured",
              "m5_disposition": "carried_forward",
              "owner": "terminal-cli",
              "m5_story": "US-013",
              "notes": ["Needs measured or blocked platform rows."]
            },
            {
              "id": "release.package_metadata",
              "title": "Package metadata",
              "category": "release",
              "source_path": "evidence/m4/m4-package-readiness.json",
              "source_status": "partial",
              "m5_disposition": "carried_forward",
              "owner": "workspace",
              "m5_story": "US-014",
              "notes": ["Metadata is incomplete."]
            },
            {
              "id": "release.publish_order",
              "title": "Publish order",
              "category": "release",
              "source_path": "evidence/m4/m4-package-readiness.json",
              "source_status": "blocked",
              "m5_disposition": "carried_forward",
              "owner": "workspace",
              "m5_story": "US-015",
              "notes": ["Internal publish order is unresolved."]
            },
            {
              "id": "security.openssf_scorecard",
              "title": "OpenSSF Scorecard",
              "category": "security",
              "source_path": "evidence/m4/m4-oss-security-baseline.json",
              "source_status": "not_measured",
              "m5_disposition": "carried_forward",
              "owner": "terminal-cli",
              "m5_story": "US-017",
              "notes": ["External Scorecard is not measured."]
            }
          ]
        }"#
    }

    #[test]
    fn m5_baseline_validates_checked_in_baseline() {
        let root = workspace_root();
        let path = root.join("evidence/m5/m5-baseline.json");
        let baseline = M5Baseline::from_path(&path).expect("M5 baseline should load");

        assert_eq!(baseline.m5_status, M5BaselineStatus::Ready);
        assert_eq!(baseline.source_milestone.prd_status, "DONE");
        assert!(!baseline.source_milestone.history_modified);
        assert!(
            baseline
                .blockers()
                .iter()
                .any(|blocker| blocker.id == "vt.cursor.csi_positioning")
        );
        assert!(
            baseline
                .dependencies()
                .iter()
                .all(|dependency| !dependency.path.contains('\\'))
        );
    }

    #[test]
    fn m5_baseline_rejects_missing_required_blocker() {
        let json = valid_baseline_json().replace(
            "\"id\": \"security.openssf_scorecard\"",
            "\"id\": \"security.openssf_scorecard_missing\"",
        );

        let error = M5Baseline::from_json_str("baseline.json", &json)
            .expect_err("required blocker must be present");

        assert!(matches!(
            error,
            FixtureLoadError::InvalidSchema { field, .. } if field == "blockers"
        ));
    }

    #[test]
    fn m5_baseline_rejects_m4_history_mutation() {
        let json = valid_baseline_json()
            .replace("\"history_modified\": false", "\"history_modified\": true");

        let error = M5Baseline::from_json_str("baseline.json", &json)
            .expect_err("M4 history must not be modified");

        assert!(matches!(
            error,
            FixtureLoadError::InvalidSchema { field, .. }
                if field == "source_milestone.history_modified"
        ));
    }

    #[test]
    fn m5_baseline_current_evidence_requires_link() {
        let json = valid_baseline_json().replace(
            "\"m5_disposition\": \"carried_forward\"",
            "\"m5_disposition\": \"current_evidence\"",
        );

        let error = M5Baseline::from_json_str("baseline.json", &json)
            .expect_err("current evidence disposition needs evidence");

        assert!(matches!(
            error,
            FixtureLoadError::InvalidSchema { field, .. }
                if field == "blockers[0].current_evidence"
        ));
    }

    #[test]
    fn m5_baseline_disposition_roundtrips() {
        let baseline =
            M5Baseline::from_json_str("baseline.json", valid_baseline_json()).expect("valid");

        assert_eq!(
            baseline.blockers()[0].m5_disposition,
            M5BaselineDisposition::CarriedForward
        );
    }
}
