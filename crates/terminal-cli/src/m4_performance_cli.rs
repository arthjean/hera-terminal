use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde::de::DeserializeOwned;
use terminal_core::{ScrollbackConfig, Terminal, TerminalConfig};
use terminal_fixtures::{
    M4_BENCHMARK_EVIDENCE_SCHEMA, M4_MEMORY_PROFILE_SCHEMA, M4_PERFORMANCE_REPORT_SCHEMA,
    M4_PERFORMANCE_THRESHOLDS_SCHEMA, M4_PERFORMANCE_VERSION, M4BenchmarkEvidence,
    M4BenchmarkMeasurement, M4BenchmarkOperation, M4MachineMetadata, M4MemoryProfileEvidence,
    M4MemoryScenario, M4MetricEvaluation, M4MetricSource, M4MetricThreshold, M4PerformanceReport,
    M4PerformanceStatus, M4PerformanceThresholds, first_snapshot_difference, m4_rollup_status,
    m4_synthetic_workload, snapshot_terminal,
};

use super::CommandOutcome;

const DEFAULT_BENCHMARK_ITERATIONS: u64 = 8;
const DEFAULT_BENCHMARK_LINES: usize = 2_000;
const DEFAULT_MEMORY_SCENARIOS: &[usize] = &[10_000, 100_000, 1_000_000];
const DEFAULT_MEMORY_TIMEOUT_MS: u64 = 120_000;
const DEFAULT_SCROLLBACK_LINES: usize = 10_000;
const DEFAULT_SCROLLBACK_BYTES: usize = 8 * 1024 * 1024;
const M4_COLUMNS: usize = 80;
const M4_ROWS: usize = 24;

pub(crate) fn benchmark_command(args: &[OsString]) -> CommandOutcome {
    let options = match BenchmarkOptions::parse(args) {
        Ok(options) => options,
        Err(error) => return CommandOutcome::failure(2, error),
    };

    let evidence = build_benchmark_evidence(&options, args);
    let code = if evidence.status.is_failure() { 1 } else { 0 };
    if let Err(error) = write_json(&options.output, &evidence) {
        return CommandOutcome::failure(1, error);
    }

    CommandOutcome::complete(
        code,
        format!("wrote M4 benchmark evidence: {}", options.output.display()),
        String::new(),
    )
}

pub(crate) fn memory_profile_command(args: &[OsString]) -> CommandOutcome {
    let options = match MemoryProfileOptions::parse(args) {
        Ok(options) => options,
        Err(error) => return CommandOutcome::failure(2, error),
    };

    let evidence = match build_memory_profile_evidence(&options, args) {
        Ok(evidence) => evidence,
        Err(error) => return CommandOutcome::failure(1, error),
    };
    let code = if evidence.status.is_failure() { 1 } else { 0 };
    if let Err(error) = write_json(&options.output, &evidence) {
        return CommandOutcome::failure(1, error);
    }

    CommandOutcome::complete(
        code,
        format!(
            "wrote M4 memory profile evidence: {}",
            options.output.display()
        ),
        String::new(),
    )
}

pub(crate) fn performance_report_command(args: &[OsString]) -> CommandOutcome {
    let options = match ReportOptions::parse(args) {
        Ok(options) => options,
        Err(error) => return CommandOutcome::failure(2, error),
    };

    let benchmark = match read_json::<M4BenchmarkEvidence>(&options.benchmark) {
        Ok(benchmark) => benchmark,
        Err(error) => return CommandOutcome::failure(1, error),
    };
    let memory = match read_json::<M4MemoryProfileEvidence>(&options.memory) {
        Ok(memory) => memory,
        Err(error) => return CommandOutcome::failure(1, error),
    };
    let thresholds = match read_json::<M4PerformanceThresholds>(&options.thresholds) {
        Ok(thresholds) => thresholds,
        Err(error) => return CommandOutcome::failure(1, error),
    };
    if let Err(error) = validate_report_inputs(&benchmark, &memory, &thresholds) {
        return CommandOutcome::failure(1, error);
    }

    let report = build_performance_report(&options, &benchmark, &memory, &thresholds);
    let markdown = render_performance_markdown(&report, &benchmark, &memory);
    let code = if report.status.is_failure() { 1 } else { 0 };

    if let Err(error) = write_json(&options.json_output, &report) {
        return CommandOutcome::failure(1, error);
    }
    if let Err(error) = write_text(&options.markdown_output, &markdown) {
        return CommandOutcome::failure(1, error);
    }

    CommandOutcome::complete(
        code,
        format!(
            "wrote M4 performance report: {} and {}",
            options.json_output.display(),
            options.markdown_output.display()
        ),
        String::new(),
    )
}

#[derive(Debug, Clone)]
struct BenchmarkOptions {
    output: PathBuf,
    iterations: u64,
    logical_lines: usize,
}

impl BenchmarkOptions {
    fn parse(args: &[OsString]) -> Result<Self, String> {
        let mut output = None;
        let mut iterations = DEFAULT_BENCHMARK_ITERATIONS;
        let mut logical_lines = DEFAULT_BENCHMARK_LINES;
        let mut index = 0;

        while index < args.len() {
            match arg_str(args, index)? {
                "--output" => {
                    output = Some(PathBuf::from(value_arg(args, index, "--output")?));
                    index += 2;
                }
                "--iterations" => {
                    iterations = parse_u64(value_arg(args, index, "--iterations")?, "iterations")?;
                    index += 2;
                }
                "--logical-lines" => {
                    logical_lines =
                        parse_usize(value_arg(args, index, "--logical-lines")?, "logical lines")?;
                    index += 2;
                }
                _ => return Err(m4_benchmark_usage()),
            }
        }

        Ok(Self {
            output: output.ok_or_else(m4_benchmark_usage)?,
            iterations,
            logical_lines,
        })
    }
}

#[derive(Debug, Clone)]
struct MemoryProfileOptions {
    output: PathBuf,
    scenarios: Vec<usize>,
    timeout_ms: u64,
    scrollback_lines: usize,
    scrollback_bytes: usize,
}

impl MemoryProfileOptions {
    fn parse(args: &[OsString]) -> Result<Self, String> {
        let mut output = None;
        let mut scenarios = DEFAULT_MEMORY_SCENARIOS.to_vec();
        let mut timeout_ms = DEFAULT_MEMORY_TIMEOUT_MS;
        let mut scrollback_lines = DEFAULT_SCROLLBACK_LINES;
        let mut scrollback_bytes = DEFAULT_SCROLLBACK_BYTES;
        let mut index = 0;

        while index < args.len() {
            match arg_str(args, index)? {
                "--output" => {
                    output = Some(PathBuf::from(value_arg(args, index, "--output")?));
                    index += 2;
                }
                "--lines" => {
                    scenarios = parse_scenarios(value_arg(args, index, "--lines")?)?;
                    index += 2;
                }
                "--timeout-ms" => {
                    timeout_ms = parse_u64(value_arg(args, index, "--timeout-ms")?, "timeout")?;
                    index += 2;
                }
                "--scrollback-lines" => {
                    scrollback_lines =
                        parse_usize(value_arg(args, index, "--scrollback-lines")?, "lines")?;
                    index += 2;
                }
                "--scrollback-bytes" => {
                    scrollback_bytes =
                        parse_usize(value_arg(args, index, "--scrollback-bytes")?, "bytes")?;
                    index += 2;
                }
                _ => return Err(m4_memory_usage()),
            }
        }

        if timeout_ms == 0 || scenarios.is_empty() {
            return Err(m4_memory_usage());
        }

        Ok(Self {
            output: output.ok_or_else(m4_memory_usage)?,
            scenarios,
            timeout_ms,
            scrollback_lines,
            scrollback_bytes,
        })
    }
}

#[derive(Debug, Clone)]
struct ReportOptions {
    benchmark: PathBuf,
    memory: PathBuf,
    thresholds: PathBuf,
    json_output: PathBuf,
    markdown_output: PathBuf,
}

impl ReportOptions {
    fn parse(args: &[OsString]) -> Result<Self, String> {
        let mut benchmark = None;
        let mut memory = None;
        let mut thresholds = None;
        let mut json_output = None;
        let mut markdown_output = None;
        let mut index = 0;

        while index < args.len() {
            match arg_str(args, index)? {
                "--bench" => {
                    benchmark = Some(PathBuf::from(value_arg(args, index, "--bench")?));
                    index += 2;
                }
                "--memory" => {
                    memory = Some(PathBuf::from(value_arg(args, index, "--memory")?));
                    index += 2;
                }
                "--thresholds" => {
                    thresholds = Some(PathBuf::from(value_arg(args, index, "--thresholds")?));
                    index += 2;
                }
                "--json-output" => {
                    json_output = Some(PathBuf::from(value_arg(args, index, "--json-output")?));
                    index += 2;
                }
                "--markdown-output" => {
                    markdown_output =
                        Some(PathBuf::from(value_arg(args, index, "--markdown-output")?));
                    index += 2;
                }
                _ => return Err(m4_report_usage()),
            }
        }

        Ok(Self {
            benchmark: benchmark.ok_or_else(m4_report_usage)?,
            memory: memory.ok_or_else(m4_report_usage)?,
            thresholds: thresholds.ok_or_else(m4_report_usage)?,
            json_output: json_output.ok_or_else(m4_report_usage)?,
            markdown_output: markdown_output.ok_or_else(m4_report_usage)?,
        })
    }
}

fn build_benchmark_evidence(options: &BenchmarkOptions, args: &[OsString]) -> M4BenchmarkEvidence {
    let mut measurements = Vec::new();
    let generated_at = utc_now();
    let input = m4_synthetic_workload(options.logical_lines);
    let machine = machine_metadata();
    let hera_commit = hera_commit();

    if options.iterations > 0 {
        measurements.push(measure_byte_ingest(options, &input));
        measurements.push(measure_snapshot_generation(options, &input));
        measurements.push(measure_replay(options, &input));
        measurements.push(measure_snapshot_comparison(options, &input));
    }

    let mut notes = vec![
        "Criterion.rs harness lives under crates/terminal-cli/benches/m4_benchmarks.rs.".to_owned(),
        "Bulky Criterion output remains under target/criterion and is not an M4 public artifact."
            .to_owned(),
    ];
    if measurements.is_empty() {
        notes.push("No benchmark iterations were requested; evidence is marked failed.".to_owned());
    }

    let status = if measurements.is_empty() {
        M4PerformanceStatus::Fail
    } else {
        m4_rollup_status(measurements.iter().map(|measurement| measurement.status))
    };

    M4BenchmarkEvidence {
        schema: M4_BENCHMARK_EVIDENCE_SCHEMA.to_owned(),
        version: M4_PERFORMANCE_VERSION,
        generated_at,
        hera_commit,
        command: command_line("m4-benchmark", args),
        machine,
        status,
        measurements,
        notes,
    }
}

fn measure_byte_ingest(options: &BenchmarkOptions, input: &[u8]) -> M4BenchmarkMeasurement {
    timed_measurement(
        "byte_ingest",
        M4BenchmarkOperation::ByteIngest,
        options,
        input,
        || {
            let mut terminal = Terminal::with_default_dimensions();
            terminal.advance_bytes(input);
            terminal.scrollback_len() <= options.logical_lines
        },
    )
}

fn measure_snapshot_generation(options: &BenchmarkOptions, input: &[u8]) -> M4BenchmarkMeasurement {
    let mut terminal = Terminal::with_default_dimensions();
    terminal.advance_bytes(input);
    timed_measurement(
        "snapshot_generation",
        M4BenchmarkOperation::SnapshotGeneration,
        options,
        input,
        || !snapshot_terminal(&mut terminal).viewport_rows().is_empty(),
    )
}

fn measure_replay(options: &BenchmarkOptions, input: &[u8]) -> M4BenchmarkMeasurement {
    let expected = snapshot_from_input(input);
    timed_measurement(
        "replay",
        M4BenchmarkOperation::Replay,
        options,
        input,
        || {
            let actual = snapshot_from_input(input);
            first_snapshot_difference(&expected, &actual).is_none()
        },
    )
}

fn measure_snapshot_comparison(options: &BenchmarkOptions, input: &[u8]) -> M4BenchmarkMeasurement {
    let left = snapshot_from_input(input);
    let right = left.clone();
    timed_measurement(
        "snapshot_comparison",
        M4BenchmarkOperation::SnapshotComparison,
        options,
        input,
        || first_snapshot_difference(&left, &right).is_none(),
    )
}

fn timed_measurement(
    id: &'static str,
    operation: M4BenchmarkOperation,
    options: &BenchmarkOptions,
    input: &[u8],
    mut measure: impl FnMut() -> bool,
) -> M4BenchmarkMeasurement {
    let started = Instant::now();
    let mut ok = true;
    for _ in 0..options.iterations {
        ok &= measure();
    }
    let total_nanos = started.elapsed().as_nanos().max(1);
    let nanos_per_iteration = total_nanos / u128::from(options.iterations).max(1);

    M4BenchmarkMeasurement {
        id: id.to_owned(),
        operation,
        input_name: format!(
            "synthetic_{lines}_logical_lines",
            lines = options.logical_lines
        ),
        input_bytes: input.len(),
        logical_lines: options.logical_lines,
        iterations: options.iterations,
        total_nanos,
        nanos_per_iteration: nanos_per_iteration.max(1),
        throughput_bytes_per_second: throughput(input.len(), options.iterations, total_nanos),
        status: if ok {
            M4PerformanceStatus::BaselineCreated
        } else {
            M4PerformanceStatus::Fail
        },
        notes: Vec::new(),
    }
}

fn build_memory_profile_evidence(
    options: &MemoryProfileOptions,
    args: &[OsString],
) -> Result<M4MemoryProfileEvidence, String> {
    let mut scenarios = Vec::new();
    for logical_lines in &options.scenarios {
        scenarios.push(run_memory_scenario(options, *logical_lines)?);
    }

    let status = m4_rollup_status(scenarios.iter().map(|scenario| scenario.status));
    Ok(M4MemoryProfileEvidence {
        schema: M4_MEMORY_PROFILE_SCHEMA.to_owned(),
        version: M4_PERFORMANCE_VERSION,
        generated_at: utc_now(),
        hera_commit: hera_commit(),
        command: command_line("m4-memory-profile", args),
        machine: machine_metadata(),
        status,
        scenarios,
        notes: vec![
            "Peak process RSS is not read through platform APIs in this portable command.".to_owned(),
            "The documented equivalent metric is Hera-owned scrollback bytes plus retained row counters."
                .to_owned(),
        ],
    })
}

fn run_memory_scenario(
    options: &MemoryProfileOptions,
    logical_lines: usize,
) -> Result<M4MemoryScenario, String> {
    let started = Instant::now();
    let timeout = Duration::from_millis(options.timeout_ms);
    let scrollback = ScrollbackConfig::new(options.scrollback_lines, options.scrollback_bytes);
    let config = TerminalConfig::with_scrollback(M4_COLUMNS, M4_ROWS, scrollback)
        .map_err(|error| error.to_string())?;
    let mut terminal = Terminal::with_config(config);
    let mut processed = 0usize;
    let mut blocked = false;

    for line in 0..logical_lines {
        advance_synthetic_line(&mut terminal, line);
        processed = processed.saturating_add(1);
        if processed % 2048 == 0 && started.elapsed() > timeout {
            blocked = true;
            break;
        }
    }

    let snapshot = snapshot_terminal(&mut terminal);
    let visible_rows = snapshot.viewport_rows().len();
    let scrollback_rows = terminal.scrollback_len();
    let hera_owned_bytes = terminal.scrollback_byte_len();
    let discarded_rows = processed.saturating_sub(scrollback_rows.saturating_add(visible_rows));
    let status = if blocked {
        M4PerformanceStatus::Blocked
    } else if hera_owned_bytes > options.scrollback_bytes {
        M4PerformanceStatus::Fail
    } else {
        M4PerformanceStatus::Pass
    };
    let mut notes = Vec::new();
    if blocked {
        notes.push(format!(
            "Scenario stopped after {processed} lines because timeout {timeout_ms}ms elapsed.",
            timeout_ms = options.timeout_ms
        ));
    }

    Ok(M4MemoryScenario {
        id: format!("memory_{logical_lines}_lines"),
        logical_lines_target: logical_lines,
        logical_lines_processed: processed,
        terminal_columns: M4_COLUMNS,
        terminal_rows: M4_ROWS,
        visible_rows,
        scrollback_rows,
        scrollback_line_budget: options.scrollback_lines,
        scrollback_byte_budget: options.scrollback_bytes,
        hera_owned_bytes,
        discarded_rows,
        peak_process_memory_bytes: None,
        process_memory_source: "hera_owned_scrollback_counter".to_owned(),
        elapsed_ms: started.elapsed().as_millis(),
        status,
        notes,
    })
}

fn build_performance_report(
    options: &ReportOptions,
    benchmark: &M4BenchmarkEvidence,
    memory: &M4MemoryProfileEvidence,
    thresholds: &M4PerformanceThresholds,
) -> M4PerformanceReport {
    let mut evaluations = benchmark
        .measurements
        .iter()
        .map(|measurement| evaluate_benchmark(measurement, thresholds))
        .collect::<Vec<_>>();
    if benchmark.measurements.is_empty() || benchmark.status.is_failure() {
        evaluations.push(source_status_evaluation(
            "benchmark_evidence",
            M4MetricSource::Benchmark,
            benchmark.status,
            "Benchmark evidence produced no measurements or reported failure.",
        ));
    }
    evaluations.extend(
        memory
            .scenarios
            .iter()
            .map(|scenario| evaluate_memory(scenario, thresholds)),
    );
    if memory.scenarios.is_empty() || memory.status.is_failure() {
        evaluations.push(source_status_evaluation(
            "memory_profile_evidence",
            M4MetricSource::MemoryProfile,
            memory.status,
            "Memory profile evidence produced no scenarios or reported failure.",
        ));
    }

    let status = m4_rollup_status(evaluations.iter().map(|evaluation| evaluation.status));
    M4PerformanceReport {
        schema: M4_PERFORMANCE_REPORT_SCHEMA.to_owned(),
        version: M4_PERFORMANCE_VERSION,
        generated_at: utc_now(),
        benchmark_input: slash_path(&options.benchmark),
        memory_input: slash_path(&options.memory),
        thresholds_input: slash_path(&options.thresholds),
        status,
        evaluations,
        notes: vec![
            "Latency metrics without prior accepted thresholds are marked baseline_created.".to_owned(),
            "Metrics marked unstable are excluded from hard pass claims until a stable platform baseline exists."
                .to_owned(),
        ],
    }
}

fn source_status_evaluation(
    metric_id: &str,
    source: M4MetricSource,
    status: M4PerformanceStatus,
    note: &str,
) -> M4MetricEvaluation {
    M4MetricEvaluation {
        metric_id: metric_id.to_owned(),
        source,
        observed_nanos_per_iteration: None,
        observed_hera_owned_bytes: None,
        status,
        threshold: None,
        notes: vec![note.to_owned()],
    }
}

fn validate_report_inputs(
    benchmark: &M4BenchmarkEvidence,
    memory: &M4MemoryProfileEvidence,
    thresholds: &M4PerformanceThresholds,
) -> Result<(), String> {
    validate_schema(
        "benchmark",
        &benchmark.schema,
        benchmark.version,
        M4_BENCHMARK_EVIDENCE_SCHEMA,
    )?;
    validate_schema(
        "memory profile",
        &memory.schema,
        memory.version,
        M4_MEMORY_PROFILE_SCHEMA,
    )?;
    validate_schema(
        "performance thresholds",
        &thresholds.schema,
        thresholds.version,
        M4_PERFORMANCE_THRESHOLDS_SCHEMA,
    )
}

fn validate_schema(
    label: &str,
    schema: &str,
    version: u32,
    expected_schema: &str,
) -> Result<(), String> {
    if schema != expected_schema {
        return Err(format!(
            "{label} schema mismatch: expected {expected_schema}, got {schema}"
        ));
    }
    if version != M4_PERFORMANCE_VERSION {
        return Err(format!(
            "{label} version mismatch: expected {M4_PERFORMANCE_VERSION}, got {version}"
        ));
    }
    Ok(())
}

fn evaluate_benchmark(
    measurement: &M4BenchmarkMeasurement,
    thresholds: &M4PerformanceThresholds,
) -> M4MetricEvaluation {
    let threshold = thresholds.threshold_for(&measurement.id).cloned();
    let (status, notes) = evaluate_nanos(
        measurement.status,
        measurement.nanos_per_iteration,
        threshold.as_ref(),
    );
    M4MetricEvaluation {
        metric_id: measurement.id.clone(),
        source: M4MetricSource::Benchmark,
        observed_nanos_per_iteration: Some(measurement.nanos_per_iteration),
        observed_hera_owned_bytes: None,
        status,
        threshold,
        notes,
    }
}

fn evaluate_memory(
    scenario: &M4MemoryScenario,
    thresholds: &M4PerformanceThresholds,
) -> M4MetricEvaluation {
    let threshold = thresholds.threshold_for(&scenario.id).cloned();
    let (status, notes) = evaluate_bytes(
        scenario.status,
        scenario.hera_owned_bytes,
        threshold.as_ref(),
    );
    M4MetricEvaluation {
        metric_id: scenario.id.clone(),
        source: M4MetricSource::MemoryProfile,
        observed_nanos_per_iteration: None,
        observed_hera_owned_bytes: Some(scenario.hera_owned_bytes),
        status,
        threshold,
        notes,
    }
}

fn evaluate_nanos(
    observed_status: M4PerformanceStatus,
    observed: u128,
    threshold: Option<&M4MetricThreshold>,
) -> (M4PerformanceStatus, Vec<String>) {
    if observed_status.is_failure() || observed_status.is_blocking_gap() {
        return (
            observed_status,
            vec!["Source evidence status carried through.".to_owned()],
        );
    }
    let Some(threshold) = threshold else {
        return (
            M4PerformanceStatus::BaselineCreated,
            vec!["No threshold record exists for this metric.".to_owned()],
        );
    };
    if threshold.unstable {
        return (
            M4PerformanceStatus::UnstableExcluded,
            threshold.notes.clone(),
        );
    }
    let Some(max) = threshold.max_nanos_per_iteration else {
        return (
            M4PerformanceStatus::BaselineCreated,
            vec!["No accepted latency baseline exists yet.".to_owned()],
        );
    };
    if observed > max {
        let status = if threshold.hard_gate {
            M4PerformanceStatus::Fail
        } else {
            M4PerformanceStatus::Partial
        };
        return (
            status,
            vec![format!("Observed {observed}ns exceeds {max}ns.")],
        );
    }

    (M4PerformanceStatus::Pass, Vec::new())
}

fn evaluate_bytes(
    observed_status: M4PerformanceStatus,
    observed: usize,
    threshold: Option<&M4MetricThreshold>,
) -> (M4PerformanceStatus, Vec<String>) {
    if observed_status.is_failure() || observed_status.is_blocking_gap() {
        return (
            observed_status,
            vec!["Source evidence status carried through.".to_owned()],
        );
    }
    let Some(threshold) = threshold else {
        return (
            M4PerformanceStatus::BaselineCreated,
            vec!["No threshold record exists for this metric.".to_owned()],
        );
    };
    let Some(max) = threshold.max_hera_owned_bytes else {
        return (
            M4PerformanceStatus::BaselineCreated,
            vec!["No accepted memory baseline exists yet.".to_owned()],
        );
    };
    if observed > max {
        let status = if threshold.hard_gate {
            M4PerformanceStatus::Fail
        } else {
            M4PerformanceStatus::Partial
        };
        return (
            status,
            vec![format!("Observed {observed} bytes exceeds {max} bytes.")],
        );
    }

    (M4PerformanceStatus::Pass, Vec::new())
}

fn render_performance_markdown(
    report: &M4PerformanceReport,
    benchmark: &M4BenchmarkEvidence,
    memory: &M4MemoryProfileEvidence,
) -> String {
    let mut output = String::new();
    output.push_str("# M4 Benchmarks And Memory\n\n");
    output.push_str(&format!("Generated: {}\n", report.generated_at));
    output.push_str(&format!("Status: `{}`\n\n", report.status.as_str()));
    output.push_str("## Benchmark Evidence\n\n");
    output.push_str(&format!(
        "Command: `{}`\n\n",
        benchmark.command.replace('|', "\\|")
    ));
    output.push_str("| Metric | Operation | Input bytes | Iterations | ns/iter | Throughput bytes/s | Status |\n");
    output.push_str("|---|---:|---:|---:|---:|---:|---|\n");
    for measurement in &benchmark.measurements {
        output.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} | `{}` |\n",
            measurement.id,
            measurement.operation.as_str(),
            measurement.input_bytes,
            measurement.iterations,
            measurement.nanos_per_iteration,
            optional_u64(measurement.throughput_bytes_per_second),
            measurement.status.as_str()
        ));
    }

    output.push_str("\n## Memory Profiles\n\n");
    output.push_str(&format!(
        "Command: `{}`\n\n",
        memory.command.replace('|', "\\|")
    ));
    output.push_str("| Scenario | Processed lines | Scrollback rows | Discarded rows | Hera-owned bytes | Status |\n");
    output.push_str("|---|---:|---:|---:|---:|---|\n");
    for scenario in &memory.scenarios {
        output.push_str(&format!(
            "| `{}` | {} / {} | {} | {} | {} | `{}` |\n",
            scenario.id,
            scenario.logical_lines_processed,
            scenario.logical_lines_target,
            scenario.scrollback_rows,
            scenario.discarded_rows,
            scenario.hera_owned_bytes,
            scenario.status.as_str()
        ));
    }

    output.push_str("\n## Threshold Evaluation\n\n");
    output.push_str("| Metric | Source | Observed | Status | Notes |\n");
    output.push_str("|---|---|---:|---|---|\n");
    for evaluation in &report.evaluations {
        output.push_str(&format!(
            "| `{}` | {} | {} | `{}` | {} |\n",
            evaluation.metric_id,
            evaluation.source.as_str(),
            observed_value(evaluation),
            evaluation.status.as_str(),
            evaluation.notes.join("; ").replace('|', "\\|")
        ));
    }

    output.push_str("\n## Public Artifact Policy\n\n");
    output.push_str(
        "Criterion target output is intentionally not listed as public evidence. The public package keeps the stable JSON summaries, this Markdown report and threshold config only.\n",
    );
    output
}

fn advance_synthetic_line(terminal: &mut Terminal, line: usize) {
    let mut bytes = Vec::with_capacity(24);
    bytes.extend_from_slice(b"m4-line-");
    bytes.extend_from_slice(line.to_string().as_bytes());
    bytes.extend_from_slice(b"\r\n");
    terminal.advance_bytes(&bytes);
}

fn snapshot_from_input(input: &[u8]) -> terminal_fixtures::TerminalSnapshot {
    let mut terminal = Terminal::with_default_dimensions();
    terminal.advance_bytes(input);
    snapshot_terminal(&mut terminal)
}

fn throughput(input_bytes: usize, iterations: u64, total_nanos: u128) -> Option<u64> {
    if total_nanos == 0 {
        return None;
    }
    let bytes = (input_bytes as u128).saturating_mul(u128::from(iterations));
    let throughput = bytes.saturating_mul(1_000_000_000) / total_nanos;
    Some(throughput.min(u128::from(u64::MAX)) as u64)
}

fn machine_metadata() -> M4MachineMetadata {
    M4MachineMetadata {
        os: std::env::consts::OS.to_owned(),
        arch: std::env::consts::ARCH.to_owned(),
        rustc: command_stdout("rustc", ["--version"]).unwrap_or_else(|| "unknown".to_owned()),
        cargo_profile: if cfg!(debug_assertions) {
            "debug".to_owned()
        } else {
            "release".to_owned()
        },
    }
}

fn hera_commit() -> String {
    command_stdout("git", ["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".to_owned())
}

fn command_stdout<const N: usize>(program: &str, args: [&str; N]) -> Option<String> {
    let output = ProcessCommand::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn command_line(command: &str, args: &[OsString]) -> String {
    let mut parts = vec!["terminal-cli".to_owned(), command.to_owned()];
    parts.extend(args.iter().map(|arg| arg.to_string_lossy().into_owned()));
    parts.join(" ")
}

fn utc_now() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    unix_seconds_to_utc(seconds)
}

fn unix_seconds_to_utc(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let seconds_of_day = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = year + i64::from(month <= 2);
    (year, month as u32, day as u32)
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    write_text(path, &(json + "\n"))
}

fn write_text(path: &Path, text: &str) -> Result<(), String> {
    ensure_parent(path)?;
    fs::write(path, text).map_err(|error| format!("{}: {error}", path.display()))
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let raw = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    serde_json::from_str(&raw).map_err(|error| format!("{}: {error}", path.display()))
}

fn ensure_parent(path: &Path) -> Result<(), String> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    if parent.as_os_str().is_empty() {
        return Ok(());
    }
    fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))
}

fn arg_str(args: &[OsString], index: usize) -> Result<&str, String> {
    args.get(index)
        .and_then(|arg| arg.to_str())
        .ok_or_else(|| "M4 command arguments must be valid UTF-8".to_owned())
}

fn value_arg(args: &[OsString], index: usize, flag: &str) -> Result<OsString, String> {
    args.get(index + 1)
        .cloned()
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn parse_u64(value: OsString, label: &str) -> Result<u64, String> {
    let raw = value
        .to_str()
        .ok_or_else(|| format!("{label} must be valid UTF-8"))?;
    raw.parse::<u64>()
        .map_err(|error| format!("invalid {label}: {error}"))
}

fn parse_usize(value: OsString, label: &str) -> Result<usize, String> {
    let raw = value
        .to_str()
        .ok_or_else(|| format!("{label} must be valid UTF-8"))?;
    raw.parse::<usize>()
        .map_err(|error| format!("invalid {label}: {error}"))
}

fn parse_scenarios(value: OsString) -> Result<Vec<usize>, String> {
    let raw = value
        .to_str()
        .ok_or_else(|| "scenario list must be valid UTF-8".to_owned())?;
    let mut scenarios = Vec::new();
    for entry in raw.split(',') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        scenarios.push(
            entry
                .parse::<usize>()
                .map_err(|error| format!("invalid scenario {entry}: {error}"))?,
        );
    }
    if scenarios.is_empty() {
        return Err("scenario list must not be empty".to_owned());
    }
    Ok(scenarios)
}

fn slash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn optional_u64(value: Option<u64>) -> String {
    value.map_or_else(|| "n/a".to_owned(), |value| value.to_string())
}

fn observed_value(evaluation: &M4MetricEvaluation) -> String {
    if let Some(value) = evaluation.observed_nanos_per_iteration {
        return format!("{value} ns/iter");
    }
    if let Some(value) = evaluation.observed_hera_owned_bytes {
        return format!("{value} bytes");
    }
    "n/a".to_owned()
}

fn m4_benchmark_usage() -> String {
    "usage: terminal-cli m4-benchmark --output <path> [--iterations <n>] [--logical-lines <n>]"
        .to_owned()
}

fn m4_memory_usage() -> String {
    "usage: terminal-cli m4-memory-profile --output <path> [--lines 10000,100000,1000000] [--timeout-ms <ms>] [--scrollback-lines <n>] [--scrollback-bytes <n>]"
        .to_owned()
}

fn m4_report_usage() -> String {
    "usage: terminal-cli m4-performance-report --bench <path> --memory <path> --thresholds <path> --json-output <path> --markdown-output <path>"
        .to_owned()
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs;
    use std::path::PathBuf;

    use terminal_fixtures::{M4PerformanceReport, M4PerformanceStatus};

    use super::{benchmark_command, performance_report_command};

    #[test]
    fn benchmark_zero_iterations_writes_failed_evidence() {
        let dir = temp_dir("hera-m4-benchmark-zero");
        let output = dir.join("benchmark.json");

        let outcome = benchmark_command(&[
            OsString::from("--output"),
            output.clone().into_os_string(),
            OsString::from("--iterations"),
            OsString::from("0"),
        ]);

        let raw = fs::read_to_string(&output).expect("benchmark evidence should be written");
        let evidence: terminal_fixtures::M4BenchmarkEvidence =
            serde_json::from_str(&raw).expect("benchmark evidence should parse");
        let _ = fs::remove_dir_all(&dir);

        assert_eq!(outcome.code, 1);
        assert_eq!(evidence.status, M4PerformanceStatus::Fail);
        assert!(evidence.measurements.is_empty());
    }

    #[test]
    fn performance_report_marks_missing_latency_baseline() {
        let dir = temp_dir("hera-m4-report-baseline");
        let bench = dir.join("bench.json");
        let memory = dir.join("memory.json");
        let thresholds = dir.join("thresholds.json");
        let json_output = dir.join("report.json");
        let markdown_output = dir.join("report.md");
        fs::write(
            &bench,
            r#"{
              "schema":"hera.m4_benchmark_evidence",
              "version":1,
              "generated_at":"2026-07-04T00:00:00Z",
              "hera_commit":"test",
              "command":"test",
              "machine":{"os":"windows","arch":"x86_64","rustc":"rustc test","cargo_profile":"debug"},
              "status":"baseline_created",
              "measurements":[{
                "id":"byte_ingest",
                "operation":"byte_ingest",
                "input_name":"test",
                "input_bytes":10,
                "logical_lines":1,
                "iterations":1,
                "total_nanos":10,
                "nanos_per_iteration":10,
                "throughput_bytes_per_second":100,
                "status":"baseline_created"
              }]
            }"#,
        )
        .expect("bench evidence should be writable");
        fs::write(
            &memory,
            r#"{
              "schema":"hera.m4_memory_profile",
              "version":1,
              "generated_at":"2026-07-04T00:00:00Z",
              "hera_commit":"test",
              "command":"test",
              "machine":{"os":"windows","arch":"x86_64","rustc":"rustc test","cargo_profile":"debug"},
              "status":"pass",
              "scenarios":[{
                "id":"memory_10_lines",
                "logical_lines_target":10,
                "logical_lines_processed":10,
                "terminal_columns":80,
                "terminal_rows":24,
                "visible_rows":24,
                "scrollback_rows":0,
                "scrollback_line_budget":100,
                "scrollback_byte_budget":1000,
                "hera_owned_bytes":0,
                "discarded_rows":0,
                "peak_process_memory_bytes":null,
                "process_memory_source":"hera_owned_scrollback_counter",
                "elapsed_ms":1,
                "status":"pass"
              }]
            }"#,
        )
        .expect("memory evidence should be writable");
        fs::write(
            &thresholds,
            r#"{
              "schema":"hera.m4_performance_thresholds",
              "version":1,
              "generated_at":"2026-07-04T00:00:00Z",
              "thresholds":[{
                "metric_id":"memory_10_lines",
                "hard_gate":true,
                "max_nanos_per_iteration":null,
                "max_hera_owned_bytes":1000
              }]
            }"#,
        )
        .expect("thresholds should be writable");

        let outcome = performance_report_command(&[
            OsString::from("--bench"),
            bench.into_os_string(),
            OsString::from("--memory"),
            memory.into_os_string(),
            OsString::from("--thresholds"),
            thresholds.into_os_string(),
            OsString::from("--json-output"),
            json_output.clone().into_os_string(),
            OsString::from("--markdown-output"),
            markdown_output.into_os_string(),
        ]);
        let raw = fs::read_to_string(&json_output).expect("report should be written");
        let report: M4PerformanceReport = serde_json::from_str(&raw).expect("report should parse");
        let _ = fs::remove_dir_all(&dir);

        assert_eq!(outcome.code, 0);
        assert_eq!(report.status, M4PerformanceStatus::BaselineCreated);
    }

    #[test]
    fn performance_report_preserves_failed_empty_benchmark_status() {
        let dir = temp_dir("hera-m4-report-failed-benchmark");
        let bench = dir.join("bench.json");
        let memory = dir.join("memory.json");
        let thresholds = dir.join("thresholds.json");
        let json_output = dir.join("report.json");
        let markdown_output = dir.join("report.md");
        fs::write(
            &bench,
            r#"{
              "schema":"hera.m4_benchmark_evidence",
              "version":1,
              "generated_at":"2026-07-04T00:00:00Z",
              "hera_commit":"test",
              "command":"test",
              "machine":{"os":"windows","arch":"x86_64","rustc":"rustc test","cargo_profile":"debug"},
              "status":"fail",
              "measurements":[]
            }"#,
        )
        .expect("bench evidence should be writable");
        fs::write(
            &memory,
            r#"{
              "schema":"hera.m4_memory_profile",
              "version":1,
              "generated_at":"2026-07-04T00:00:00Z",
              "hera_commit":"test",
              "command":"test",
              "machine":{"os":"windows","arch":"x86_64","rustc":"rustc test","cargo_profile":"debug"},
              "status":"pass",
              "scenarios":[{
                "id":"memory_10_lines",
                "logical_lines_target":10,
                "logical_lines_processed":10,
                "terminal_columns":80,
                "terminal_rows":24,
                "visible_rows":24,
                "scrollback_rows":0,
                "scrollback_line_budget":100,
                "scrollback_byte_budget":1000,
                "hera_owned_bytes":0,
                "discarded_rows":0,
                "peak_process_memory_bytes":null,
                "process_memory_source":"hera_owned_scrollback_counter",
                "elapsed_ms":1,
                "status":"pass"
              }]
            }"#,
        )
        .expect("memory evidence should be writable");
        fs::write(
            &thresholds,
            r#"{
              "schema":"hera.m4_performance_thresholds",
              "version":1,
              "generated_at":"2026-07-04T00:00:00Z",
              "thresholds":[{
                "metric_id":"memory_10_lines",
                "hard_gate":true,
                "max_nanos_per_iteration":null,
                "max_hera_owned_bytes":1000
              }]
            }"#,
        )
        .expect("thresholds should be writable");

        let outcome = performance_report_command(&[
            OsString::from("--bench"),
            bench.into_os_string(),
            OsString::from("--memory"),
            memory.into_os_string(),
            OsString::from("--thresholds"),
            thresholds.into_os_string(),
            OsString::from("--json-output"),
            json_output.clone().into_os_string(),
            OsString::from("--markdown-output"),
            markdown_output.into_os_string(),
        ]);
        let raw = fs::read_to_string(&json_output).expect("failed report should be written");
        let report: M4PerformanceReport =
            serde_json::from_str(&raw).expect("failed report should parse");
        let _ = fs::remove_dir_all(&dir);

        assert_eq!(outcome.code, 1);
        assert_eq!(report.status, M4PerformanceStatus::Fail);
        assert!(report.evaluations.iter().any(|evaluation| {
            evaluation.metric_id == "benchmark_evidence"
                && evaluation.status == M4PerformanceStatus::Fail
        }));
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp dir should be writable");
        dir
    }
}
