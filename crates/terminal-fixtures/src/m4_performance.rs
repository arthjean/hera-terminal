use serde::{Deserialize, Serialize};

pub const M4_BENCHMARK_EVIDENCE_SCHEMA: &str = "hera.m4_benchmark_evidence";
pub const M4_MEMORY_PROFILE_SCHEMA: &str = "hera.m4_memory_profile";
pub const M4_PERFORMANCE_REPORT_SCHEMA: &str = "hera.m4_performance_report";
pub const M4_PERFORMANCE_THRESHOLDS_SCHEMA: &str = "hera.m4_performance_thresholds";
pub const M4_PERFORMANCE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct M4MachineMetadata {
    pub os: String,
    pub arch: String,
    pub rustc: String,
    pub cargo_profile: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M4PerformanceStatus {
    Pass,
    Fail,
    Blocked,
    BaselineCreated,
    Partial,
    UnstableExcluded,
}

impl M4PerformanceStatus {
    #[must_use]
    pub const fn is_failure(self) -> bool {
        matches!(self, Self::Fail)
    }

    #[must_use]
    pub const fn is_blocking_gap(self) -> bool {
        matches!(self, Self::Blocked | Self::Partial)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Blocked => "blocked",
            Self::BaselineCreated => "baseline_created",
            Self::Partial => "partial",
            Self::UnstableExcluded => "unstable_excluded",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct M4BenchmarkEvidence {
    pub schema: String,
    pub version: u32,
    pub generated_at: String,
    pub hera_commit: String,
    pub command: String,
    pub machine: M4MachineMetadata,
    pub status: M4PerformanceStatus,
    pub measurements: Vec<M4BenchmarkMeasurement>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct M4BenchmarkMeasurement {
    pub id: String,
    pub operation: M4BenchmarkOperation,
    pub input_name: String,
    pub input_bytes: usize,
    pub logical_lines: usize,
    pub iterations: u64,
    pub total_nanos: u128,
    pub nanos_per_iteration: u128,
    pub throughput_bytes_per_second: Option<u64>,
    pub status: M4PerformanceStatus,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M4BenchmarkOperation {
    ByteIngest,
    SnapshotGeneration,
    Replay,
    SnapshotComparison,
}

impl M4BenchmarkOperation {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ByteIngest => "byte_ingest",
            Self::SnapshotGeneration => "snapshot_generation",
            Self::Replay => "replay",
            Self::SnapshotComparison => "snapshot_comparison",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct M4MemoryProfileEvidence {
    pub schema: String,
    pub version: u32,
    pub generated_at: String,
    pub hera_commit: String,
    pub command: String,
    pub machine: M4MachineMetadata,
    pub status: M4PerformanceStatus,
    pub scenarios: Vec<M4MemoryScenario>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct M4MemoryScenario {
    pub id: String,
    pub logical_lines_target: usize,
    pub logical_lines_processed: usize,
    pub terminal_columns: usize,
    pub terminal_rows: usize,
    pub visible_rows: usize,
    pub scrollback_rows: usize,
    pub scrollback_line_budget: usize,
    pub scrollback_byte_budget: usize,
    pub hera_owned_bytes: usize,
    pub discarded_rows: usize,
    pub peak_process_memory_bytes: Option<u64>,
    pub process_memory_source: String,
    pub elapsed_ms: u128,
    pub status: M4PerformanceStatus,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct M4PerformanceThresholds {
    pub schema: String,
    pub version: u32,
    pub generated_at: String,
    pub thresholds: Vec<M4MetricThreshold>,
}

impl M4PerformanceThresholds {
    #[must_use]
    pub fn threshold_for(&self, metric_id: &str) -> Option<&M4MetricThreshold> {
        self.thresholds
            .iter()
            .find(|threshold| threshold.metric_id == metric_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct M4MetricThreshold {
    pub metric_id: String,
    pub hard_gate: bool,
    #[serde(default)]
    pub unstable: bool,
    pub max_nanos_per_iteration: Option<u128>,
    pub max_hera_owned_bytes: Option<usize>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct M4PerformanceReport {
    pub schema: String,
    pub version: u32,
    pub generated_at: String,
    pub benchmark_input: String,
    pub memory_input: String,
    pub thresholds_input: String,
    pub status: M4PerformanceStatus,
    pub evaluations: Vec<M4MetricEvaluation>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct M4MetricEvaluation {
    pub metric_id: String,
    pub source: M4MetricSource,
    pub observed_nanos_per_iteration: Option<u128>,
    pub observed_hera_owned_bytes: Option<usize>,
    pub status: M4PerformanceStatus,
    pub threshold: Option<M4MetricThreshold>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M4MetricSource {
    Benchmark,
    MemoryProfile,
}

impl M4MetricSource {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Benchmark => "benchmark",
            Self::MemoryProfile => "memory_profile",
        }
    }
}

#[must_use]
pub fn m4_synthetic_workload(logical_lines: usize) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(logical_lines.saturating_mul(20));
    for line in 0..logical_lines {
        bytes.extend_from_slice(b"m4-line-");
        bytes.extend_from_slice(line.to_string().as_bytes());
        bytes.extend_from_slice(b"\r\n");
    }
    bytes
}

#[must_use]
pub fn m4_rollup_status(
    statuses: impl IntoIterator<Item = M4PerformanceStatus>,
) -> M4PerformanceStatus {
    let mut saw_status = false;
    let mut saw_baseline = false;
    let mut saw_gap = false;
    let mut saw_pass = false;
    let mut saw_unstable = false;

    for status in statuses {
        saw_status = true;
        if status.is_failure() {
            return M4PerformanceStatus::Fail;
        }
        if status.is_blocking_gap() {
            saw_gap = true;
        }
        match status {
            M4PerformanceStatus::Pass => saw_pass = true,
            M4PerformanceStatus::BaselineCreated => saw_baseline = true,
            M4PerformanceStatus::UnstableExcluded => saw_unstable = true,
            M4PerformanceStatus::Fail
            | M4PerformanceStatus::Blocked
            | M4PerformanceStatus::Partial => {}
        }
    }

    if !saw_status {
        return M4PerformanceStatus::Fail;
    }
    if saw_gap {
        return M4PerformanceStatus::Partial;
    }
    if saw_baseline {
        return M4PerformanceStatus::BaselineCreated;
    }
    if saw_pass {
        return M4PerformanceStatus::Pass;
    }
    if saw_unstable {
        return M4PerformanceStatus::UnstableExcluded;
    }

    M4PerformanceStatus::Fail
}

#[cfg(test)]
mod tests {
    use super::{M4PerformanceStatus, m4_rollup_status, m4_synthetic_workload};

    #[test]
    fn m4_synthetic_workload_is_private_safe_and_deterministic() {
        let first = m4_synthetic_workload(3);
        let second = m4_synthetic_workload(3);

        assert_eq!(first, second);
        assert_eq!(first, b"m4-line-0\r\nm4-line-1\r\nm4-line-2\r\n");
    }

    #[test]
    fn m4_rollup_keeps_failed_and_baseline_statuses_visible() {
        assert_eq!(
            m4_rollup_status([
                M4PerformanceStatus::Pass,
                M4PerformanceStatus::BaselineCreated
            ]),
            M4PerformanceStatus::BaselineCreated
        );
        assert_eq!(
            m4_rollup_status([M4PerformanceStatus::Pass, M4PerformanceStatus::Fail]),
            M4PerformanceStatus::Fail
        );
        assert_eq!(
            m4_rollup_status([M4PerformanceStatus::Blocked]),
            M4PerformanceStatus::Partial
        );
    }
}
