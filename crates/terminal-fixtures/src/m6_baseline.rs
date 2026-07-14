use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{FixtureLoadError, invalid_schema, recording_schema_error};

pub const M6_BASELINE_SCHEMA: &str = "hera.m6_baseline";
pub const M6_BASELINE_VERSION: u32 = 1;
pub const M6_MAX_BASELINE_BYTES: u64 = 1024 * 1024;

const EXPECTED_M5_STATUS: &str = "DONE";
const EXPECTED_M5_STORIES: u64 = 18;
const EXPECTED_M5_STATUS_PATH: &str = "tasks/prd-m5-compatibility-release-hardening-status.json";
const EXPECTED_M5_LIVE_SUMMARY_PATH: &str = "evidence/m5/dogfood/live-gpui-summary-2026-07-10.json";
const EXPECTED_PANEFLOW_COMMIT: &str = "4129f8ac";
const EXPECTED_PANEFLOW_WORKTREE: &str = "paneflow-hera-m6";
const EXPECTED_PANEFLOW_BRANCH: &str = "feat/hera-m6-host-replacement";
const REQUIRED_OUTCOMES: [(&str, M6OutcomeStatus); 3] = [
    ("windows_visible_canary", M6OutcomeStatus::Candidate),
    ("cross_platform_canary", M6OutcomeStatus::Blocked),
    ("default_replacement", M6OutcomeStatus::Prohibited),
];
const WINDOWS_VISIBLE_CANARY_CRITERIA: [&str; 4] = [
    "M5 remains DONE with 18 completed stories",
    "The 45-minute Windows shadow run remains zero-mismatch",
    "The isolated Paneflow worktree contract matches",
    "All M6 activation, authority, fallback and default-path gates pass",
];
const CROSS_PLATFORM_CANARY_CRITERIA: [&str; 3] = [
    "The Windows visible canary passes",
    "Linux default and hera-host feature rows are measured",
    "macOS default and hera-host feature rows are measured",
];
const DEFAULT_REPLACEMENT_CRITERIA: [&str; 2] = [
    "M6 cannot authorize a default-engine change",
    "A later PRD must review cross-platform canaries and removal scope explicitly",
];
const FORBIDDEN_PUBLIC_KEYS: [&str; 6] = [
    "command",
    "prompt",
    "raw_output",
    "raw_terminal_bytes",
    "terminal_text",
    "transcript",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M6Baseline {
    pub schema: String,
    pub version: u32,
    pub generated_at: String,
    pub source_milestone: M6SourceMilestone,
    pub live_shadow: M6LiveShadowBaseline,
    pub paneflow: M6PaneflowBaseline,
    pub outcomes: Vec<M6OutcomePolicy>,
    pub privacy: M6BaselinePrivacy,
}

impl M6Baseline {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref();
        let raw = read_text_file_capped(path, M6_MAX_BASELINE_BYTES, "M6 baseline")?;
        Self::from_json_str(path, &raw)
    }

    pub fn from_json_str(path: impl AsRef<Path>, json: &str) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref().to_path_buf();
        if json.len() as u64 > M6_MAX_BASELINE_BYTES {
            return Err(invalid_schema(
                &path,
                "$",
                format!(
                    "M6 baseline JSON is {} bytes, maximum is {M6_MAX_BASELINE_BYTES}",
                    json.len()
                ),
            ));
        }

        let public_value: Value = serde_json::from_str(json)
            .map_err(|error| recording_schema_error(&path, "$", &error.to_string()))?;
        validate_public_payload(&path, "$", &public_value)?;

        let mut deserializer = serde_json::Deserializer::from_str(json);
        let baseline: Self =
            serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
                recording_schema_error(&path, &error.path().to_string(), &error.inner().to_string())
            })?;
        baseline.validate(&path)?;
        Ok(baseline)
    }

    pub fn validate_workspace(
        &self,
        baseline_path: impl AsRef<Path>,
        hera_root: impl AsRef<Path>,
        paneflow_worktree: impl AsRef<Path>,
    ) -> Result<(), FixtureLoadError> {
        let baseline_path = baseline_path.as_ref();
        let hera_root = hera_root.as_ref();
        let paneflow_worktree = paneflow_worktree.as_ref();

        let status = read_required_json(
            baseline_path,
            hera_root,
            "source_milestone.status_path",
            &self.source_milestone.status_path,
        )?;
        expect_json_string(
            baseline_path,
            "source_milestone.status",
            &status,
            &["prd", "status"],
            &self.source_milestone.status,
        )?;
        let stories = status
            .get("stories")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                invalid_schema(
                    baseline_path,
                    "source_milestone.stories_total",
                    "source status field stories must be an array",
                )
            })?;
        expect_u64(
            baseline_path,
            "source_milestone.stories_total",
            stories.len() as u64,
            self.source_milestone.stories_total,
        )?;
        let done = stories
            .iter()
            .filter(|story| story.get("status").and_then(Value::as_str) == Some("DONE"))
            .count() as u64;
        expect_u64(
            baseline_path,
            "source_milestone.stories_done",
            done,
            self.source_milestone.stories_done,
        )?;

        let live = read_required_json(
            baseline_path,
            hera_root,
            "live_shadow.summary_path",
            &self.live_shadow.summary_path,
        )?;
        expect_json_string(
            baseline_path,
            "live_shadow.status",
            &live,
            &["status"],
            &self.live_shadow.status,
        )?;
        expect_json_bool(
            baseline_path,
            "live_shadow.completed_45_minute_target",
            &live,
            &["run", "completed_45_minute_target"],
            self.live_shadow.completed_45_minute_target,
        )?;
        expect_json_u64(
            baseline_path,
            "live_shadow.mismatch_reports",
            &live,
            &["mismatch_summary", "total_reports"],
            self.live_shadow.mismatch_reports,
        )?;
        expect_json_u64(
            baseline_path,
            "live_shadow.p0_mismatches",
            &live,
            &["mismatch_summary", "p0"],
            self.live_shadow.p0_mismatches,
        )?;

        let label = paneflow_worktree
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or_default();
        let merge_base = git_value(
            paneflow_worktree,
            "base commit",
            &["merge-base", "HEAD", self.paneflow.base_commit.as_str()],
        )
        .map_err(|message| invalid_schema(baseline_path, "paneflow.base_commit", message))?;
        let merge_base_short = merge_base.chars().take(8).collect::<String>();
        validate_paneflow_facts(
            baseline_path,
            &self.paneflow,
            label,
            &git_value(paneflow_worktree, "branch", &["branch", "--show-current"])
                .map_err(|message| invalid_schema(baseline_path, "paneflow.branch", message))?,
            &merge_base_short,
        )
    }

    pub fn outcomes(&self) -> &[M6OutcomePolicy] {
        &self.outcomes
    }

    fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        if self.schema != M6_BASELINE_SCHEMA {
            return Err(invalid_schema(
                path,
                "schema",
                format!("expected {M6_BASELINE_SCHEMA}"),
            ));
        }
        if self.version != M6_BASELINE_VERSION {
            return Err(invalid_schema(
                path,
                "version",
                format!("expected {M6_BASELINE_VERSION}"),
            ));
        }
        if !self.generated_at.contains('T') || !self.generated_at.ends_with('Z') {
            return Err(invalid_schema(
                path,
                "generated_at",
                "timestamp must use UTC ISO-8601 form ending in Z",
            ));
        }
        self.source_milestone.validate(path)?;
        self.live_shadow.validate(path)?;
        self.paneflow.validate(path)?;
        self.privacy.validate(path)?;

        if self.outcomes.len() != REQUIRED_OUTCOMES.len() {
            return Err(invalid_schema(
                path,
                "outcomes",
                "exactly the three M6 decision outcomes are required",
            ));
        }

        let mut seen = BTreeSet::new();
        for (index, outcome) in self.outcomes.iter().enumerate() {
            outcome.validate(path, index)?;
            if !seen.insert(outcome.outcome.as_str()) {
                return Err(invalid_schema(
                    path,
                    format!("outcomes[{index}].outcome"),
                    "outcome must be unique",
                ));
            }
        }
        for (required, status) in REQUIRED_OUTCOMES {
            let Some(outcome) = self.outcomes.iter().find(|item| item.outcome == required) else {
                return Err(invalid_schema(
                    path,
                    "outcomes",
                    format!("missing required M6 outcome {required}"),
                ));
            };
            if outcome.status != status {
                return Err(invalid_schema(
                    path,
                    format!("outcomes.{required}.status"),
                    format!("expected {status:?}"),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M6SourceMilestone {
    pub id: String,
    pub status_path: String,
    pub status: String,
    pub stories_total: u64,
    pub stories_done: u64,
}

impl M6SourceMilestone {
    fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        validate_repo_path(path, "source_milestone.status_path", &self.status_path)?;
        if self.status_path != EXPECTED_M5_STATUS_PATH {
            return Err(invalid_schema(
                path,
                "source_milestone.status_path",
                format!("expected {EXPECTED_M5_STATUS_PATH}"),
            ));
        }
        if self.id != "M5" {
            return Err(invalid_schema(path, "source_milestone.id", "expected M5"));
        }
        if self.status != EXPECTED_M5_STATUS {
            return Err(invalid_schema(
                path,
                "source_milestone.status",
                format!("expected {EXPECTED_M5_STATUS}"),
            ));
        }
        expect_u64(
            path,
            "source_milestone.stories_total",
            self.stories_total,
            EXPECTED_M5_STORIES,
        )?;
        expect_u64(
            path,
            "source_milestone.stories_done",
            self.stories_done,
            EXPECTED_M5_STORIES,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M6LiveShadowBaseline {
    pub summary_path: String,
    pub status: String,
    pub completed_45_minute_target: bool,
    pub mismatch_reports: u64,
    pub p0_mismatches: u64,
}

impl M6LiveShadowBaseline {
    fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        validate_repo_path(path, "live_shadow.summary_path", &self.summary_path)?;
        if self.summary_path != EXPECTED_M5_LIVE_SUMMARY_PATH {
            return Err(invalid_schema(
                path,
                "live_shadow.summary_path",
                format!("expected {EXPECTED_M5_LIVE_SUMMARY_PATH}"),
            ));
        }
        if self.status != "targeted_pass" {
            return Err(invalid_schema(
                path,
                "live_shadow.status",
                "expected targeted_pass",
            ));
        }
        if !self.completed_45_minute_target {
            return Err(invalid_schema(
                path,
                "live_shadow.completed_45_minute_target",
                "M5 live shadow must complete the 45-minute target",
            ));
        }
        expect_u64(
            path,
            "live_shadow.mismatch_reports",
            self.mismatch_reports,
            0,
        )?;
        expect_u64(path, "live_shadow.p0_mismatches", self.p0_mismatches, 0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M6PaneflowBaseline {
    pub base_commit: String,
    pub worktree_label: String,
    pub branch: String,
}

impl M6PaneflowBaseline {
    fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        validate_paneflow_facts(
            path,
            self,
            EXPECTED_PANEFLOW_WORKTREE,
            EXPECTED_PANEFLOW_BRANCH,
            EXPECTED_PANEFLOW_COMMIT,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M6OutcomePolicy {
    pub outcome: String,
    pub status: M6OutcomeStatus,
    pub criteria: Vec<String>,
}

impl M6OutcomePolicy {
    fn validate(&self, path: &Path, index: usize) -> Result<(), FixtureLoadError> {
        if self.outcome.trim().is_empty() {
            return Err(invalid_schema(
                path,
                format!("outcomes[{index}].outcome"),
                "outcome is required",
            ));
        }
        if self.criteria.is_empty() || self.criteria.iter().any(|item| item.trim().is_empty()) {
            return Err(invalid_schema(
                path,
                format!("outcomes[{index}].criteria"),
                "at least one non-empty criterion is required",
            ));
        }
        let expected = match self.outcome.as_str() {
            "windows_visible_canary" => WINDOWS_VISIBLE_CANARY_CRITERIA.as_slice(),
            "cross_platform_canary" => CROSS_PLATFORM_CANARY_CRITERIA.as_slice(),
            "default_replacement" => DEFAULT_REPLACEMENT_CRITERIA.as_slice(),
            _ => {
                return Err(invalid_schema(
                    path,
                    format!("outcomes[{index}].outcome"),
                    "unknown M6 decision outcome",
                ));
            }
        };
        if self
            .criteria
            .iter()
            .map(String::as_str)
            .ne(expected.iter().copied())
        {
            return Err(invalid_schema(
                path,
                format!("outcomes[{index}].criteria"),
                "criteria must match the fixed public M6 decision policy",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M6OutcomeStatus {
    Candidate,
    Blocked,
    Prohibited,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M6BaselinePrivacy {
    pub public_summary: bool,
    pub contains_absolute_host_paths: bool,
    pub contains_terminal_transcript: bool,
}

impl M6BaselinePrivacy {
    fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        if !self.public_summary {
            return Err(invalid_schema(
                path,
                "privacy.public_summary",
                "M6 baseline must be a public summary",
            ));
        }
        if self.contains_absolute_host_paths {
            return Err(invalid_schema(
                path,
                "privacy.contains_absolute_host_paths",
                "absolute host paths are forbidden",
            ));
        }
        if self.contains_terminal_transcript {
            return Err(invalid_schema(
                path,
                "privacy.contains_terminal_transcript",
                "terminal transcript content is forbidden",
            ));
        }
        Ok(())
    }
}

fn validate_paneflow_facts(
    path: &Path,
    expected: &M6PaneflowBaseline,
    label: &str,
    branch: &str,
    commit: &str,
) -> Result<(), FixtureLoadError> {
    for (field, actual, expected_value) in [
        (
            "paneflow.worktree_label",
            label,
            expected.worktree_label.as_str(),
        ),
        ("paneflow.branch", branch, expected.branch.as_str()),
        (
            "paneflow.base_commit",
            commit,
            expected.base_commit.as_str(),
        ),
    ] {
        if actual != expected_value {
            return Err(invalid_schema(
                path,
                field,
                format!("expected {expected_value}, observed {actual}"),
            ));
        }
    }
    Ok(())
}

fn read_required_json(
    baseline_path: &Path,
    root: &Path,
    field: &'static str,
    relative: &str,
) -> Result<Value, FixtureLoadError> {
    let source = root.join(relative);
    if !source.is_file() {
        return Err(invalid_schema(
            baseline_path,
            field,
            format!("required source artifact is missing: {relative}"),
        ));
    }
    let canonical_root = fs::canonicalize(root).map_err(|error| {
        invalid_schema(
            baseline_path,
            field,
            format!("failed to resolve Hera repository root: {error}"),
        )
    })?;
    let canonical_source = fs::canonicalize(&source).map_err(|error| {
        invalid_schema(
            baseline_path,
            field,
            format!("failed to resolve source artifact: {error}"),
        )
    })?;
    if !canonical_source.starts_with(&canonical_root) {
        return Err(invalid_schema(
            baseline_path,
            field,
            "source artifact resolves outside the Hera repository root",
        ));
    }
    let raw = read_text_file_capped(
        &canonical_source,
        M6_MAX_BASELINE_BYTES,
        "M6 source artifact",
    )?;
    serde_json::from_str(&raw).map_err(|error| {
        invalid_schema(
            baseline_path,
            field,
            format!("source artifact {relative} is invalid JSON: {error}"),
        )
    })
}

fn value_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    path.iter()
        .try_fold(value, |current, field| current.get(field))
}

fn expect_json_string(
    baseline_path: &Path,
    field: &'static str,
    value: &Value,
    source_path: &[&str],
    expected: &str,
) -> Result<(), FixtureLoadError> {
    let actual = value_at(value, source_path)
        .and_then(Value::as_str)
        .unwrap_or("<missing>");
    if actual != expected {
        return Err(invalid_schema(
            baseline_path,
            field,
            format!("expected {expected}, observed {actual}"),
        ));
    }
    Ok(())
}

fn expect_json_bool(
    baseline_path: &Path,
    field: &'static str,
    value: &Value,
    source_path: &[&str],
    expected: bool,
) -> Result<(), FixtureLoadError> {
    let actual = value_at(value, source_path).and_then(Value::as_bool);
    if actual != Some(expected) {
        return Err(invalid_schema(
            baseline_path,
            field,
            format!("expected {expected}, observed {actual:?}"),
        ));
    }
    Ok(())
}

fn expect_json_u64(
    baseline_path: &Path,
    field: &'static str,
    value: &Value,
    source_path: &[&str],
    expected: u64,
) -> Result<(), FixtureLoadError> {
    let actual = value_at(value, source_path).and_then(Value::as_u64);
    if actual != Some(expected) {
        return Err(invalid_schema(
            baseline_path,
            field,
            format!("expected {expected}, observed {actual:?}"),
        ));
    }
    Ok(())
}

fn expect_u64(
    path: &Path,
    field: &'static str,
    actual: u64,
    expected: u64,
) -> Result<(), FixtureLoadError> {
    if actual != expected {
        return Err(invalid_schema(
            path,
            field,
            format!("expected {expected}, observed {actual}"),
        ));
    }
    Ok(())
}

fn git_value(worktree: &Path, label: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(worktree)
        .args(args)
        .output()
        .map_err(|error| format!("failed to inspect Paneflow {label}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "failed to inspect Paneflow {label}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn validate_public_payload(
    path: &Path,
    field: &str,
    value: &Value,
) -> Result<(), FixtureLoadError> {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                let child_field = if field == "$" {
                    key.clone()
                } else {
                    format!("{field}.{key}")
                };
                if FORBIDDEN_PUBLIC_KEYS.contains(&key.to_ascii_lowercase().as_str()) {
                    return Err(invalid_schema(
                        path,
                        child_field,
                        "terminal transcript fields are forbidden in the public M6 baseline",
                    ));
                }
                validate_public_payload(path, &child_field, child)?;
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                validate_public_payload(path, &format!("{field}[{index}]"), child)?;
            }
        }
        Value::String(text) if looks_like_absolute_host_path(text) => {
            return Err(invalid_schema(
                path,
                field,
                "absolute host paths are forbidden in the public M6 baseline",
            ));
        }
        _ => {}
    }
    Ok(())
}

fn looks_like_absolute_host_path(value: &str) -> bool {
    let bytes = value.as_bytes();
    value.starts_with('/')
        || value.starts_with("\\\\")
        || (bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && matches!(bytes[2], b'/' | b'\\'))
}

fn validate_repo_path(
    manifest_path: &Path,
    field: &'static str,
    value: &str,
) -> Result<(), FixtureLoadError> {
    if value.trim().is_empty()
        || value.contains('\\')
        || value.contains(':')
        || Path::new(value).is_absolute()
        || value
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(invalid_schema(
            manifest_path,
            field,
            "path must be a relative slash-separated repo path",
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
    if !metadata.is_file() || metadata.len() > max_bytes {
        return Err(invalid_schema(
            path,
            "$",
            format!("{label} must be a regular file no larger than {max_bytes} bytes"),
        ));
    }
    let file = fs::File::open(path).map_err(|error| FixtureLoadError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    let mut raw = String::new();
    file.take(max_bytes + 1)
        .read_to_string(&mut raw)
        .map_err(|error| FixtureLoadError::Io {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    if raw.len() as u64 > max_bytes {
        return Err(invalid_schema(
            path,
            "$",
            format!("{label} exceeded maximum of {max_bytes} bytes while reading"),
        ));
    }
    Ok(raw)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{M6Baseline, M6PaneflowBaseline, validate_paneflow_facts};

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn checked_in_m6_baseline_is_public_and_structurally_valid() {
        let baseline = M6Baseline::from_path(workspace_root().join("evidence/m6/m6-baseline.json"))
            .expect("checked-in M6 baseline should validate");
        assert_eq!(baseline.outcomes().len(), 3);
    }

    #[test]
    fn m6_workspace_validation_names_a_missing_m5_artifact() {
        let root = workspace_root();
        let path = root.join("evidence/m6/m6-baseline.json");
        let baseline = M6Baseline::from_path(&path).expect("checked-in baseline");
        let missing_root = std::env::temp_dir().join("hera-m6-missing-source-root");
        let error = baseline
            .validate_workspace(&path, &missing_root, root)
            .expect_err("missing M5 status must fail");
        assert!(error.to_string().contains("source_milestone.status_path"));
        assert!(
            error
                .to_string()
                .contains("required source artifact is missing")
        );
    }

    #[test]
    fn m6_paneflow_validation_names_branch_and_commit_mismatches() {
        let expected = M6PaneflowBaseline {
            base_commit: "4129f8ac".to_owned(),
            worktree_label: "paneflow-hera-m6".to_owned(),
            branch: "feat/hera-m6-host-replacement".to_owned(),
        };
        let path = PathBuf::from("baseline.json");
        let branch =
            validate_paneflow_facts(&path, &expected, "paneflow-hera-m6", "main", "4129f8ac")
                .expect_err("wrong branch must fail");
        assert!(branch.to_string().contains("paneflow.branch"));
        let commit = validate_paneflow_facts(
            &path,
            &expected,
            "paneflow-hera-m6",
            "feat/hera-m6-host-replacement",
            "deadbeef",
        )
        .expect_err("wrong commit must fail");
        assert!(commit.to_string().contains("paneflow.base_commit"));
    }

    #[test]
    fn m6_baseline_rejects_absolute_paths_and_transcript_fields() {
        let raw = std::fs::read_to_string(workspace_root().join("evidence/m6/m6-baseline.json"))
            .expect("checked-in baseline");
        let absolute = raw.replace("paneflow-hera-m6", "C:\\\\dev\\\\paneflow-hera-m6");
        let error = M6Baseline::from_json_str("baseline.json", &absolute)
            .expect_err("absolute path must fail");
        assert!(error.to_string().contains("absolute host paths"));

        let unc = raw.replace("paneflow-hera-m6", r"\\\\server\\private-share");
        let error =
            M6Baseline::from_json_str("baseline.json", &unc).expect_err("UNC path must fail");
        assert!(error.to_string().contains("absolute host paths"));

        let transcript = raw.replace(
            "\"privacy\": {",
            "\"transcript\": \"private terminal text\", \"privacy\": {",
        );
        let error = M6Baseline::from_json_str("baseline.json", &transcript)
            .expect_err("transcript field must fail");
        assert!(error.to_string().contains("terminal transcript fields"));

        let free_form = raw.replace(
            "The Windows visible canary passes",
            "private terminal transcript content",
        );
        let error = M6Baseline::from_json_str("baseline.json", &free_form)
            .expect_err("free-form policy content must fail");
        assert!(
            error
                .to_string()
                .contains("fixed public M6 decision policy")
        );
    }
}
