use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{FixtureLoadError, invalid_schema, recording_schema_error};

pub const M5_SECURITY_BASELINE_SCHEMA: &str = "hera.m5_security_baseline";
pub const M5_SECURITY_BASELINE_VERSION: u32 = 1;
pub const M5_MAX_SECURITY_BASELINE_BYTES: u64 = 1024 * 1024;

const REQUIRED_TOOLS: [M5SecurityToolId; 3] = [
    M5SecurityToolId::CargoAudit,
    M5SecurityToolId::CargoDeny,
    M5SecurityToolId::OpenSsfScorecard,
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5SecurityBaseline {
    pub schema: String,
    pub version: u32,
    pub generated_at: String,
    pub source_command: String,
    pub status: M5SecurityBaselineStatus,
    pub tools: Vec<M5SecurityToolCheck>,
    pub summary: M5SecuritySummary,
    #[serde(default)]
    pub notes: Vec<String>,
}

impl M5SecurityBaseline {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref();
        let raw =
            read_text_file_capped(path, M5_MAX_SECURITY_BASELINE_BYTES, "M5 security baseline")?;
        Self::from_json_str(path, &raw)
    }

    pub fn from_json_str(path: impl AsRef<Path>, json: &str) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref().to_path_buf();
        if json.len() as u64 > M5_MAX_SECURITY_BASELINE_BYTES {
            return Err(invalid_schema(
                &path,
                "$",
                format!(
                    "M5 security baseline JSON is {} bytes, maximum is {M5_MAX_SECURITY_BASELINE_BYTES}",
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

    pub fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        if self.schema != M5_SECURITY_BASELINE_SCHEMA {
            return Err(invalid_schema(
                path,
                "schema",
                format!("expected {M5_SECURITY_BASELINE_SCHEMA}"),
            ));
        }
        if self.version != M5_SECURITY_BASELINE_VERSION {
            return Err(invalid_schema(
                path,
                "version",
                format!("expected {M5_SECURITY_BASELINE_VERSION}"),
            ));
        }
        validate_timestamp(path, "generated_at", &self.generated_at)?;
        validate_required_string(path, "source_command", &self.source_command)?;
        validate_string_list(path, "notes", &self.notes)?;

        let mut ids = BTreeSet::new();
        for (index, tool) in self.tools.iter().enumerate() {
            tool.validate(path, &format!("tools[{index}]"))?;
            if !ids.insert(tool.id) {
                return Err(invalid_schema(
                    path,
                    format!("tools[{index}].id"),
                    "security tool ids must be unique",
                ));
            }
        }
        for required in REQUIRED_TOOLS {
            if !ids.contains(&required) {
                return Err(invalid_schema(
                    path,
                    "tools",
                    format!("missing required security tool {required:?}"),
                ));
            }
        }

        self.summary.validate(path, &self.tools)?;

        let has_failed = self
            .tools
            .iter()
            .any(|tool| tool.status == M5SecurityCheckStatus::Failed);
        let has_blocked = self
            .tools
            .iter()
            .any(|tool| tool.status == M5SecurityCheckStatus::Blocked);
        let has_release_blocker = self.tools.iter().any(|tool| tool.release_blocking);

        match self.status {
            M5SecurityBaselineStatus::Pass if has_failed || has_blocked || has_release_blocker => {
                Err(invalid_schema(
                    path,
                    "status",
                    "passing security baseline cannot contain failed, blocked or release-blocking tool rows",
                ))
            }
            M5SecurityBaselineStatus::Failed if !has_failed && !has_release_blocker => {
                Err(invalid_schema(
                    path,
                    "status",
                    "failed security baseline must contain a failed tool or release-blocking finding",
                ))
            }
            M5SecurityBaselineStatus::Blocked if has_failed || has_release_blocker => {
                Err(invalid_schema(
                    path,
                    "status",
                    "blocked security baseline must not hide failed or release-blocking findings",
                ))
            }
            M5SecurityBaselineStatus::Blocked if !has_blocked => Err(invalid_schema(
                path,
                "status",
                "blocked security baseline must contain a blocked tool row",
            )),
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5SecuritySummary {
    pub passed_tools: u32,
    pub blocked_tools: u32,
    pub failed_tools: u32,
    pub release_blocking_findings: u32,
}

impl M5SecuritySummary {
    fn validate(&self, path: &Path, tools: &[M5SecurityToolCheck]) -> Result<(), FixtureLoadError> {
        let passed = tools
            .iter()
            .filter(|tool| tool.status == M5SecurityCheckStatus::Pass)
            .count() as u32;
        let blocked = tools
            .iter()
            .filter(|tool| tool.status == M5SecurityCheckStatus::Blocked)
            .count() as u32;
        let failed = tools
            .iter()
            .filter(|tool| tool.status == M5SecurityCheckStatus::Failed)
            .count() as u32;
        let release_blocking = tools
            .iter()
            .flat_map(|tool| tool.findings.iter())
            .filter(|finding| finding.release_blocking)
            .count() as u32;

        if self.passed_tools != passed {
            return Err(invalid_schema(
                path,
                "summary.passed_tools",
                format!("expected {passed}"),
            ));
        }
        if self.blocked_tools != blocked {
            return Err(invalid_schema(
                path,
                "summary.blocked_tools",
                format!("expected {blocked}"),
            ));
        }
        if self.failed_tools != failed {
            return Err(invalid_schema(
                path,
                "summary.failed_tools",
                format!("expected {failed}"),
            ));
        }
        if self.release_blocking_findings != release_blocking {
            return Err(invalid_schema(
                path,
                "summary.release_blocking_findings",
                format!("expected {release_blocking}"),
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5SecurityToolCheck {
    pub id: M5SecurityToolId,
    pub tool_name: String,
    pub install_status: M5SecurityInstallStatus,
    pub attempted_command: String,
    pub status: M5SecurityCheckStatus,
    #[serde(default)]
    pub exit_code: Option<i32>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub stdout_summary: String,
    #[serde(default)]
    pub stderr_summary: String,
    #[serde(default)]
    pub blocked_reason: Option<String>,
    pub coverage: Vec<M5SecurityCoverage>,
    pub findings: Vec<M5SecurityFinding>,
    pub release_blocking: bool,
}

impl M5SecurityToolCheck {
    fn validate(&self, path: &Path, field: &str) -> Result<(), FixtureLoadError> {
        validate_required_string(path, format!("{field}.tool_name"), &self.tool_name)?;
        validate_required_string(
            path,
            format!("{field}.attempted_command"),
            &self.attempted_command,
        )?;
        if self.coverage.is_empty() {
            return Err(invalid_schema(
                path,
                format!("{field}.coverage"),
                "coverage rows are required",
            ));
        }

        let mut coverage = BTreeSet::new();
        for (index, row) in self.coverage.iter().enumerate() {
            row.validate(path, &format!("{field}.coverage[{index}]"))?;
            coverage.insert(row.category);
        }
        self.id
            .validate_required_coverage(path, &format!("{field}.coverage"), &coverage)?;

        let release_blocking_findings = self
            .findings
            .iter()
            .filter(|finding| finding.release_blocking)
            .count();
        for (index, finding) in self.findings.iter().enumerate() {
            finding.validate(path, &format!("{field}.findings[{index}]"))?;
        }
        if self.release_blocking && release_blocking_findings == 0 {
            return Err(invalid_schema(
                path,
                format!("{field}.release_blocking"),
                "release-blocking tool rows must include a release-blocking finding",
            ));
        }

        match self.status {
            M5SecurityCheckStatus::Pass => {
                if self.install_status != M5SecurityInstallStatus::Available {
                    return Err(invalid_schema(
                        path,
                        format!("{field}.install_status"),
                        "passing security tools must be available",
                    ));
                }
                if self.exit_code != Some(0) {
                    return Err(invalid_schema(
                        path,
                        format!("{field}.exit_code"),
                        "passing security tools must record exit_code 0",
                    ));
                }
                if self.release_blocking {
                    return Err(invalid_schema(
                        path,
                        format!("{field}.release_blocking"),
                        "passing security tools cannot be release-blocking",
                    ));
                }
            }
            M5SecurityCheckStatus::Failed => {
                if self.install_status != M5SecurityInstallStatus::Available {
                    return Err(invalid_schema(
                        path,
                        format!("{field}.install_status"),
                        "failed security tools must have been available and executed",
                    ));
                }
                if self.exit_code.is_none_or(|code| code == 0) {
                    return Err(invalid_schema(
                        path,
                        format!("{field}.exit_code"),
                        "failed security tools must record a non-zero exit code",
                    ));
                }
            }
            M5SecurityCheckStatus::Blocked => {
                if self.exit_code.is_some() {
                    return Err(invalid_schema(
                        path,
                        format!("{field}.exit_code"),
                        "blocked security tools must not pretend an executed exit code exists",
                    ));
                }
                if self
                    .blocked_reason
                    .as_deref()
                    .is_none_or(|reason| reason.trim().is_empty())
                    && self.stderr_summary.trim().is_empty()
                {
                    return Err(invalid_schema(
                        path,
                        format!("{field}.blocked_reason"),
                        "blocked security tools must include blocked_reason or stderr_summary",
                    ));
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5SecurityCoverage {
    pub category: M5SecurityCoverageCategory,
    pub status: M5SecurityCoverageStatus,
    pub summary: String,
}

impl M5SecurityCoverage {
    fn validate(&self, path: &Path, field: &str) -> Result<(), FixtureLoadError> {
        validate_required_string(path, format!("{field}.summary"), &self.summary)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5SecurityFinding {
    pub id: String,
    pub severity: M5SecurityFindingSeverity,
    pub release_blocking: bool,
    pub summary: String,
}

impl M5SecurityFinding {
    fn validate(&self, path: &Path, field: &str) -> Result<(), FixtureLoadError> {
        validate_required_string(path, format!("{field}.id"), &self.id)?;
        validate_required_string(path, format!("{field}.summary"), &self.summary)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5SecurityToolId {
    CargoAudit,
    CargoDeny,
    OpenSsfScorecard,
}

impl M5SecurityToolId {
    fn validate_required_coverage(
        self,
        path: &Path,
        field: &str,
        coverage: &BTreeSet<M5SecurityCoverageCategory>,
    ) -> Result<(), FixtureLoadError> {
        let required = match self {
            Self::CargoAudit => &[M5SecurityCoverageCategory::Advisories][..],
            Self::CargoDeny => &[
                M5SecurityCoverageCategory::Advisories,
                M5SecurityCoverageCategory::Licenses,
                M5SecurityCoverageCategory::Bans,
                M5SecurityCoverageCategory::DuplicateVersions,
                M5SecurityCoverageCategory::Sources,
            ][..],
            Self::OpenSsfScorecard => &[M5SecurityCoverageCategory::ScorecardChecks][..],
        };

        for category in required {
            if !coverage.contains(category) {
                return Err(invalid_schema(
                    path,
                    field,
                    format!("missing required coverage category {category:?}"),
                ));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5SecurityBaselineStatus {
    Pass,
    Blocked,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5SecurityInstallStatus {
    Available,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5SecurityCheckStatus {
    Pass,
    Failed,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5SecurityCoverageCategory {
    Advisories,
    Licenses,
    Bans,
    DuplicateVersions,
    Sources,
    ScorecardChecks,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5SecurityCoverageStatus {
    Pass,
    Failed,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5SecurityFindingSeverity {
    Critical,
    High,
    Medium,
    Low,
    Info,
    Unknown,
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

    use super::{
        M5SecurityBaseline, M5SecurityBaselineStatus, M5SecurityCoverageCategory, M5SecurityToolId,
    };
    use crate::FixtureLoadError;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn m5_security_baseline_validates_checked_in_artifact() {
        let path = workspace_root().join("evidence/m5/m5-security-baseline.json");
        let baseline =
            M5SecurityBaseline::from_path(&path).expect("M5 security baseline should load");

        assert_eq!(baseline.tools.len(), 3);
        assert!(
            matches!(
                baseline.status,
                M5SecurityBaselineStatus::Pass
                    | M5SecurityBaselineStatus::Blocked
                    | M5SecurityBaselineStatus::Failed
            ),
            "status should be one of the wire statuses"
        );
    }

    #[test]
    fn m5_security_baseline_records_cargo_deny_coverage() {
        let path = workspace_root().join("evidence/m5/m5-security-baseline.json");
        let baseline =
            M5SecurityBaseline::from_path(&path).expect("M5 security baseline should load");
        let deny = baseline
            .tools
            .iter()
            .find(|tool| tool.id == M5SecurityToolId::CargoDeny)
            .expect("cargo-deny row must exist");
        let coverage = deny
            .coverage
            .iter()
            .map(|row| row.category)
            .collect::<std::collections::BTreeSet<_>>();

        for required in [
            M5SecurityCoverageCategory::Advisories,
            M5SecurityCoverageCategory::Licenses,
            M5SecurityCoverageCategory::Bans,
            M5SecurityCoverageCategory::DuplicateVersions,
            M5SecurityCoverageCategory::Sources,
        ] {
            assert!(coverage.contains(&required), "missing {required:?}");
        }
    }

    #[test]
    fn security_baseline_rejects_missing_tool() {
        let json = r#"{
          "schema": "hera.m5_security_baseline",
          "version": 1,
          "generated_at": "2026-07-04T00:00:00Z",
          "source_command": "terminal-cli generate-m5-security-baseline --output evidence/m5/m5-security-baseline.json",
          "status": "blocked",
          "tools": [],
          "summary": {
            "passed_tools": 0,
            "blocked_tools": 0,
            "failed_tools": 0,
            "release_blocking_findings": 0
          }
        }"#;

        let error = M5SecurityBaseline::from_json_str("security.json", json)
            .expect_err("missing required tools must fail");

        assert!(matches!(
            error,
            FixtureLoadError::InvalidSchema { field, .. } if field == "tools"
        ));
    }
}
