use std::fs;
use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{FixtureLoadError, invalid_schema, recording_schema_error};

pub const M5_PANEFLOW_DOGFOOD_SCHEMA: &str = "hera.m5_paneflow_shadow_dogfood";
pub const M5_PANEFLOW_DOGFOOD_VERSION: u32 = 1;
pub const M5_MAX_DOGFOOD_REPORT_BYTES: u64 = 1024 * 1024;

const PRIVATE_PATTERNS: &[&str] = &[
    "C:\\Users\\",
    "C:/Users/",
    "\\Users\\",
    "%USERPROFILE%",
    "/home/",
    "OPENAI_API_KEY",
    "ANTHROPIC_API_KEY",
    "BEGIN OPENSSH PRIVATE KEY",
    "BEGIN RSA PRIVATE KEY",
    "sk-",
    "ghp_",
    "github_pat_",
    "xoxb-",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5PaneflowDogfoodReport {
    pub schema: String,
    pub version: u32,
    pub generated_at: String,
    pub source_command: String,
    pub status: M5DogfoodStatus,
    pub scenario: M5DogfoodScenario,
    pub mismatch_summary: M5DogfoodMismatchSummary,
    pub retention: M5DogfoodRetention,
    pub replacement_blocked: bool,
    #[serde(default)]
    pub notes: Vec<String>,
}

impl M5PaneflowDogfoodReport {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref();
        let raw = read_report_file_capped(path)?;
        Self::from_json_str(path, &raw)
    }

    pub fn from_json_str(path: impl AsRef<Path>, json: &str) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref().to_path_buf();
        if json.len() as u64 > M5_MAX_DOGFOOD_REPORT_BYTES {
            return Err(invalid_schema(
                &path,
                "$",
                format!(
                    "M5 dogfood report JSON is {} bytes, maximum is {M5_MAX_DOGFOOD_REPORT_BYTES}",
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

    pub fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        if self.schema != M5_PANEFLOW_DOGFOOD_SCHEMA {
            return Err(invalid_schema(
                path,
                "schema",
                format!("expected {M5_PANEFLOW_DOGFOOD_SCHEMA}"),
            ));
        }
        if self.version != M5_PANEFLOW_DOGFOOD_VERSION {
            return Err(invalid_schema(
                path,
                "version",
                format!("expected {M5_PANEFLOW_DOGFOOD_VERSION}"),
            ));
        }
        validate_timestamp(path, "generated_at", &self.generated_at)?;
        validate_required_string(path, "source_command", &self.source_command)?;
        if !self.source_command.contains("PANEFLOW_HERA_DOGFOOD=shadow") {
            return Err(invalid_schema(
                path,
                "source_command",
                "dogfood report must name PANEFLOW_HERA_DOGFOOD=shadow",
            ));
        }
        self.scenario.validate(path, "scenario")?;
        self.mismatch_summary
            .validate(path, "mismatch_summary", self.status)?;
        self.retention.validate(path, "retention")?;
        for (index, note) in self.notes.iter().enumerate() {
            validate_required_string(path, format!("notes[{index}]"), note)?;
        }
        self.scan_public_content(path)
    }

    fn scan_public_content(&self, path: &Path) -> Result<(), FixtureLoadError> {
        let mut values = vec![
            ("source_command".to_owned(), self.source_command.as_str()),
            (
                "scenario.artifact_directory".to_owned(),
                self.scenario.artifact_directory.as_str(),
            ),
            (
                "retention.raw_local_artifact_location".to_owned(),
                self.retention.raw_local_artifact_location.as_str(),
            ),
        ];
        for (index, class) in self.scenario.command_classes.iter().enumerate() {
            values.push((format!("scenario.command_classes[{index}]"), class.as_str()));
        }
        for (index, note) in self.notes.iter().enumerate() {
            values.push((format!("notes[{index}]"), note.as_str()));
        }
        for (index, mismatch) in self
            .mismatch_summary
            .sanitized_mismatches
            .iter()
            .enumerate()
        {
            values.push((
                format!("mismatch_summary.sanitized_mismatches[{index}].summary"),
                mismatch.summary.as_str(),
            ));
            values.push((
                format!("mismatch_summary.sanitized_mismatches[{index}].reproduction_pointer"),
                mismatch.reproduction_pointer.as_str(),
            ));
        }

        for (field, value) in values {
            let value_lower = value.to_ascii_lowercase();
            for pattern in PRIVATE_PATTERNS {
                let pattern_lower = pattern.to_ascii_lowercase();
                if value.contains(pattern) || value_lower.contains(&pattern_lower) {
                    return Err(invalid_schema(
                        path,
                        field,
                        format!("dogfood public summary contains rejected pattern {pattern:?}"),
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5DogfoodStatus {
    TargetedPass,
    Failed,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5DogfoodScenario {
    pub mode: M5DogfoodMode,
    pub duration_minutes: u32,
    pub pane_count: u32,
    pub command_classes: Vec<String>,
    pub artifact_directory: String,
}

impl M5DogfoodScenario {
    fn validate(&self, path: &Path, field: &str) -> Result<(), FixtureLoadError> {
        if self.mode != M5DogfoodMode::Shadow {
            return Err(invalid_schema(
                path,
                format!("{field}.mode"),
                "M5 dogfood scenario must run in shadow mode",
            ));
        }
        if self.duration_minutes == 0 {
            return Err(invalid_schema(
                path,
                format!("{field}.duration_minutes"),
                "duration must be non-zero",
            ));
        }
        if self.pane_count == 0 {
            return Err(invalid_schema(
                path,
                format!("{field}.pane_count"),
                "pane count must be non-zero",
            ));
        }
        if self.command_classes.is_empty() {
            return Err(invalid_schema(
                path,
                format!("{field}.command_classes"),
                "at least one command class is required",
            ));
        }
        for (index, class) in self.command_classes.iter().enumerate() {
            validate_required_string(path, format!("{field}.command_classes[{index}]"), class)?;
        }
        validate_repo_path(
            path,
            format!("{field}.artifact_directory"),
            &self.artifact_directory,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5DogfoodMode {
    Shadow,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5DogfoodMismatchSummary {
    pub total: u32,
    pub p0: u32,
    pub p1: u32,
    pub p2: u32,
    pub sanitized_mismatches: Vec<M5DogfoodMismatch>,
}

impl M5DogfoodMismatchSummary {
    fn validate(
        &self,
        path: &Path,
        field: &str,
        status: M5DogfoodStatus,
    ) -> Result<(), FixtureLoadError> {
        let computed = self.p0.saturating_add(self.p1).saturating_add(self.p2);
        if self.total != computed {
            return Err(invalid_schema(
                path,
                format!("{field}.total"),
                "total must equal p0 + p1 + p2",
            ));
        }
        if status == M5DogfoodStatus::TargetedPass && self.p0 != 0 {
            return Err(invalid_schema(
                path,
                format!("{field}.p0"),
                "targeted_pass requires zero P0 mismatches",
            ));
        }
        if self.total as usize != self.sanitized_mismatches.len() {
            return Err(invalid_schema(
                path,
                format!("{field}.sanitized_mismatches"),
                "every mismatch count must have a sanitized summary",
            ));
        }
        for (index, mismatch) in self.sanitized_mismatches.iter().enumerate() {
            mismatch.validate(path, &format!("{field}.sanitized_mismatches[{index}]"))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5DogfoodMismatch {
    pub severity: M5DogfoodMismatchSeverity,
    pub summary: String,
    pub reproduction_pointer: String,
}

impl M5DogfoodMismatch {
    fn validate(&self, path: &Path, field: &str) -> Result<(), FixtureLoadError> {
        validate_required_string(path, format!("{field}.summary"), &self.summary)?;
        validate_repo_path(
            path,
            format!("{field}.reproduction_pointer"),
            &self.reproduction_pointer,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5DogfoodMismatchSeverity {
    P0,
    P1,
    P2,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5DogfoodRetention {
    pub raw_local_retained_outside_repo: bool,
    pub raw_local_artifact_location: String,
    pub public_artifacts_only_scrubbed: bool,
}

impl M5DogfoodRetention {
    fn validate(&self, path: &Path, field: &str) -> Result<(), FixtureLoadError> {
        if !self.raw_local_retained_outside_repo {
            return Err(invalid_schema(
                path,
                format!("{field}.raw_local_retained_outside_repo"),
                "raw local bytes must remain outside the Hera repo",
            ));
        }
        validate_required_string(
            path,
            format!("{field}.raw_local_artifact_location"),
            &self.raw_local_artifact_location,
        )?;
        if !self.public_artifacts_only_scrubbed {
            return Err(invalid_schema(
                path,
                format!("{field}.public_artifacts_only_scrubbed"),
                "public dogfood artifacts must be scrubbed summaries only",
            ));
        }
        Ok(())
    }
}

#[must_use]
pub fn m5_default_paneflow_dogfood_report(generated_at: &str) -> M5PaneflowDogfoodReport {
    M5PaneflowDogfoodReport {
        schema: M5_PANEFLOW_DOGFOOD_SCHEMA.to_owned(),
        version: M5_PANEFLOW_DOGFOOD_VERSION,
        generated_at: generated_at.to_owned(),
        source_command: "manual: run Paneflow with PANEFLOW_HERA_DOGFOOD=shadow and PANEFLOW_HERA_DOGFOOD_RETENTION=summary".to_owned(),
        status: M5DogfoodStatus::Blocked,
        scenario: M5DogfoodScenario {
            mode: M5DogfoodMode::Shadow,
            duration_minutes: 45,
            pane_count: 2,
            command_classes: vec![
                "codex_cli_agent".to_owned(),
                "claude_code_agent".to_owned(),
                "cargo_test".to_owned(),
                "rapid_output".to_owned(),
                "long_session_replay".to_owned(),
            ],
            artifact_directory: "evidence/m5/dogfood".to_owned(),
        },
        mismatch_summary: M5DogfoodMismatchSummary {
            total: 0,
            p0: 0,
            p1: 0,
            p2: 0,
            sanitized_mismatches: Vec::new(),
        },
        retention: M5DogfoodRetention {
            raw_local_retained_outside_repo: true,
            raw_local_artifact_location: "outside_repo_local_only".to_owned(),
            public_artifacts_only_scrubbed: true,
        },
        replacement_blocked: true,
        notes: vec![
            "Blocked state: the checked-in public summary defines the shadow scenario shape and mismatch severity contract, but does not assert that a live Paneflow shadow run happened.".to_owned(),
            "Raw local terminal bytes must stay outside Hera; replace this artifact with a scrubbed summary after the live run.".to_owned(),
            "Host replacement remains blocked until dogfood, platform, package and final M5 evidence stories pass.".to_owned(),
        ],
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

fn validate_repo_path(
    path: &Path,
    field: impl Into<String>,
    value: &str,
) -> Result<(), FixtureLoadError> {
    let field = field.into();
    validate_required_string(path, field.clone(), value)?;
    if value.contains('\\') || value.contains(':') {
        return Err(invalid_schema(
            path,
            field,
            "paths must be relative slash-separated repo paths",
        ));
    }
    let candidate = Path::new(value);
    if candidate.is_absolute() {
        return Err(invalid_schema(path, field, "paths must not be absolute"));
    }
    if value
        .split('/')
        .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(invalid_schema(
            path,
            field,
            "path contains an empty or parent segment",
        ));
    }
    Ok(())
}

fn read_report_file_capped(path: &Path) -> Result<String, FixtureLoadError> {
    let metadata = fs::metadata(path).map_err(|error| FixtureLoadError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    if !metadata.is_file() {
        return Err(invalid_schema(
            path,
            "$",
            "M5 dogfood report path must be a regular file",
        ));
    }
    if metadata.len() > M5_MAX_DOGFOOD_REPORT_BYTES {
        return Err(invalid_schema(
            path,
            "$",
            format!(
                "M5 dogfood report file is {} bytes, maximum is {M5_MAX_DOGFOOD_REPORT_BYTES}",
                metadata.len()
            ),
        ));
    }

    let file = fs::File::open(path).map_err(|error| FixtureLoadError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    let mut reader = file.take(M5_MAX_DOGFOOD_REPORT_BYTES + 1);
    let mut raw = String::new();
    reader
        .read_to_string(&mut raw)
        .map_err(|error| FixtureLoadError::Io {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    if raw.len() as u64 > M5_MAX_DOGFOOD_REPORT_BYTES {
        return Err(invalid_schema(
            path,
            "$",
            format!(
                "M5 dogfood report exceeded maximum of {M5_MAX_DOGFOOD_REPORT_BYTES} bytes while reading"
            ),
        ));
    }
    Ok(raw)
}

#[cfg(test)]
mod tests {
    use super::{
        M5DogfoodMismatch, M5DogfoodMismatchSeverity, M5DogfoodStatus, M5PaneflowDogfoodReport,
        m5_default_paneflow_dogfood_report,
    };

    #[test]
    fn default_paneflow_dogfood_report_validates() {
        let report = m5_default_paneflow_dogfood_report("2026-07-04T00:00:00Z");
        report
            .validate(std::path::Path::new("dogfood.json"))
            .expect("default report should validate");
    }

    #[test]
    fn targeted_pass_rejects_p0_mismatch() {
        let mut report = m5_default_paneflow_dogfood_report("2026-07-04T00:00:00Z");
        report.status = M5DogfoodStatus::TargetedPass;
        report.mismatch_summary.total = 1;
        report.mismatch_summary.p0 = 1;
        report
            .mismatch_summary
            .sanitized_mismatches
            .push(M5DogfoodMismatch {
                severity: M5DogfoodMismatchSeverity::P0,
                summary: "primary screen diverged".to_owned(),
                reproduction_pointer: "evidence/m5/dogfood/mismatch.json".to_owned(),
            });

        let error = report
            .validate(std::path::Path::new("dogfood.json"))
            .expect_err("targeted pass cannot include P0 mismatches");
        assert!(error.to_string().contains("targeted_pass requires zero P0"));
    }

    #[test]
    fn dogfood_public_summary_rejects_private_paths() {
        let mut report = m5_default_paneflow_dogfood_report("2026-07-04T00:00:00Z");
        report.notes.push("C:\\Users\\Arthur\\secret".to_owned());
        let json = serde_json::to_string(&report).expect("report should serialize");

        let error = M5PaneflowDogfoodReport::from_json_str("dogfood.json", &json)
            .expect_err("private path must be rejected");
        assert!(error.to_string().contains("rejected pattern"));
    }
}
