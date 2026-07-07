use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{FixtureLoadError, invalid_schema, recording_schema_error};

pub const M5_GO_NO_GO_SCHEMA: &str = "hera.m5_go_no_go_policy";
pub const M5_GO_NO_GO_VERSION: u32 = 1;
pub const M5_MAX_GO_NO_GO_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5GoNoGoPolicy {
    pub schema: String,
    pub version: u32,
    pub generated_at: String,
    pub outcomes: Vec<M5OutcomePolicy>,
}

impl M5GoNoGoPolicy {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref();
        let raw = read_text_file_capped(path, M5_MAX_GO_NO_GO_BYTES, "M5 go/no-go policy")?;
        Self::from_json_str(path, &raw)
    }

    pub fn from_json_str(path: impl AsRef<Path>, json: &str) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref().to_path_buf();
        if json.len() as u64 > M5_MAX_GO_NO_GO_BYTES {
            return Err(invalid_schema(
                &path,
                "$",
                format!(
                    "M5 go/no-go JSON is {} bytes, maximum is {M5_MAX_GO_NO_GO_BYTES}",
                    json.len()
                ),
            ));
        }

        let mut deserializer = serde_json::Deserializer::from_str(json);
        let policy: Self =
            serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
                recording_schema_error(&path, &error.path().to_string(), &error.inner().to_string())
            })?;
        policy.validate(&path)?;
        Ok(policy)
    }

    pub fn outcomes(&self) -> &[M5OutcomePolicy] {
        &self.outcomes
    }

    fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        if self.schema != M5_GO_NO_GO_SCHEMA {
            return Err(invalid_schema(
                path,
                "schema",
                format!("expected {M5_GO_NO_GO_SCHEMA}"),
            ));
        }
        if self.version != M5_GO_NO_GO_VERSION {
            return Err(invalid_schema(
                path,
                "version",
                format!("expected {M5_GO_NO_GO_VERSION}"),
            ));
        }
        validate_timestamp(path, "generated_at", &self.generated_at)?;

        let mut outcomes = BTreeSet::new();
        for (index, outcome) in self.outcomes.iter().enumerate() {
            outcome.validate(path, index)?;
            if !outcomes.insert(outcome.outcome) {
                return Err(invalid_schema(
                    path,
                    format!("outcomes[{index}].outcome"),
                    "outcome must be unique",
                ));
            }
        }
        for required in M5Outcome::all() {
            if !outcomes.contains(&required) {
                return Err(invalid_schema(
                    path,
                    "outcomes",
                    format!("missing required M6 outcome {}", required.as_str()),
                ));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5OutcomePolicy {
    pub outcome: M5Outcome,
    pub recommendation_label: String,
    pub status: M5OutcomeStatus,
    pub criteria: Vec<M5GoNoGoCriterion>,
    #[serde(default)]
    pub blockers: Vec<String>,
    pub evidence_sources: Vec<String>,
}

impl M5OutcomePolicy {
    fn validate(&self, path: &Path, index: usize) -> Result<(), FixtureLoadError> {
        validate_required_string(
            path,
            format!("outcomes[{index}].recommendation_label"),
            &self.recommendation_label,
        )?;
        if self.criteria.is_empty() {
            return Err(invalid_schema(
                path,
                format!("outcomes[{index}].criteria"),
                "each outcome needs measurable criteria",
            ));
        }
        if self.evidence_sources.is_empty() {
            return Err(invalid_schema(
                path,
                format!("outcomes[{index}].evidence_sources"),
                "each outcome needs evidence sources",
            ));
        }

        let mut criteria = BTreeSet::new();
        for (criterion_index, criterion) in self.criteria.iter().enumerate() {
            criterion.validate(
                path,
                &format!("outcomes[{index}].criteria[{criterion_index}]"),
            )?;
            criteria.insert(criterion.id.as_str());
        }
        self.outcome.validate_required_criteria(
            path,
            &format!("outcomes[{index}].criteria"),
            &criteria,
        )?;
        validate_string_list(path, format!("outcomes[{index}].blockers"), &self.blockers)?;
        for (source_index, source) in self.evidence_sources.iter().enumerate() {
            validate_repo_path(
                path,
                format!("outcomes[{index}].evidence_sources[{source_index}]"),
                source,
            )?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5Outcome {
    HostReplacementExperiment,
    PublicPreReleasePackaging,
    CompatibilityHardeningMilestone,
}

impl M5Outcome {
    const fn all() -> [Self; 3] {
        [
            Self::HostReplacementExperiment,
            Self::PublicPreReleasePackaging,
            Self::CompatibilityHardeningMilestone,
        ]
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::HostReplacementExperiment => "host_replacement_experiment",
            Self::PublicPreReleasePackaging => "public_pre_release_packaging",
            Self::CompatibilityHardeningMilestone => "compatibility_hardening_milestone",
        }
    }

    fn validate_required_criteria(
        self,
        path: &Path,
        field: &str,
        criteria: &BTreeSet<&str>,
    ) -> Result<(), FixtureLoadError> {
        let required = match self {
            Self::HostReplacementExperiment => [
                "host.compatibility_p0_clear",
                "host.paneflow_shadow_p0_clear",
                "host.platform_rows_measured_or_blocked",
            ],
            Self::PublicPreReleasePackaging => [
                "package.dry_runs_clear",
                "package.publish_order_clear",
                "package.security_blockers_clear",
            ],
            Self::CompatibilityHardeningMilestone => [
                "compatibility.unresolved_p0_present",
                "compatibility.release_or_platform_blocked",
                "compatibility.followup_owner_present",
            ],
        };

        for criterion in required {
            if !criteria.contains(criterion) {
                return Err(invalid_schema(
                    path,
                    field,
                    format!("{} requires criterion {criterion}", self.as_str()),
                ));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5OutcomeStatus {
    Candidate,
    BlockedUntilVerified,
    Fallback,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5GoNoGoCriterion {
    pub id: String,
    pub measurement: String,
    pub pass_condition: String,
    pub blocks_when: String,
}

impl M5GoNoGoCriterion {
    fn validate(&self, path: &Path, field: &str) -> Result<(), FixtureLoadError> {
        validate_non_ambiguous_string(path, format!("{field}.id"), &self.id)?;
        validate_non_ambiguous_string(path, format!("{field}.measurement"), &self.measurement)?;
        validate_non_ambiguous_string(
            path,
            format!("{field}.pass_condition"),
            &self.pass_condition,
        )?;
        validate_non_ambiguous_string(path, format!("{field}.blocks_when"), &self.blocks_when)
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

fn validate_non_ambiguous_string(
    path: &Path,
    field: impl Into<String>,
    value: &str,
) -> Result<(), FixtureLoadError> {
    let field = field.into();
    validate_required_string(path, field.clone(), value)?;
    let lower = value.to_ascii_lowercase();
    for marker in ["tbd", "todo", "ambiguous", "maybe", "unclear"] {
        if lower.contains(marker) {
            return Err(invalid_schema(
                path,
                field,
                format!("go/no-go criteria must be measurable, found {marker:?}"),
            ));
        }
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

    use super::{M5GoNoGoPolicy, M5Outcome, M5OutcomeStatus};
    use crate::FixtureLoadError;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn valid_policy_json() -> &'static str {
        r#"{
          "schema": "hera.m5_go_no_go_policy",
          "version": 1,
          "generated_at": "2026-07-04T00:00:00Z",
          "outcomes": [
            {
              "outcome": "host_replacement_experiment",
              "recommendation_label": "Host replacement experiment",
              "status": "blocked_until_verified",
              "criteria": [
                {
                  "id": "host.compatibility_p0_clear",
                  "measurement": "evidence/m5/compatibility-matrix.json P0 rows",
                  "pass_condition": "No P0 row has failed or not_implemented disposition.",
                  "blocks_when": "Any P0 row has failed or not_implemented disposition."
                },
                {
                  "id": "host.paneflow_shadow_p0_clear",
                  "measurement": "M5 Paneflow shadow dogfood mismatch summary",
                  "pass_condition": "P0 mismatch count is 0.",
                  "blocks_when": "P0 mismatch count is greater than 0."
                },
                {
                  "id": "host.platform_rows_measured_or_blocked",
                  "measurement": "M5 platform rows for Windows, Linux and macOS",
                  "pass_condition": "Every required platform row is pass, failed or blocked with command evidence.",
                  "blocks_when": "Any platform is inferred or missing."
                }
              ],
              "blockers": [],
              "evidence_sources": ["evidence/m5/m5-baseline.json"]
            },
            {
              "outcome": "public_pre_release_packaging",
              "recommendation_label": "Public pre-release packaging",
              "status": "blocked_until_verified",
              "criteria": [
                {
                  "id": "package.dry_runs_clear",
                  "measurement": "M5 package readiness dry-run rows",
                  "pass_condition": "Every intended public crate packages or has an accepted blocker.",
                  "blocks_when": "Any intended public crate dry-run fails without an accepted blocker."
                },
                {
                  "id": "package.publish_order_clear",
                  "measurement": "M5 publish order report",
                  "pass_condition": "Every public crate appears after its Hera dependencies.",
                  "blocks_when": "Publish order is missing or cyclic."
                },
                {
                  "id": "package.security_blockers_clear",
                  "measurement": "M5 security posture report",
                  "pass_condition": "No high-confidence advisory, secret or raw private artifact blocks release readiness.",
                  "blocks_when": "Security report records a release-blocking finding."
                }
              ],
              "blockers": [],
              "evidence_sources": ["evidence/m5/m5-baseline.json"]
            },
            {
              "outcome": "compatibility_hardening_milestone",
              "recommendation_label": "Another compatibility hardening milestone",
              "status": "fallback",
              "criteria": [
                {
                  "id": "compatibility.unresolved_p0_present",
                  "measurement": "M5 compatibility and dogfood rows",
                  "pass_condition": "At least one P0 compatibility, replay or dogfood blocker remains.",
                  "blocks_when": "No unresolved P0 blocker remains."
                },
                {
                  "id": "compatibility.release_or_platform_blocked",
                  "measurement": "M5 package, platform or security rows",
                  "pass_condition": "A release or platform blocker explains why host or packaging outcomes are not eligible.",
                  "blocks_when": "No blocker is recorded for the fallback decision."
                },
                {
                  "id": "compatibility.followup_owner_present",
                  "measurement": "M5 baseline or final report owner fields",
                  "pass_condition": "Every blocker has owner and follow-up story or milestone.",
                  "blocks_when": "A blocker lacks owner or follow-up."
                }
              ],
              "blockers": [],
              "evidence_sources": ["evidence/m5/m5-baseline.json"]
            }
          ]
        }"#
    }

    #[test]
    fn m5_go_no_go_policy_validates_checked_in_policy() {
        let root = workspace_root();
        let path = root.join("evidence/m5/m5-go-no-go-thresholds.json");
        let policy = M5GoNoGoPolicy::from_path(&path).expect("M5 policy should load");

        assert_eq!(policy.outcomes().len(), 3);
        assert!(
            policy
                .outcomes()
                .iter()
                .any(|outcome| outcome.outcome == M5Outcome::HostReplacementExperiment)
        );
    }

    #[test]
    fn m5_go_no_go_policy_rejects_missing_outcome() {
        let json = valid_policy_json().replace(
            "\"outcome\": \"compatibility_hardening_milestone\"",
            "\"outcome\": \"host_replacement_experiment\"",
        );

        let error = M5GoNoGoPolicy::from_json_str("policy.json", &json)
            .expect_err("missing outcome must fail");

        assert!(matches!(error, FixtureLoadError::InvalidSchema { .. }));
    }

    #[test]
    fn m5_go_no_go_policy_rejects_ambiguous_thresholds() {
        let json =
            valid_policy_json().replace("P0 mismatch count is 0.", "Maybe no P0 mismatch appears.");

        let error = M5GoNoGoPolicy::from_json_str("policy.json", &json)
            .expect_err("ambiguous threshold must fail");

        assert!(matches!(
            error,
            FixtureLoadError::InvalidSchema { field, .. }
                if field == "outcomes[0].criteria[1].pass_condition"
        ));
    }

    #[test]
    fn m5_go_no_go_policy_status_roundtrips() {
        let policy = M5GoNoGoPolicy::from_json_str("policy.json", valid_policy_json())
            .expect("policy should load");

        assert_eq!(
            policy.outcomes()[0].status,
            M5OutcomeStatus::BlockedUntilVerified
        );
    }
}
