use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{FixtureLoadError, invalid_schema, recording_schema_error};

pub const M4_EVIDENCE_MANIFEST_SCHEMA: &str = "hera.m4_evidence_manifest";
pub const M4_EVIDENCE_MANIFEST_VERSION: u32 = 1;
pub const M4_MAX_EVIDENCE_MANIFEST_BYTES: u64 = 1024 * 1024;
pub const M4_MAX_PUBLIC_ARTIFACT_BYTES: u64 = 4 * 1024 * 1024;
const M4_MAX_EVIDENCE_ARTIFACTS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M4EvidenceManifest {
    pub schema: String,
    pub version: u32,
    pub generated_at: String,
    pub hera_commit: String,
    pub redaction: M4RedactionPolicy,
    pub artifacts: Vec<M4EvidenceArtifact>,
}

impl M4EvidenceManifest {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref();
        let raw = read_manifest_file_capped(path)?;
        Self::from_json_str(path, &raw)
    }

    pub fn from_json_str(path: impl AsRef<Path>, json: &str) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref().to_path_buf();
        if json.len() as u64 > M4_MAX_EVIDENCE_MANIFEST_BYTES {
            return Err(invalid_schema(
                &path,
                "$",
                format!(
                    "M4 evidence manifest JSON is {} bytes, maximum is {M4_MAX_EVIDENCE_MANIFEST_BYTES}",
                    json.len()
                ),
            ));
        }

        let mut deserializer = serde_json::Deserializer::from_str(json);
        let manifest: Self =
            serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
                recording_schema_error(&path, &error.path().to_string(), &error.inner().to_string())
            })?;
        manifest.validate(&path)?;
        Ok(manifest)
    }

    pub fn validate_public_artifact_files(
        &self,
        repo_root: impl AsRef<Path>,
    ) -> Result<(), FixtureLoadError> {
        let repo_root = repo_root.as_ref();
        for (index, artifact) in self.artifacts.iter().enumerate() {
            if !artifact.privacy.requires_public_scan() {
                continue;
            }

            let path = repo_root.join(&artifact.path);
            let raw = read_public_artifact_capped(&path)?;
            let raw_lower = raw.to_ascii_lowercase();
            for pattern in &self.redaction.reject_patterns {
                let pattern_lower = pattern.to_ascii_lowercase();
                if raw.contains(pattern) || raw_lower.contains(&pattern_lower) {
                    return Err(invalid_schema(
                        &path,
                        format!("artifacts[{index}].path"),
                        format!("public artifact contains rejected redaction pattern {pattern:?}"),
                    ));
                }
            }
            if artifact.requires_raw_field_scan() {
                for field in [
                    "raw_transcript",
                    "raw_bytes",
                    "terminal_bytes",
                    "prompt_text",
                ] {
                    if raw_lower.contains(field) {
                        return Err(invalid_schema(
                            &path,
                            format!("artifacts[{index}].path"),
                            format!("public artifact contains raw transcript field {field:?}"),
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    pub fn artifacts(&self) -> &[M4EvidenceArtifact] {
        &self.artifacts
    }

    fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        if self.schema != M4_EVIDENCE_MANIFEST_SCHEMA {
            return Err(invalid_schema(
                path,
                "schema",
                format!("expected {M4_EVIDENCE_MANIFEST_SCHEMA}"),
            ));
        }
        if self.version != M4_EVIDENCE_MANIFEST_VERSION {
            return Err(invalid_schema(
                path,
                "version",
                format!("expected {M4_EVIDENCE_MANIFEST_VERSION}"),
            ));
        }
        validate_timestamp(path, "generated_at", &self.generated_at)?;
        validate_required_string(path, "hera_commit", &self.hera_commit)?;
        self.redaction.validate(path)?;

        if self.artifacts.is_empty() {
            return Err(invalid_schema(
                path,
                "artifacts",
                "at least one artifact is required",
            ));
        }
        if self.artifacts.len() > M4_MAX_EVIDENCE_ARTIFACTS {
            return Err(invalid_schema(
                path,
                "artifacts",
                format!(
                    "manifest has {} artifacts, maximum is {M4_MAX_EVIDENCE_ARTIFACTS}",
                    self.artifacts.len()
                ),
            ));
        }

        let mut ids = BTreeSet::new();
        for (index, artifact) in self.artifacts.iter().enumerate() {
            artifact.validate(path, index, &self.redaction)?;
            if !ids.insert(artifact.id.as_str()) {
                return Err(invalid_schema(
                    path,
                    format!("artifacts[{index}].id"),
                    "artifact id must be unique",
                ));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M4RedactionPolicy {
    pub version: u32,
    pub updated_at: String,
    pub reject_patterns: Vec<String>,
    pub public_privacy_classes: Vec<M4ArtifactPrivacy>,
}

impl M4RedactionPolicy {
    fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        if self.version == 0 {
            return Err(invalid_schema(
                path,
                "redaction.version",
                "version must be non-zero",
            ));
        }
        validate_timestamp(path, "redaction.updated_at", &self.updated_at)?;
        if self.reject_patterns.is_empty() {
            return Err(invalid_schema(
                path,
                "redaction.reject_patterns",
                "at least one reject pattern is required",
            ));
        }
        for (index, pattern) in self.reject_patterns.iter().enumerate() {
            validate_required_string(path, format!("redaction.reject_patterns[{index}]"), pattern)?;
        }
        if !self
            .public_privacy_classes
            .contains(&M4ArtifactPrivacy::PublicSummary)
            || !self
                .public_privacy_classes
                .contains(&M4ArtifactPrivacy::ScrubbedPublic)
        {
            return Err(invalid_schema(
                path,
                "redaction.public_privacy_classes",
                "public_summary and scrubbed_public must be public scan classes",
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M4EvidenceArtifact {
    pub id: String,
    #[serde(rename = "type")]
    pub artifact_type: String,
    pub path: String,
    pub source_command: String,
    pub generated_at: String,
    pub redaction_checked_at: String,
    pub privacy: M4ArtifactPrivacy,
    #[serde(default)]
    pub feature_flags: Vec<String>,
    pub reproducible: bool,
    #[serde(default)]
    pub non_reproducible_reason: Option<String>,
    #[serde(default)]
    pub notes: Vec<String>,
}

impl M4EvidenceArtifact {
    fn validate(
        &self,
        manifest_path: &Path,
        index: usize,
        redaction: &M4RedactionPolicy,
    ) -> Result<(), FixtureLoadError> {
        validate_required_string(manifest_path, format!("artifacts[{index}].id"), &self.id)?;
        validate_required_string(
            manifest_path,
            format!("artifacts[{index}].type"),
            &self.artifact_type,
        )?;
        validate_artifact_path(
            manifest_path,
            format!("artifacts[{index}].path"),
            &self.path,
        )?;
        validate_required_string(
            manifest_path,
            format!("artifacts[{index}].source_command"),
            &self.source_command,
        )?;
        validate_timestamp(
            manifest_path,
            format!("artifacts[{index}].generated_at"),
            &self.generated_at,
        )?;
        validate_timestamp(
            manifest_path,
            format!("artifacts[{index}].redaction_checked_at"),
            &self.redaction_checked_at,
        )?;
        if self.privacy == M4ArtifactPrivacy::RawLocal {
            return Err(invalid_schema(
                manifest_path,
                format!("artifacts[{index}].privacy"),
                "raw_local artifacts must not be listed in the public M4 manifest",
            ));
        }
        if self.privacy.requires_public_scan()
            && !redaction.public_privacy_classes.contains(&self.privacy)
        {
            return Err(invalid_schema(
                manifest_path,
                format!("artifacts[{index}].privacy"),
                "public artifact privacy class is not enabled by redaction policy",
            ));
        }
        if self.privacy.requires_public_scan()
            && self.redaction_checked_at.as_str() < redaction.updated_at.as_str()
        {
            return Err(invalid_schema(
                manifest_path,
                format!("artifacts[{index}].redaction_checked_at"),
                "artifact was checked before the current redaction policy",
            ));
        }
        if !self.reproducible {
            let has_reason = self
                .non_reproducible_reason
                .as_deref()
                .is_some_and(|reason| !reason.trim().is_empty());
            if !has_reason {
                return Err(invalid_schema(
                    manifest_path,
                    format!("artifacts[{index}].non_reproducible_reason"),
                    "non-reproducible artifacts must explain why",
                ));
            }
        }

        for (flag_index, flag) in self.feature_flags.iter().enumerate() {
            validate_required_string(
                manifest_path,
                format!("artifacts[{index}].feature_flags[{flag_index}]"),
                flag,
            )?;
        }

        Ok(())
    }

    fn requires_raw_field_scan(&self) -> bool {
        self.path.ends_with(".json")
            || self.artifact_type.contains("replay")
            || self.artifact_type.contains("dogfood")
            || self.artifact_type.contains("demo")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M4ArtifactPrivacy {
    PublicSummary,
    ScrubbedPublic,
    LocalPrivate,
    RawLocal,
}

impl M4ArtifactPrivacy {
    const fn requires_public_scan(self) -> bool {
        matches!(self, Self::PublicSummary | Self::ScrubbedPublic)
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

fn validate_artifact_path(
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
            "public artifact paths must be relative slash-separated repo paths",
        ));
    }
    let path = Path::new(value);
    if path.is_absolute() {
        return Err(invalid_schema(
            manifest_path,
            field,
            "public artifact paths must not be absolute",
        ));
    }
    if value
        .split('/')
        .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(invalid_schema(
            manifest_path,
            field,
            "public artifact path contains an empty or parent segment",
        ));
    }

    Ok(())
}

fn read_manifest_file_capped(path: &Path) -> Result<String, FixtureLoadError> {
    read_text_file_capped(path, M4_MAX_EVIDENCE_MANIFEST_BYTES, "M4 evidence manifest")
}

fn read_public_artifact_capped(path: &Path) -> Result<String, FixtureLoadError> {
    read_text_file_capped(path, M4_MAX_PUBLIC_ARTIFACT_BYTES, "M4 public artifact")
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
    use std::fs;
    use std::path::PathBuf;

    use super::{M4ArtifactPrivacy, M4EvidenceManifest};
    use crate::FixtureLoadError;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn temp_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("temp dir should be writable");
        path
    }

    fn valid_manifest_json(path: &str) -> String {
        format!(
            r#"{{
              "schema": "hera.m4_evidence_manifest",
              "version": 1,
              "generated_at": "2026-07-04T00:00:00Z",
              "hera_commit": "d897c20",
              "redaction": {{
                "version": 1,
                "updated_at": "2026-07-04T00:00:00Z",
                "reject_patterns": ["C:\\\\Users\\\\", "/home/", "OPENAI_API_KEY"],
                "public_privacy_classes": ["public_summary", "scrubbed_public"]
              }},
              "artifacts": [{{
                "id": "artifact",
                "type": "report",
                "path": "{path}",
                "source_command": "manual",
                "generated_at": "2026-07-04T00:00:00Z",
                "redaction_checked_at": "2026-07-04T00:00:00Z",
                "privacy": "public_summary",
                "reproducible": true
              }}]
            }}"#
        )
    }

    #[test]
    fn m4_evidence_manifest_validates_checked_in_contract() {
        let root = workspace_root();
        let manifest_path = root.join("evidence/m4/evidence-manifest.json");
        let manifest =
            M4EvidenceManifest::from_path(&manifest_path).expect("M4 manifest should load");

        for expected_id in [
            "m4_benchmark_summary",
            "m4_memory_profile",
            "m4_performance_thresholds",
            "m4_performance_report",
            "m4_benchmarks_and_memory_doc",
            "m4_replay_fixture_basic_shell",
            "m4_replay_fixture_resize_and_wrap",
            "m4_replay_fixture_alternate_screen",
            "m4_replay_verification",
            "m4_replay_event_stream_basic_shell",
            "m4_replay_and_dogfood_demo_doc",
            "m4_headless_embedder_example",
            "m4_pty_boundary_example",
            "m4_api_and_package_readiness_doc",
            "m4_api_audit",
            "m4_package_readiness",
            "m4_oss_security_baseline",
            "paneflow_dogfood_demo_2026_07_04_ep004",
        ] {
            assert!(
                manifest
                    .artifacts()
                    .iter()
                    .any(|artifact| artifact.id == expected_id),
                "missing M4 artifact {expected_id}"
            );
        }
        assert!(
            manifest
                .artifacts()
                .iter()
                .any(|artifact| artifact.privacy == M4ArtifactPrivacy::PublicSummary)
        );
        manifest
            .validate_public_artifact_files(&root)
            .expect("checked-in M4 artifacts should pass redaction validation");
    }

    #[test]
    fn m4_evidence_manifest_rejects_raw_local_artifact() {
        let json = valid_manifest_json("docs/m4-public-proof-report.md").replace(
            "\"privacy\": \"public_summary\"",
            "\"privacy\": \"raw_local\"",
        );

        let error = M4EvidenceManifest::from_json_str("manifest.json", &json)
            .expect_err("raw_local must be rejected");

        assert!(matches!(
            error,
            FixtureLoadError::InvalidSchema { field, .. } if field == "artifacts[0].privacy"
        ));
    }

    #[test]
    fn m4_evidence_manifest_flags_stale_redaction_check() {
        let json = valid_manifest_json("docs/m4-public-proof-report.md").replace(
            "\"redaction_checked_at\": \"2026-07-04T00:00:00Z\"",
            "\"redaction_checked_at\": \"2026-07-03T00:00:00Z\"",
        );

        let error = M4EvidenceManifest::from_json_str("manifest.json", &json)
            .expect_err("stale redaction check must be rejected");

        assert!(matches!(
            error,
            FixtureLoadError::InvalidSchema { field, .. } if field == "artifacts[0].redaction_checked_at"
        ));
    }

    #[test]
    fn m4_evidence_manifest_rejects_private_path_in_public_content() {
        let root = temp_dir("hera-m4-evidence-redaction");
        let artifact_path = root.join("artifact.json");
        fs::write(&artifact_path, r#"{"summary":"c:\\users\\arthur\\secret"}"#)
            .expect("artifact should be writable");
        let manifest = M4EvidenceManifest::from_json_str(
            "manifest.json",
            &valid_manifest_json("artifact.json"),
        )
        .expect("manifest should load");

        let error = manifest
            .validate_public_artifact_files(&root)
            .expect_err("private path must be rejected");
        let _ = fs::remove_dir_all(&root);

        assert!(matches!(
            error,
            FixtureLoadError::InvalidSchema { field, .. } if field == "artifacts[0].path"
        ));
    }
}
