use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{FixtureLoadError, invalid_schema, recording_schema_error};

pub const M5_PACKAGE_READINESS_SCHEMA: &str = "hera.m5_package_readiness";
pub const M5_RELEASE_PLAN_SCHEMA: &str = "hera.m5_release_plan";
pub const M5_API_AUDIT_SCHEMA: &str = "hera.m5_api_audit";
pub const M5_RELEASE_EVIDENCE_VERSION: u32 = 1;
pub const M5_MAX_RELEASE_EVIDENCE_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5PackageReadiness {
    pub schema: String,
    pub version: u32,
    pub generated_at: String,
    pub source_command: String,
    pub status: M5ReadinessStatus,
    pub release_plan_path: String,
    pub crates: Vec<M5PackageCrate>,
    pub publish_actions: Vec<M5PublishAction>,
    #[serde(default)]
    pub notes: Vec<String>,
}

impl M5PackageReadiness {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref();
        let raw = read_text_file_capped(
            path,
            M5_MAX_RELEASE_EVIDENCE_BYTES,
            "M5 package readiness evidence",
        )?;
        Self::from_json_str(path, &raw)
    }

    pub fn from_json_str(path: impl AsRef<Path>, json: &str) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref().to_path_buf();
        let mut deserializer = serde_json::Deserializer::from_str(json);
        let readiness: Self =
            serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
                recording_schema_error(&path, &error.path().to_string(), &error.inner().to_string())
            })?;
        readiness.validate(&path)?;
        Ok(readiness)
    }

    pub fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        if self.schema != M5_PACKAGE_READINESS_SCHEMA {
            return Err(invalid_schema(
                path,
                "schema",
                format!("expected {M5_PACKAGE_READINESS_SCHEMA}"),
            ));
        }
        validate_release_header(path, self.version, &self.generated_at, &self.source_command)?;
        validate_repo_path(path, "release_plan_path", &self.release_plan_path)?;
        validate_string_list(path, "notes", &self.notes)?;
        if self.crates.is_empty() {
            return Err(invalid_schema(
                path,
                "crates",
                "at least one crate is required",
            ));
        }
        if self.publish_actions.is_empty() {
            return Err(invalid_schema(
                path,
                "publish_actions",
                "publish action policy is required",
            ));
        }

        let mut names = BTreeSet::new();
        let mut has_blocked_crate = false;
        for (index, krate) in self.crates.iter().enumerate() {
            krate.validate(path, &format!("crates[{index}]"))?;
            if !names.insert(krate.name.as_str()) {
                return Err(invalid_schema(
                    path,
                    format!("crates[{index}].name"),
                    "crate names must be unique",
                ));
            }
            if krate.package_status != M5ReadinessStatus::Pass {
                has_blocked_crate = true;
            }
        }

        for (index, action) in self.publish_actions.iter().enumerate() {
            action.validate(path, &format!("publish_actions[{index}]"))?;
        }
        let records_publish_guard = self.publish_actions.iter().any(|action| {
            action.command == "cargo publish" && action.status == M5ActionStatus::OutOfScope
        });
        if !records_publish_guard {
            return Err(invalid_schema(
                path,
                "publish_actions",
                "cargo publish must be recorded as out_of_scope",
            ));
        }
        if self.status == M5ReadinessStatus::Pass && has_blocked_crate {
            return Err(invalid_schema(
                path,
                "status",
                "pass readiness cannot contain blocked crate rows",
            ));
        }
        Ok(())
    }

    pub fn public_crates(&self) -> impl Iterator<Item = &M5PackageCrate> {
        self.crates
            .iter()
            .filter(|krate| krate.publish_intent == M5PublishIntent::IntendedPublic)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5ReleasePlan {
    pub schema: String,
    pub version: u32,
    pub generated_at: String,
    pub source_command: String,
    pub publish_out_of_scope: bool,
    pub publish_order: Vec<String>,
    pub crates: Vec<M5ReleasePlanCrate>,
    #[serde(default)]
    pub notes: Vec<String>,
}

impl M5ReleasePlan {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref();
        let raw = read_text_file_capped(
            path,
            M5_MAX_RELEASE_EVIDENCE_BYTES,
            "M5 release plan evidence",
        )?;
        Self::from_json_str(path, &raw)
    }

    pub fn from_json_str(path: impl AsRef<Path>, json: &str) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref().to_path_buf();
        let mut deserializer = serde_json::Deserializer::from_str(json);
        let plan: Self = serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
            recording_schema_error(&path, &error.path().to_string(), &error.inner().to_string())
        })?;
        plan.validate(&path)?;
        Ok(plan)
    }

    pub fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        if self.schema != M5_RELEASE_PLAN_SCHEMA {
            return Err(invalid_schema(
                path,
                "schema",
                format!("expected {M5_RELEASE_PLAN_SCHEMA}"),
            ));
        }
        validate_release_header(path, self.version, &self.generated_at, &self.source_command)?;
        if !self.publish_out_of_scope {
            return Err(invalid_schema(
                path,
                "publish_out_of_scope",
                "M5 must not authorize cargo publish",
            ));
        }
        if self.publish_order.is_empty() {
            return Err(invalid_schema(
                path,
                "publish_order",
                "at least one public crate is required",
            ));
        }
        validate_string_list(path, "publish_order", &self.publish_order)?;
        validate_string_list(path, "notes", &self.notes)?;

        let mut crates_by_name = BTreeMap::new();
        for (index, krate) in self.crates.iter().enumerate() {
            krate.validate(path, &format!("crates[{index}]"))?;
            if crates_by_name.insert(krate.name.as_str(), krate).is_some() {
                return Err(invalid_schema(
                    path,
                    format!("crates[{index}].name"),
                    "crate names must be unique",
                ));
            }
        }

        let order = self
            .publish_order
            .iter()
            .enumerate()
            .map(|(index, name)| (name.as_str(), index))
            .collect::<BTreeMap<_, _>>();
        for krate in self
            .crates
            .iter()
            .filter(|krate| krate.publish_intent == M5PublishIntent::IntendedPublic)
        {
            if !order.contains_key(krate.name.as_str()) {
                return Err(invalid_schema(
                    path,
                    "publish_order",
                    format!("public crate {} is missing from publish order", krate.name),
                ));
            }
        }
        for name in &self.publish_order {
            let Some(krate) = crates_by_name.get(name.as_str()) else {
                return Err(invalid_schema(
                    path,
                    "publish_order",
                    format!("publish order references unknown crate {name}"),
                ));
            };
            if krate.publish_intent != M5PublishIntent::IntendedPublic {
                return Err(invalid_schema(
                    path,
                    "publish_order",
                    format!("non-public crate {name} must not be in publish order"),
                ));
            }
            let crate_index = order[name.as_str()];
            for dependency in &krate.depends_on {
                let Some(dependency_index) = order.get(dependency.as_str()) else {
                    return Err(invalid_schema(
                        path,
                        format!("crates.{}.depends_on", krate.name),
                        format!("dependency {dependency} is not in publish order"),
                    ));
                };
                if *dependency_index > crate_index {
                    return Err(invalid_schema(
                        path,
                        "publish_order",
                        format!(
                            "dependency {dependency} must be published before {}",
                            krate.name
                        ),
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5ApiAudit {
    pub schema: String,
    pub version: u32,
    pub generated_at: String,
    pub source_command: String,
    pub status: M5ReadinessStatus,
    pub sources: Vec<M5AuditSource>,
    pub public_crates: Vec<M5ApiPublicCrate>,
    pub boundary_checks: Vec<M5ApiBoundaryCheck>,
    pub semver_baseline: M5SemverBaseline,
    pub findings: Vec<M5ApiFinding>,
    #[serde(default)]
    pub notes: Vec<String>,
}

impl M5ApiAudit {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref();
        let raw =
            read_text_file_capped(path, M5_MAX_RELEASE_EVIDENCE_BYTES, "M5 API audit evidence")?;
        Self::from_json_str(path, &raw)
    }

    pub fn from_json_str(path: impl AsRef<Path>, json: &str) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref().to_path_buf();
        let mut deserializer = serde_json::Deserializer::from_str(json);
        let audit: Self = serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
            recording_schema_error(&path, &error.path().to_string(), &error.inner().to_string())
        })?;
        audit.validate(&path)?;
        Ok(audit)
    }

    pub fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        if self.schema != M5_API_AUDIT_SCHEMA {
            return Err(invalid_schema(
                path,
                "schema",
                format!("expected {M5_API_AUDIT_SCHEMA}"),
            ));
        }
        validate_release_header(path, self.version, &self.generated_at, &self.source_command)?;
        if self.sources.is_empty() {
            return Err(invalid_schema(
                path,
                "sources",
                "audit sources are required",
            ));
        }
        if self.public_crates.is_empty() {
            return Err(invalid_schema(
                path,
                "public_crates",
                "at least one public crate must be audited",
            ));
        }
        if self.boundary_checks.is_empty() {
            return Err(invalid_schema(
                path,
                "boundary_checks",
                "boundary checks are required",
            ));
        }
        validate_string_list(path, "notes", &self.notes)?;

        for (index, source) in self.sources.iter().enumerate() {
            source.validate(path, &format!("sources[{index}]"))?;
        }
        for (index, krate) in self.public_crates.iter().enumerate() {
            krate.validate(path, &format!("public_crates[{index}]"))?;
        }
        for (index, check) in self.boundary_checks.iter().enumerate() {
            check.validate(path, &format!("boundary_checks[{index}]"))?;
        }
        self.semver_baseline.validate(path, "semver_baseline")?;
        for (index, finding) in self.findings.iter().enumerate() {
            finding.validate(path, &format!("findings[{index}]"))?;
        }

        let has_p0_open = self.findings.iter().any(|finding| {
            finding.severity == M5FindingSeverity::P0
                && finding.status == M5FindingStatus::Open
                && !finding.m6_blocker
        });
        if has_p0_open {
            return Err(invalid_schema(
                path,
                "findings",
                "P0 findings must be resolved or listed as M6 blockers",
            ));
        }
        let has_failed_boundary = self
            .boundary_checks
            .iter()
            .any(|check| check.status != M5CheckStatus::Pass);
        let has_failed_crate_check = self.public_crates.iter().any(|krate| {
            krate.docs_status != M5CheckStatus::Pass
                || krate.errors_status != M5CheckStatus::Pass
                || krate.common_traits_status != M5CheckStatus::Pass
                || krate.boundary_leak_status != M5CheckStatus::Pass
        });
        let semver_failed = self.semver_baseline.status == M5ToolStatus::Failed;
        if self.status == M5ReadinessStatus::Pass
            && (has_failed_boundary || has_failed_crate_check || semver_failed)
        {
            return Err(invalid_schema(
                path,
                "status",
                "pass API audit cannot contain failed crate, boundary or semver checks",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5PackageCrate {
    pub name: String,
    pub manifest_path: String,
    pub publish_intent: M5PublishIntent,
    pub package_status: M5ReadinessStatus,
    pub description: String,
    pub license: String,
    pub repository: String,
    pub documentation: String,
    pub readme: String,
    pub keywords: Vec<String>,
    pub categories: Vec<String>,
    pub docs_rs: M5DocsRsPolicy,
    #[serde(default)]
    pub metadata_omissions: Vec<M5MetadataOmission>,
    pub dry_run: M5PackageDryRun,
    #[serde(default)]
    pub notes: Vec<String>,
}

impl M5PackageCrate {
    fn validate(&self, path: &Path, field: &str) -> Result<(), FixtureLoadError> {
        validate_required_string(path, format!("{field}.name"), &self.name)?;
        validate_repo_path(path, format!("{field}.manifest_path"), &self.manifest_path)?;
        validate_required_string(path, format!("{field}.description"), &self.description)?;
        validate_required_string(path, format!("{field}.repository"), &self.repository)?;
        validate_required_string(path, format!("{field}.documentation"), &self.documentation)?;
        validate_required_string(path, format!("{field}.readme"), &self.readme)?;
        validate_repo_path(path, format!("{field}.readme"), &self.readme)?;
        validate_string_list(path, format!("{field}.keywords"), &self.keywords)?;
        validate_string_list(path, format!("{field}.categories"), &self.categories)?;
        validate_string_list(path, format!("{field}.notes"), &self.notes)?;
        for (index, omission) in self.metadata_omissions.iter().enumerate() {
            omission.validate(path, &format!("{field}.metadata_omissions[{index}]"))?;
        }
        self.docs_rs.validate(path, &format!("{field}.docs_rs"))?;
        self.dry_run.validate(path, &format!("{field}.dry_run"))?;

        match self.publish_intent {
            M5PublishIntent::IntendedPublic => {
                validate_required_string(path, format!("{field}.license"), &self.license)?;
                if self.package_status == M5ReadinessStatus::Pass
                    && self.dry_run.status != M5DryRunStatus::Pass
                {
                    return Err(invalid_schema(
                        path,
                        format!("{field}.package_status"),
                        "passing public crates must have a passing dry-run",
                    ));
                }
            }
            M5PublishIntent::InternalNonPublishable => {
                if self.package_status == M5ReadinessStatus::Pass {
                    return Err(invalid_schema(
                        path,
                        format!("{field}.package_status"),
                        "internal crates must not be counted as public package pass",
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5DocsRsPolicy {
    pub all_features: bool,
    pub default_target: String,
    pub targets: Vec<String>,
    #[serde(default)]
    pub cargo_args: Vec<String>,
    #[serde(default)]
    pub rustdoc_args: Vec<String>,
}

impl M5DocsRsPolicy {
    fn validate(&self, path: &Path, field: &str) -> Result<(), FixtureLoadError> {
        validate_required_string(
            path,
            format!("{field}.default_target"),
            &self.default_target,
        )?;
        validate_string_list(path, format!("{field}.targets"), &self.targets)?;
        validate_string_list(path, format!("{field}.cargo_args"), &self.cargo_args)?;
        validate_string_list(path, format!("{field}.rustdoc_args"), &self.rustdoc_args)?;
        if !self.all_features {
            return Err(invalid_schema(
                path,
                format!("{field}.all_features"),
                "docs.rs policy must document all feature behavior",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5PackageDryRun {
    pub command: String,
    pub status: M5DryRunStatus,
    #[serde(default)]
    pub exit_code: Option<i32>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    pub stdout_summary: String,
    pub stderr_summary: String,
    #[serde(default)]
    pub package_archive: Option<String>,
    #[serde(default)]
    pub blocked_reason: Option<String>,
}

impl M5PackageDryRun {
    fn validate(&self, path: &Path, field: &str) -> Result<(), FixtureLoadError> {
        validate_required_string(path, format!("{field}.command"), &self.command)?;
        if let Some(package_archive) = &self.package_archive {
            validate_repo_path(path, format!("{field}.package_archive"), package_archive)?;
        }
        match self.status {
            M5DryRunStatus::Pass => {
                if self.exit_code != Some(0) {
                    return Err(invalid_schema(
                        path,
                        format!("{field}.exit_code"),
                        "passing dry-runs must record exit_code 0",
                    ));
                }
                if self.package_archive.is_none() {
                    return Err(invalid_schema(
                        path,
                        format!("{field}.package_archive"),
                        "passing dry-runs must record the package archive",
                    ));
                }
            }
            M5DryRunStatus::Failed => {
                if self.exit_code.is_none_or(|code| code == 0) {
                    return Err(invalid_schema(
                        path,
                        format!("{field}.exit_code"),
                        "failed dry-runs must record a non-zero exit code",
                    ));
                }
            }
            M5DryRunStatus::Blocked => {
                if self
                    .blocked_reason
                    .as_deref()
                    .is_none_or(|reason| reason.trim().is_empty())
                {
                    return Err(invalid_schema(
                        path,
                        format!("{field}.blocked_reason"),
                        "blocked dry-runs must include a reason",
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5ReleasePlanCrate {
    pub name: String,
    pub publish_intent: M5PublishIntent,
    pub publish_config: String,
    pub release_step: u32,
    pub depends_on: Vec<String>,
    pub rationale: String,
}

impl M5ReleasePlanCrate {
    fn validate(&self, path: &Path, field: &str) -> Result<(), FixtureLoadError> {
        validate_required_string(path, format!("{field}.name"), &self.name)?;
        validate_required_string(
            path,
            format!("{field}.publish_config"),
            &self.publish_config,
        )?;
        validate_required_string(path, format!("{field}.rationale"), &self.rationale)?;
        validate_string_list(path, format!("{field}.depends_on"), &self.depends_on)?;
        if self.publish_intent == M5PublishIntent::IntendedPublic && self.release_step == 0 {
            return Err(invalid_schema(
                path,
                format!("{field}.release_step"),
                "public crates need a one-based release step",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5PublishAction {
    pub command: String,
    pub status: M5ActionStatus,
    pub summary: String,
}

impl M5PublishAction {
    fn validate(&self, path: &Path, field: &str) -> Result<(), FixtureLoadError> {
        validate_required_string(path, format!("{field}.command"), &self.command)?;
        validate_required_string(path, format!("{field}.summary"), &self.summary)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5MetadataOmission {
    pub field: String,
    pub reason: String,
}

impl M5MetadataOmission {
    fn validate(&self, path: &Path, field: &str) -> Result<(), FixtureLoadError> {
        validate_required_string(path, format!("{field}.field"), &self.field)?;
        validate_required_string(path, format!("{field}.reason"), &self.reason)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5AuditSource {
    pub label: String,
    pub url: String,
}

impl M5AuditSource {
    fn validate(&self, path: &Path, field: &str) -> Result<(), FixtureLoadError> {
        validate_required_string(path, format!("{field}.label"), &self.label)?;
        validate_required_string(path, format!("{field}.url"), &self.url)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5ApiPublicCrate {
    pub name: String,
    pub boundary: String,
    pub exported_surface: Vec<String>,
    pub docs_status: M5CheckStatus,
    pub errors_status: M5CheckStatus,
    pub common_traits_status: M5CheckStatus,
    pub boundary_leak_status: M5CheckStatus,
}

impl M5ApiPublicCrate {
    fn validate(&self, path: &Path, field: &str) -> Result<(), FixtureLoadError> {
        validate_required_string(path, format!("{field}.name"), &self.name)?;
        validate_required_string(path, format!("{field}.boundary"), &self.boundary)?;
        validate_string_list(
            path,
            format!("{field}.exported_surface"),
            &self.exported_surface,
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5ApiBoundaryCheck {
    pub id: String,
    pub status: M5CheckStatus,
    pub evidence: String,
}

impl M5ApiBoundaryCheck {
    fn validate(&self, path: &Path, field: &str) -> Result<(), FixtureLoadError> {
        validate_required_string(path, format!("{field}.id"), &self.id)?;
        validate_required_string(path, format!("{field}.evidence"), &self.evidence)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5SemverBaseline {
    pub command: String,
    pub status: M5ToolStatus,
    #[serde(default)]
    pub exit_code: Option<i32>,
    pub summary: String,
    #[serde(default)]
    pub blocked_reason: Option<String>,
}

impl M5SemverBaseline {
    fn validate(&self, path: &Path, field: &str) -> Result<(), FixtureLoadError> {
        validate_required_string(path, format!("{field}.command"), &self.command)?;
        validate_required_string(path, format!("{field}.summary"), &self.summary)?;
        match self.status {
            M5ToolStatus::Pass => {
                if self.exit_code != Some(0) {
                    return Err(invalid_schema(
                        path,
                        format!("{field}.exit_code"),
                        "passing tools must record exit_code 0",
                    ));
                }
            }
            M5ToolStatus::Failed => {
                if self.exit_code.is_none_or(|code| code == 0) {
                    return Err(invalid_schema(
                        path,
                        format!("{field}.exit_code"),
                        "failed tools must record a non-zero exit code",
                    ));
                }
            }
            M5ToolStatus::Blocked => {
                if self
                    .blocked_reason
                    .as_deref()
                    .is_none_or(|reason| reason.trim().is_empty())
                {
                    return Err(invalid_schema(
                        path,
                        format!("{field}.blocked_reason"),
                        "blocked tools must record why coverage is unavailable",
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5ApiFinding {
    pub id: String,
    pub severity: M5FindingSeverity,
    pub status: M5FindingStatus,
    pub summary: String,
    pub m6_blocker: bool,
}

impl M5ApiFinding {
    fn validate(&self, path: &Path, field: &str) -> Result<(), FixtureLoadError> {
        validate_required_string(path, format!("{field}.id"), &self.id)?;
        validate_required_string(path, format!("{field}.summary"), &self.summary)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5ReadinessStatus {
    Pass,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5PublishIntent {
    IntendedPublic,
    InternalNonPublishable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5DryRunStatus {
    Pass,
    Failed,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5ActionStatus {
    OutOfScope,
    NotRun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5CheckStatus {
    Pass,
    Blocked,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5ToolStatus {
    Pass,
    Failed,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5FindingSeverity {
    P0,
    P1,
    P2,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5FindingStatus {
    Open,
    Resolved,
    AcceptedM6Blocker,
}

fn validate_release_header(
    path: &Path,
    version: u32,
    generated_at: &str,
    source_command: &str,
) -> Result<(), FixtureLoadError> {
    if version != M5_RELEASE_EVIDENCE_VERSION {
        return Err(invalid_schema(
            path,
            "version",
            format!("expected {M5_RELEASE_EVIDENCE_VERSION}"),
        ));
    }
    validate_timestamp(path, "generated_at", generated_at)?;
    validate_required_string(path, "source_command", source_command)
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
    let repo_path = Path::new(value);
    if repo_path.is_absolute() {
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

    use super::{M5ApiAudit, M5PackageReadiness, M5ReadinessStatus, M5ReleasePlan, M5ToolStatus};
    use crate::FixtureLoadError;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn m5_package_readiness_validates_checked_in_artifact() {
        let path = workspace_root().join("evidence/m5/m5-package-readiness.json");
        let readiness =
            M5PackageReadiness::from_path(&path).expect("M5 package readiness should load");

        assert_eq!(readiness.status, M5ReadinessStatus::Blocked);
        assert_eq!(readiness.public_crates().count(), 6);
    }

    #[test]
    fn m5_release_plan_validates_checked_in_artifact() {
        let path = workspace_root().join("evidence/m5/m5-release-plan.json");
        let plan = M5ReleasePlan::from_path(&path).expect("M5 release plan should load");

        assert_eq!(
            plan.publish_order.first().map(String::as_str),
            Some("terminal-protocol")
        );
        assert!(plan.publish_out_of_scope);
    }

    #[test]
    fn m5_api_audit_validates_checked_in_artifact() {
        let path = workspace_root().join("evidence/m5/m5-api-audit.json");
        let audit = M5ApiAudit::from_path(&path).expect("M5 API audit should load");

        assert_eq!(audit.status, M5ReadinessStatus::Pass);
        assert_eq!(audit.semver_baseline.status, M5ToolStatus::Blocked);
    }

    #[test]
    fn release_plan_rejects_wrong_dependency_order() {
        let json =
            std::fs::read_to_string(workspace_root().join("evidence/m5/m5-release-plan.json"))
                .expect("release plan fixture should read")
                .replace(
                    r#""terminal-protocol",
    "terminal-render-model",
    "terminal-core""#,
                    r#""terminal-core",
    "terminal-protocol",
    "terminal-render-model""#,
                );

        let error = M5ReleasePlan::from_json_str("release-plan.json", &json)
            .expect_err("wrong publish order must fail");

        assert!(matches!(
            error,
            FixtureLoadError::InvalidSchema { field, .. } if field == "publish_order"
        ));
    }
}
