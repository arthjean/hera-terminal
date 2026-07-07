use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{FixtureLoadError, invalid_schema, recording_schema_error};

pub const M5_PLATFORM_RUNTIME_EVIDENCE_SCHEMA: &str = "hera.m5_platform_runtime_evidence";
pub const M5_PLATFORM_RUNTIME_EVIDENCE_VERSION: u32 = 1;
pub const M5_MAX_PLATFORM_RUNTIME_EVIDENCE_BYTES: u64 = 1024 * 1024;

const M5_REQUIRED_COMMANDS: [M5PlatformCommandId; 5] = [
    M5PlatformCommandId::CargoCheckWorkspace,
    M5PlatformCommandId::CargoTestWorkspace,
    M5PlatformCommandId::CargoDocWorkspace,
    M5PlatformCommandId::ValidateM5Compatibility,
    M5PlatformCommandId::VerifyM5Replay,
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5PlatformRuntimeEvidence {
    pub schema: String,
    pub version: u32,
    pub generated_at: String,
    pub source_command: String,
    pub freshness_policy: M5PlatformFreshnessPolicy,
    pub required_commands: Vec<M5PlatformCommandId>,
    pub platforms: M5PlatformRows,
    #[serde(default)]
    pub notes: Vec<String>,
}

impl M5PlatformRuntimeEvidence {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref();
        let raw = read_text_file_capped(
            path,
            M5_MAX_PLATFORM_RUNTIME_EVIDENCE_BYTES,
            "M5 platform runtime evidence",
        )?;
        Self::from_json_str(path, &raw)
    }

    pub fn from_json_str(path: impl AsRef<Path>, json: &str) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref().to_path_buf();
        if json.len() as u64 > M5_MAX_PLATFORM_RUNTIME_EVIDENCE_BYTES {
            return Err(invalid_schema(
                &path,
                "$",
                format!(
                    "M5 platform runtime evidence JSON is {} bytes, maximum is {M5_MAX_PLATFORM_RUNTIME_EVIDENCE_BYTES}",
                    json.len()
                ),
            ));
        }

        let mut deserializer = serde_json::Deserializer::from_str(json);
        let evidence: Self =
            serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
                recording_schema_error(&path, &error.path().to_string(), &error.inner().to_string())
            })?;
        evidence.validate(&path)?;
        Ok(evidence)
    }

    pub fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        if self.schema != M5_PLATFORM_RUNTIME_EVIDENCE_SCHEMA {
            return Err(invalid_schema(
                path,
                "schema",
                format!("expected {M5_PLATFORM_RUNTIME_EVIDENCE_SCHEMA}"),
            ));
        }
        if self.version != M5_PLATFORM_RUNTIME_EVIDENCE_VERSION {
            return Err(invalid_schema(
                path,
                "version",
                format!("expected {M5_PLATFORM_RUNTIME_EVIDENCE_VERSION}"),
            ));
        }
        validate_timestamp(path, "generated_at", &self.generated_at)?;
        validate_required_string(path, "source_command", &self.source_command)?;
        self.freshness_policy.validate(path)?;
        validate_string_list(path, "notes", &self.notes)?;
        validate_required_command_set(path, "required_commands", &self.required_commands)?;
        self.platforms.validate(path, &self.required_commands)
    }

    #[must_use]
    pub fn platform_rows(&self) -> [&M5PlatformRow; 3] {
        [
            &self.platforms.windows,
            &self.platforms.linux,
            &self.platforms.macos,
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5PlatformFreshnessPolicy {
    pub generated_after: String,
    pub stale_after_days: u32,
}

impl M5PlatformFreshnessPolicy {
    fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        validate_timestamp(
            path,
            "freshness_policy.generated_after",
            &self.generated_after,
        )?;
        if self.stale_after_days == 0 {
            return Err(invalid_schema(
                path,
                "freshness_policy.stale_after_days",
                "stale-after window must be non-zero",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5PlatformRows {
    pub windows: M5PlatformRow,
    pub linux: M5PlatformRow,
    pub macos: M5PlatformRow,
}

impl M5PlatformRows {
    fn validate(
        &self,
        path: &Path,
        required_commands: &[M5PlatformCommandId],
    ) -> Result<(), FixtureLoadError> {
        self.windows.validate(
            path,
            "platforms.windows",
            M5RuntimePlatform::Windows,
            required_commands,
        )?;
        self.linux.validate(
            path,
            "platforms.linux",
            M5RuntimePlatform::Linux,
            required_commands,
        )?;
        self.macos.validate(
            path,
            "platforms.macos",
            M5RuntimePlatform::Macos,
            required_commands,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5PlatformRow {
    pub platform: M5RuntimePlatform,
    pub status: M5PlatformEvidenceStatus,
    pub os: String,
    pub target_triple: String,
    pub rustc_version: String,
    pub generated_at: String,
    pub commands: Vec<M5PlatformCommandResult>,
    #[serde(default)]
    pub artifact_paths: Vec<String>,
    #[serde(default)]
    pub blocked_reason: Option<String>,
    #[serde(default)]
    pub notes: Vec<String>,
}

impl M5PlatformRow {
    fn validate(
        &self,
        path: &Path,
        field: &str,
        expected_platform: M5RuntimePlatform,
        required_commands: &[M5PlatformCommandId],
    ) -> Result<(), FixtureLoadError> {
        if self.platform != expected_platform {
            return Err(invalid_schema(
                path,
                format!("{field}.platform"),
                format!("expected {expected_platform:?}"),
            ));
        }
        validate_required_string(path, format!("{field}.os"), &self.os)?;
        validate_required_string(path, format!("{field}.target_triple"), &self.target_triple)?;
        validate_required_string(path, format!("{field}.rustc_version"), &self.rustc_version)?;
        validate_timestamp(path, format!("{field}.generated_at"), &self.generated_at)?;
        validate_repo_paths(
            path,
            format!("{field}.artifact_paths"),
            &self.artifact_paths,
        )?;
        validate_string_list(path, format!("{field}.notes"), &self.notes)?;
        validate_command_results(
            path,
            format!("{field}.commands"),
            required_commands,
            &self.commands,
        )?;

        if matches!(
            self.status,
            M5PlatformEvidenceStatus::Blocked | M5PlatformEvidenceStatus::Stale
        ) && self
            .blocked_reason
            .as_deref()
            .is_none_or(|reason| reason.trim().is_empty())
        {
            return Err(invalid_schema(
                path,
                format!("{field}.blocked_reason"),
                "blocked or stale platform rows must explain why",
            ));
        }

        let has_failed = self
            .commands
            .iter()
            .any(|command| command.status == M5PlatformCommandStatus::Failed);
        let has_blocked = self
            .commands
            .iter()
            .any(|command| command.status == M5PlatformCommandStatus::Blocked);
        match self.status {
            M5PlatformEvidenceStatus::Pass if has_failed || has_blocked => Err(invalid_schema(
                path,
                format!("{field}.status"),
                "pass platform rows cannot contain failed or blocked commands",
            )),
            M5PlatformEvidenceStatus::Failed if !has_failed => Err(invalid_schema(
                path,
                format!("{field}.status"),
                "failed platform rows must contain at least one failed command",
            )),
            M5PlatformEvidenceStatus::Blocked if has_failed => Err(invalid_schema(
                path,
                format!("{field}.status"),
                "blocked platform rows must not hide failed commands",
            )),
            M5PlatformEvidenceStatus::Blocked if !has_blocked => Err(invalid_schema(
                path,
                format!("{field}.status"),
                "blocked platform rows must contain at least one blocked command",
            )),
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M5PlatformCommandResult {
    pub id: M5PlatformCommandId,
    pub command: String,
    pub status: M5PlatformCommandStatus,
    #[serde(default)]
    pub exit_code: Option<i32>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub stdout_summary: String,
    #[serde(default)]
    pub stderr_summary: String,
    #[serde(default)]
    pub artifact_paths: Vec<String>,
    #[serde(default)]
    pub blocked_reason: Option<String>,
}

impl M5PlatformCommandResult {
    fn validate(&self, path: &Path, field: &str) -> Result<(), FixtureLoadError> {
        validate_required_string(path, format!("{field}.command"), &self.command)?;
        validate_repo_paths(
            path,
            format!("{field}.artifact_paths"),
            &self.artifact_paths,
        )?;
        match self.status {
            M5PlatformCommandStatus::Pass => {
                if self.exit_code != Some(0) {
                    return Err(invalid_schema(
                        path,
                        format!("{field}.exit_code"),
                        "pass commands must record exit_code 0",
                    ));
                }
            }
            M5PlatformCommandStatus::Failed => {
                if self.exit_code.is_none_or(|code| code == 0) {
                    return Err(invalid_schema(
                        path,
                        format!("{field}.exit_code"),
                        "failed commands must record a non-zero exit code",
                    ));
                }
            }
            M5PlatformCommandStatus::Blocked => {
                if self.exit_code.is_some() {
                    return Err(invalid_schema(
                        path,
                        format!("{field}.exit_code"),
                        "blocked commands must not pretend an executed exit code exists",
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
                        "blocked commands must include blocked_reason or stderr_summary",
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5PlatformCommandId {
    CargoCheckWorkspace,
    CargoTestWorkspace,
    CargoDocWorkspace,
    ValidateM5Compatibility,
    VerifyM5Replay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5PlatformCommandStatus {
    Pass,
    Failed,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5PlatformEvidenceStatus {
    Pass,
    Failed,
    Blocked,
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M5RuntimePlatform {
    Windows,
    Linux,
    Macos,
}

#[must_use]
pub fn m5_required_platform_commands() -> Vec<M5PlatformCommandId> {
    M5_REQUIRED_COMMANDS.to_vec()
}

fn validate_required_command_set(
    path: &Path,
    field: &str,
    commands: &[M5PlatformCommandId],
) -> Result<(), FixtureLoadError> {
    if commands.is_empty() {
        return Err(invalid_schema(
            path,
            field,
            "required command list is empty",
        ));
    }
    let expected = M5_REQUIRED_COMMANDS.into_iter().collect::<BTreeSet<_>>();
    let actual = commands.iter().copied().collect::<BTreeSet<_>>();
    if actual != expected || actual.len() != commands.len() {
        return Err(invalid_schema(
            path,
            field,
            "required commands must be the exact M5 platform command set",
        ));
    }
    Ok(())
}

fn validate_command_results(
    path: &Path,
    field: String,
    required_commands: &[M5PlatformCommandId],
    commands: &[M5PlatformCommandResult],
) -> Result<(), FixtureLoadError> {
    let required = required_commands.iter().copied().collect::<BTreeSet<_>>();
    let actual = commands
        .iter()
        .map(|command| command.id)
        .collect::<BTreeSet<_>>();
    if actual != required || actual.len() != commands.len() {
        return Err(invalid_schema(
            path,
            field.clone(),
            "platform row must contain exactly one result for every required command",
        ));
    }
    for (index, command) in commands.iter().enumerate() {
        command.validate(path, &format!("{field}[{index}]"))?;
    }
    Ok(())
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

fn validate_repo_paths(
    path: &Path,
    field: impl Into<String>,
    values: &[String],
) -> Result<(), FixtureLoadError> {
    let field = field.into();
    for (index, value) in values.iter().enumerate() {
        validate_repo_path(path, format!("{field}[{index}]"), value)?;
    }
    Ok(())
}

fn validate_repo_path(
    evidence_path: &Path,
    field: impl Into<String>,
    value: &str,
) -> Result<(), FixtureLoadError> {
    let field = field.into();
    validate_required_string(evidence_path, field.clone(), value)?;
    if value.contains('\\') || value.contains(':') {
        return Err(invalid_schema(
            evidence_path,
            field,
            "artifact paths must be relative slash-separated repo paths",
        ));
    }
    let path = Path::new(value);
    if path.is_absolute() {
        return Err(invalid_schema(
            evidence_path,
            field,
            "artifact paths must not be absolute",
        ));
    }
    if value
        .split('/')
        .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(invalid_schema(
            evidence_path,
            field,
            "artifact path contains an empty or parent segment",
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
    use super::{M5PlatformRuntimeEvidence, M5RuntimePlatform};
    use crate::FixtureLoadError;
    use std::path::PathBuf;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn valid_json(path: &str) -> String {
        format!(
            r#"{{
              "schema": "hera.m5_platform_runtime_evidence",
              "version": 1,
              "generated_at": "2026-07-04T00:00:00Z",
              "source_command": "terminal-cli measure-m5-platform --json-output evidence/m5/platform-runtime-evidence.json",
              "freshness_policy": {{
                "generated_after": "2026-07-04T00:00:00Z",
                "stale_after_days": 7
              }},
              "required_commands": [
                "cargo_check_workspace",
                "cargo_test_workspace",
                "cargo_doc_workspace",
                "validate_m5_compatibility",
                "verify_m5_replay"
              ],
              "platforms": {{
                "windows": {platform},
                "linux": {blocked_linux},
                "macos": {blocked_macos}
              }}
            }}"#,
            platform = platform_row("windows", "pass", path),
            blocked_linux = blocked_row("linux"),
            blocked_macos = blocked_row("macos")
        )
    }

    fn platform_row(platform: &str, status: &str, artifact_path: &str) -> String {
        format!(
            r#"{{
              "platform": "{platform}",
              "status": "{status}",
              "os": "{platform}",
              "target_triple": "x86_64-pc-windows-msvc",
              "rustc_version": "rustc 1.89.0",
              "generated_at": "2026-07-04T00:00:00Z",
              "commands": [
                {pass_check},
                {pass_test},
                {pass_doc},
                {pass_compat},
                {pass_replay}
              ],
              "artifact_paths": ["{artifact_path}"]
            }}"#,
            pass_check = pass_command("cargo_check_workspace", "cargo check --workspace", ""),
            pass_test = pass_command("cargo_test_workspace", "cargo test --workspace", ""),
            pass_doc = pass_command("cargo_doc_workspace", "cargo doc --workspace --no-deps", ""),
            pass_compat = pass_command(
                "validate_m5_compatibility",
                "terminal-cli validate-m5-compatibility evidence/m5/compatibility-matrix.json",
                ""
            ),
            pass_replay = pass_command(
                "verify_m5_replay",
                "terminal-cli verify-m5-replay crates/terminal-fixtures/fixtures/m5-replay --json-output evidence/m5/m5-replay-verification.json",
                r#""evidence/m5/m5-replay-verification.json""#
            )
        )
    }

    fn blocked_row(platform: &str) -> String {
        format!(
            r#"{{
              "platform": "{platform}",
              "status": "blocked",
              "os": "{platform}",
              "target_triple": "not_measured",
              "rustc_version": "not_measured",
              "generated_at": "2026-07-04T00:00:00Z",
              "commands": [
                {check},
                {test},
                {doc},
                {compat},
                {replay}
              ],
              "artifact_paths": [],
              "blocked_reason": "No {platform} runner is available in this local session."
            }}"#,
            check = blocked_command("cargo_check_workspace", "cargo check --workspace"),
            test = blocked_command("cargo_test_workspace", "cargo test --workspace"),
            doc = blocked_command("cargo_doc_workspace", "cargo doc --workspace --no-deps"),
            compat = blocked_command(
                "validate_m5_compatibility",
                "terminal-cli validate-m5-compatibility evidence/m5/compatibility-matrix.json"
            ),
            replay = blocked_command(
                "verify_m5_replay",
                "terminal-cli verify-m5-replay crates/terminal-fixtures/fixtures/m5-replay --json-output evidence/m5/m5-replay-verification.json"
            )
        )
    }

    fn pass_command(id: &str, command: &str, artifact_path: &str) -> String {
        let artifact_paths = if artifact_path.is_empty() {
            "[]".to_owned()
        } else {
            format!("[{artifact_path}]")
        };
        format!(
            r#"{{
              "id": "{id}",
              "command": "{command}",
              "status": "pass",
              "exit_code": 0,
              "duration_ms": 1,
              "stdout_summary": "ok",
              "stderr_summary": "",
              "artifact_paths": {artifact_paths}
            }}"#
        )
    }

    fn blocked_command(id: &str, command: &str) -> String {
        format!(
            r#"{{
              "id": "{id}",
              "command": "{command}",
              "status": "blocked",
              "exit_code": null,
              "duration_ms": null,
              "stdout_summary": "",
              "stderr_summary": "runner unavailable",
              "artifact_paths": [],
              "blocked_reason": "runner unavailable"
            }}"#
        )
    }

    #[test]
    fn m5_platform_runtime_evidence_validates_checked_in_artifact() {
        let root = workspace_root();
        let path = root.join("evidence/m5/platform-runtime-evidence.json");
        let evidence =
            M5PlatformRuntimeEvidence::from_path(&path).expect("M5 platform evidence should load");

        assert_eq!(
            evidence.platforms.windows.platform,
            M5RuntimePlatform::Windows
        );
        assert_eq!(evidence.platforms.linux.platform, M5RuntimePlatform::Linux);
        assert_eq!(evidence.platforms.macos.platform, M5RuntimePlatform::Macos);
    }

    #[test]
    fn platform_evidence_rejects_absolute_artifact_paths() {
        let json = valid_json("evidence/m5/platform-runtime-evidence.json").replace(
            "evidence/m5/platform-runtime-evidence.json",
            "C:/Users/Arthur/private.json",
        );

        let error = M5PlatformRuntimeEvidence::from_json_str("platform.json", &json)
            .expect_err("absolute artifact path must fail");

        assert!(matches!(
            error,
            FixtureLoadError::InvalidSchema { field, .. }
                if field == "platforms.windows.artifact_paths[0]"
        ));
    }

    #[test]
    fn platform_evidence_requires_all_command_rows() {
        let json = valid_json("evidence/m5/platform-runtime-evidence.json").replace(
            r#""id": "cargo_doc_workspace""#,
            r#""id": "cargo_check_workspace""#,
        );

        let error = M5PlatformRuntimeEvidence::from_json_str("platform.json", &json)
            .expect_err("duplicate command id must fail");

        assert!(matches!(
            error,
            FixtureLoadError::InvalidSchema { field, .. }
                if field == "platforms.windows.commands"
        ));
    }

    #[test]
    fn stale_platform_row_accepts_prior_command_results_with_reason() {
        let json = valid_json("evidence/m5/platform-runtime-evidence.json")
            .replacen(r#""status": "pass""#, r#""status": "stale""#, 1)
            .replace(
                r#""artifact_paths": ["evidence/m5/platform-runtime-evidence.json"]"#,
                r#""artifact_paths": ["evidence/m5/platform-runtime-evidence.json"],
              "blocked_reason": "Evidence is older than the M5 freshness policy.""#,
            );

        M5PlatformRuntimeEvidence::from_json_str("platform.json", &json)
            .expect("stale rows can preserve prior pass command results");
    }

    #[test]
    fn blocked_platform_rows_require_reason() {
        let json = valid_json("evidence/m5/platform-runtime-evidence.json").replace(
            r#""blocked_reason": "No linux runner is available in this local session.""#,
            r#""blocked_reason": """#,
        );

        let error = M5PlatformRuntimeEvidence::from_json_str("platform.json", &json)
            .expect_err("blocked row without reason must fail");

        assert!(matches!(
            error,
            FixtureLoadError::InvalidSchema { field, .. }
                if field == "platforms.linux.blocked_reason"
        ));
    }
}
