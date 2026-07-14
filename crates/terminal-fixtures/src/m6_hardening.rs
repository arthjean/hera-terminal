use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::FixtureLoadError;

pub const M6_MISMATCH_CLASSIFICATION_SCHEMA: &str = "hera.m6_mismatch_classification";
pub const M6_MISMATCH_CLASSIFICATION_VERSION: u32 = 1;
pub const M6_MAX_MISMATCH_CLASSIFICATION_BYTES: u64 = 64 * 1024;
pub const M6_HARDENING_TEST_MANIFEST_SCHEMA: &str = "hera.m6_hardening_test_manifest";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M6MismatchClassification {
    schema: String,
    version: u32,
    generated_at: String,
    source_evidence: String,
    historical_p0_mismatches: HistoricalMismatches,
    historical_unsupported_checkpoints: HistoricalUnsupported,
    diagnostic_contract: DiagnosticContract,
    deterministic_reproducer: DeterministicReproducer,
}

impl M6MismatchClassification {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref();
        let metadata = fs::metadata(path).map_err(|error| io_error(path, error))?;
        if metadata.len() > M6_MAX_MISMATCH_CLASSIFICATION_BYTES {
            return Err(invalid(path, "$", "classification artifact exceeds 64 KiB"));
        }
        let raw = fs::read_to_string(path).map_err(|error| io_error(path, error))?;
        let artifact: Self = serde_json::from_str(&raw)
            .map_err(|error| invalid(path, "$", format!("invalid JSON: {error}")))?;
        artifact.validate(path)?;
        Ok(artifact)
    }

    fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        if self.schema != M6_MISMATCH_CLASSIFICATION_SCHEMA
            || self.version != M6_MISMATCH_CLASSIFICATION_VERSION
        {
            return Err(invalid(path, "schema", "unexpected schema or version"));
        }
        if self.generated_at.trim().is_empty()
            || self.source_evidence != "evidence/m6/m6-exit-evidence.json"
        {
            return Err(invalid(
                path,
                "source_evidence",
                "unexpected historical source",
            ));
        }
        self.historical_p0_mismatches.validate(path)?;
        self.historical_unsupported_checkpoints.validate(path)?;
        self.diagnostic_contract.validate(path)?;
        self.deterministic_reproducer.validate(path)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoricalMismatches {
    total: u64,
    classes: MismatchClasses,
}

impl HistoricalMismatches {
    fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        if self.total != 219 || self.classes.total() != self.total {
            return Err(invalid(
                path,
                "historical_p0_mismatches",
                "class counts must sum to 219",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MismatchClasses {
    stale_checkpoint: u64,
    terminal_semantic_divergence: u64,
    adapter_divergence: u64,
    resize_generation_divergence: u64,
    unclassified_from_public_evidence: u64,
}

impl MismatchClasses {
    fn total(&self) -> u64 {
        self.stale_checkpoint
            .saturating_add(self.terminal_semantic_divergence)
            .saturating_add(self.adapter_divergence)
            .saturating_add(self.resize_generation_divergence)
            .saturating_add(self.unclassified_from_public_evidence)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoricalUnsupported {
    total: u64,
    field_path: String,
    classification: String,
}

impl HistoricalUnsupported {
    fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        if self.total != 1
            || self.field_path != "unclassified_from_public_evidence"
            || self.classification != "unclassified_from_public_evidence"
        {
            return Err(invalid(
                path,
                "historical_unsupported_checkpoints",
                "unsupported checkpoint must remain separate and unclassified",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DiagnosticContract {
    allowed_fields: Vec<String>,
    forbidden_fields: Vec<String>,
}

impl DiagnosticContract {
    fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        let allowed = self
            .allowed_fields
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let expected = [
            "active_screen_class",
            "checkpoint_duration_ms",
            "field_path",
            "outcome_class",
            "pane_pseudonym",
            "pty_batch_sequence",
            "resize_generation",
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        if allowed != expected || allowed.len() != self.allowed_fields.len() {
            return Err(invalid(
                path,
                "diagnostic_contract.allowed_fields",
                "allowed fields must match the scrubbed checkpoint contract exactly",
            ));
        }
        let forbidden = self
            .forbidden_fields
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        for required in [
            "bytes",
            "cells",
            "commands",
            "identities",
            "lines",
            "paths",
            "prompts",
        ] {
            if !forbidden.contains(required) {
                return Err(invalid(
                    path,
                    "diagnostic_contract.forbidden_fields",
                    format!("missing {required}"),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeterministicReproducer {
    test: String,
    class: String,
    first_differing_field: String,
    historical_attribution: bool,
    note: String,
}

impl DeterministicReproducer {
    fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        if self.test
            != "terminal::hera_dogfood::comparison::legacy_stale_checkpoint_reproducer_has_a_stable_field"
            || self.class != "stale_checkpoint"
            || self.first_differing_field != "$.viewport_lines[0]"
            || self.historical_attribution
            || self.note.trim().is_empty()
        {
            return Err(invalid(
                path,
                "deterministic_reproducer",
                "reproducer must stay deterministic and must not relabel historical counters",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M6HardeningTestManifest {
    schema: String,
    version: u32,
    generated_at: String,
    iterations: u32,
    filters: Vec<HardeningTestFilter>,
    status: String,
    flaky_failures: u64,
    sequence_gaps: u64,
    dropped_bytes: u64,
    leaked_worker_threads: u64,
}

impl M6HardeningTestManifest {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, FixtureLoadError> {
        let path = path.as_ref();
        let raw = fs::read_to_string(path).map_err(|error| io_error(path, error))?;
        let manifest: Self = serde_json::from_str(&raw)
            .map_err(|error| invalid(path, "$", format!("invalid JSON: {error}")))?;
        manifest.validate(path)?;
        Ok(manifest)
    }

    fn validate(&self, path: &Path) -> Result<(), FixtureLoadError> {
        if self.schema != M6_HARDENING_TEST_MANIFEST_SCHEMA
            || self.version != 1
            || self.generated_at.trim().is_empty()
            || self.iterations != 25
            || self.status != "pass"
        {
            return Err(invalid(path, "$", "hardening manifest header is invalid"));
        }
        let expected = [
            ("comparison", "terminal::hera_dogfood::comparison"),
            ("authoritative", "terminal::hera_dogfood::authoritative"),
            ("pty_session", "terminal::pty_session"),
        ];
        if self.filters.len() != expected.len() {
            return Err(invalid(
                path,
                "filters",
                "three focused filters are required",
            ));
        }
        for (id, filter) in expected {
            let Some(row) = self.filters.iter().find(|row| row.id == id) else {
                return Err(invalid(path, "filters", format!("missing {id}")));
            };
            if row.filter != filter
                || row.selected_per_iteration == 0
                || row.total_executions
                    != u64::from(row.selected_per_iteration) * u64::from(self.iterations)
            {
                return Err(invalid(
                    path,
                    format!("filters.{id}"),
                    "selection count is invalid",
                ));
            }
        }
        if self.flaky_failures != 0
            || self.sequence_gaps != 0
            || self.dropped_bytes != 0
            || self.leaked_worker_threads != 0
        {
            return Err(invalid(
                path,
                "status",
                "passing hardening evidence requires zero failures, gaps, drops and leaked workers",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HardeningTestFilter {
    id: String,
    filter: String,
    selected_per_iteration: u32,
    total_executions: u64,
}

fn io_error(path: &Path, error: std::io::Error) -> FixtureLoadError {
    FixtureLoadError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}

fn invalid(path: &Path, field: impl Into<String>, message: impl Into<String>) -> FixtureLoadError {
    FixtureLoadError::InvalidSchema {
        path: PathBuf::from(path),
        field: field.into(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::{M6HardeningTestManifest, M6MismatchClassification};

    #[test]
    fn checked_in_classification_is_valid() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../evidence/m6/m6-mismatch-classification.json");
        M6MismatchClassification::from_path(path).expect("checked-in M6 classification");
    }

    #[test]
    fn checked_in_hardening_manifest_is_valid() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../evidence/m6/ep007-hardening-test-manifest.json");
        M6HardeningTestManifest::from_path(path).expect("checked-in EP-007 test manifest");
    }
}
