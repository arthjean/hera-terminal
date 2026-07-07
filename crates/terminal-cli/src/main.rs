//! Debug CLI boundary for Hera M1 and M2.

#![forbid(unsafe_code)]

mod m4_performance_cli;

use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, ExitCode};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use terminal_core::{Terminal, TerminalError};
use terminal_fixtures::{
    FixtureRunner, M1_MAX_FIXTURE_INPUT_BYTES, M1_MAX_SNAPSHOT_BYTES,
    M2_MAX_PTY_RECORDING_OUTPUT_BYTES, M4_PUBLIC_REPLAY_VERSION, M4_REPLAY_VERIFICATION_SCHEMA,
    M4_REPLAY_VERIFICATION_VERSION, M4CompatibilityMatrix, M4CompatibilityStatus,
    M4EvidenceManifest, M4PublicReplayFixture, M4ReplayVerificationStatus,
    M4ReplayVerificationSummary, M5_BASELINE_SCHEMA, M5_BASELINE_VERSION, M5Baseline,
    M5BaselineBlocker, M5BaselineDependency, M5BaselineDependencyStatus, M5BaselineDisposition,
    M5BaselineStatus, M5CompatibilityMatrix, M5CurrentEvidence, M5EvidenceManifest, M5GoNoGoPolicy,
    M5PaneflowDogfoodReport, M5PlatformCommandId, M5PlatformCommandResult, M5PlatformCommandStatus,
    M5PlatformEvidenceStatus, M5PlatformFreshnessPolicy, M5PlatformRow, M5PlatformRows,
    M5PlatformRuntimeEvidence, M5PublicReplayFixture, M5ReleasePlan, M5ReplayVerificationStatus,
    M5ReplayVerificationSummary, M5RuntimePlatform, M5SourceMilestone, PtyRecording,
    PtyRecordingCommandMetadata, PtyRecordingCommandMode, PtyRecordingEvent,
    PtyRecordingExitMetadata, PtyRecordingInputMetadata, PtyRecordingMetadata,
    PtyRecordingPlatformMetadata, PtyRecordingRuntimeMetadata, PtyRecordingSize,
    PtyRecordingStorageMetadata, TerminalSnapshot, deserialize_snapshot, first_snapshot_difference,
    m5_default_paneflow_dogfood_report, m5_default_replay_fixtures, m5_required_platform_commands,
    serialize_pty_recording_pretty, serialize_snapshot_pretty, snapshot_terminal,
};
use terminal_pty::{
    M2_DEFAULT_COMMAND_TIMEOUT_MS, M2_MAX_COMMAND_TIMEOUT_MS, M2_MAX_WRITE_CHUNK_BYTES,
    PortablePtyBackend, PtyBackend, PtyBridge, PtyBridgeError, PtyCommand, PtyEvent, PtyEventSink,
    PtyExit, PtyPlatformMetadata, PtyRunOutcome, PtyRuntimeConfig, PtySessionConfig,
    PtySessionRunner, PtySize,
};

const DEFAULT_COLUMNS: u16 = 80;
const DEFAULT_ROWS: u16 = 24;
const M2_CLI_MAX_INPUT_BYTES: usize = M2_MAX_WRITE_CHUNK_BYTES;
const M2_CLI_MAX_RECORDING_OUTPUT_BYTES: usize = M2_MAX_PTY_RECORDING_OUTPUT_BYTES;

fn main() -> ExitCode {
    let outcome = run(std::env::args_os().skip(1).collect());

    if !outcome.stdout.is_empty() {
        println!("{}", outcome.stdout);
    }
    if !outcome.stderr.is_empty() {
        eprintln!("{}", outcome.stderr);
    }

    ExitCode::from(outcome.code)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommandOutcome {
    code: u8,
    stdout: String,
    stderr: String,
}

impl CommandOutcome {
    pub(crate) fn success(stdout: impl Into<String>) -> Self {
        Self {
            code: 0,
            stdout: stdout.into(),
            stderr: String::new(),
        }
    }

    pub(crate) fn failure(code: u8, stderr: impl Into<String>) -> Self {
        Self {
            code,
            stdout: String::new(),
            stderr: stderr.into(),
        }
    }

    pub(crate) fn complete(code: u8, stdout: impl Into<String>, stderr: impl Into<String>) -> Self {
        Self {
            code,
            stdout: stdout.into(),
            stderr: stderr.into(),
        }
    }
}

fn run(args: Vec<OsString>) -> CommandOutcome {
    if args.is_empty() {
        return CommandOutcome::success(usage());
    }

    let Some(command) = args.first().and_then(|value| value.to_str()) else {
        return CommandOutcome::failure(2, usage());
    };

    match command {
        "inject" => inject_command(&args[1..]),
        "replay" => replay_command(&args[1..]),
        "compare" => compare_command(&args[1..]),
        "run" => run_command(&args[1..]),
        "validate-m4-evidence" => validate_m4_evidence_command(&args[1..]),
        "validate-m4-compatibility" => validate_m4_compatibility_command(&args[1..]),
        "generate-m5-baseline" => generate_m5_baseline_command(&args[1..]),
        "validate-m5-baseline" => validate_m5_baseline_command(&args[1..]),
        "validate-m5-evidence" => validate_m5_evidence_command(&args[1..]),
        "validate-m5-compatibility" => validate_m5_compatibility_command(&args[1..]),
        "validate-m5-go-no-go" => validate_m5_go_no_go_command(&args[1..]),
        "generate-m5-replay-derivatives" => generate_m5_replay_derivatives_command(&args[1..]),
        "verify-m5-replay" => verify_m5_replay_command(&args[1..]),
        "generate-m5-dogfood-report" => generate_m5_dogfood_report_command(&args[1..]),
        "validate-m5-dogfood" => validate_m5_dogfood_command(&args[1..]),
        "measure-m5-platform" => measure_m5_platform_command(&args[1..]),
        "validate-m5-platform" => validate_m5_platform_command(&args[1..]),
        "generate-m5-package-readiness" => generate_m5_package_readiness_command(&args[1..]),
        "validate-m5-package-readiness" => validate_m5_package_readiness_command(&args[1..]),
        "validate-m5-release-plan" => validate_m5_release_plan_command(&args[1..]),
        "generate-m5-api-audit" => generate_m5_api_audit_command(&args[1..]),
        "validate-m5-api-audit" => validate_m5_api_audit_command(&args[1..]),
        "generate-m5-security-baseline" => generate_m5_security_baseline_command(&args[1..]),
        "validate-m5-security-baseline" => validate_m5_security_baseline_command(&args[1..]),
        "verify-m4-replay" => verify_m4_replay_command(&args[1..]),
        "export-m4-event-stream" => export_m4_event_stream_command(&args[1..]),
        "m4-benchmark" => m4_performance_cli::benchmark_command(&args[1..]),
        "m4-memory-profile" => m4_performance_cli::memory_profile_command(&args[1..]),
        "m4-performance-report" => m4_performance_cli::performance_report_command(&args[1..]),
        _ => CommandOutcome::failure(2, usage()),
    }
}

fn inject_command(args: &[OsString]) -> CommandOutcome {
    let Some(path) = one_path_arg(args) else {
        return CommandOutcome::failure(2, "usage: terminal-cli inject <file>");
    };

    let bytes = match read_bytes_capped(&path, M1_MAX_FIXTURE_INPUT_BYTES) {
        Ok(bytes) => bytes,
        Err(message) => return CommandOutcome::failure(1, message),
    };

    let mut terminal = Terminal::with_default_dimensions();
    terminal.advance_bytes(&bytes);
    let snapshot = snapshot_terminal(&mut terminal);

    match serialize_snapshot_pretty(&snapshot) {
        Ok(snapshot) => CommandOutcome::success(snapshot),
        Err(error) => CommandOutcome::failure(1, error.to_string()),
    }
}

fn replay_command(args: &[OsString]) -> CommandOutcome {
    let Some(path) = one_path_arg(args) else {
        return CommandOutcome::failure(2, "usage: terminal-cli replay <fixture>");
    };

    match FixtureRunner::run_pack_path(&path) {
        Ok(reports) => {
            let mut lines = reports
                .iter()
                .map(|report| format!("fixture {}: pass", report.name()))
                .collect::<Vec<_>>();
            lines.push(format!("{} fixtures passed", reports.len()));
            CommandOutcome::success(lines.join("\n"))
        }
        Err(error) => CommandOutcome::failure(1, error.to_string()),
    }
}

fn compare_command(args: &[OsString]) -> CommandOutcome {
    if args.len() != 2 {
        return CommandOutcome::failure(2, "usage: terminal-cli compare <a> <b>");
    }

    let left = match read_snapshot(&PathBuf::from(&args[0])) {
        Ok(snapshot) => snapshot,
        Err(message) => return CommandOutcome::failure(1, message),
    };
    let right = match read_snapshot(&PathBuf::from(&args[1])) {
        Ok(snapshot) => snapshot,
        Err(message) => return CommandOutcome::failure(1, message),
    };

    match first_snapshot_difference(&left, &right) {
        Some(difference) => CommandOutcome::failure(1, difference.to_string()),
        None => CommandOutcome::success("snapshots match"),
    }
}

fn run_command(args: &[OsString]) -> CommandOutcome {
    let options = match RunOptions::parse(args) {
        Ok(options) => options,
        Err(RunParseError::Usage(message)) => return CommandOutcome::failure(2, message),
        Err(RunParseError::Message(message)) => return CommandOutcome::failure(1, message),
    };

    let artifact = match execute_run(&options) {
        Ok(artifact) => artifact,
        Err(error) => return CommandOutcome::failure(error.exit_code(), error.to_string()),
    };

    let output = match emit_run_output(&options, &artifact.run) {
        Ok(output) => output,
        Err(error) => return CommandOutcome::failure(1, error),
    };

    let mut code = child_exit_code(artifact.run.metadata.exit.code);
    let mut stderr = String::new();

    if let Some(record_path) = options.record_path.as_deref() {
        if let Err(error) = write_recording(record_path, &artifact.recording) {
            code = 1;
            stderr = error;
        }
    }

    CommandOutcome::complete(code, output, stderr)
}

fn validate_m4_evidence_command(args: &[OsString]) -> CommandOutcome {
    let Some(path) = one_path_arg(args) else {
        return CommandOutcome::failure(2, "usage: terminal-cli validate-m4-evidence <manifest>");
    };

    let manifest = match M4EvidenceManifest::from_path(&path) {
        Ok(manifest) => manifest,
        Err(error) => return CommandOutcome::failure(1, error.to_string()),
    };
    let repo_root = m4_manifest_repo_root(&path);
    if let Err(error) = manifest.validate_public_artifact_files(&repo_root) {
        return CommandOutcome::failure(1, error.to_string());
    }

    CommandOutcome::success(format!(
        "M4 evidence manifest valid: {} artifacts",
        manifest.artifacts().len()
    ))
}

fn validate_m4_compatibility_command(args: &[OsString]) -> CommandOutcome {
    let Some(path) = one_path_arg(args) else {
        return CommandOutcome::failure(
            2,
            "usage: terminal-cli validate-m4-compatibility <matrix>",
        );
    };

    let matrix = match M4CompatibilityMatrix::from_path(&path) {
        Ok(matrix) => matrix,
        Err(error) => return CommandOutcome::failure(1, error.to_string()),
    };
    let repo_root = m4_manifest_repo_root(&path);
    if let Err(error) = matrix.validate_referenced_artifacts(&path, &repo_root) {
        return CommandOutcome::failure(1, error.to_string());
    }

    CommandOutcome::success(format!(
        "M4 compatibility matrix valid: {} rows",
        matrix.rows().len()
    ))
}

fn generate_m5_baseline_command(args: &[OsString]) -> CommandOutcome {
    let options = match GenerateM5BaselineOptions::parse(args) {
        Ok(options) => options,
        Err(message) => return CommandOutcome::failure(2, message),
    };

    let baseline = build_m5_baseline(&options);
    if let Err(error) = write_json(&options.output, &baseline) {
        return CommandOutcome::failure(1, error);
    }
    if let Err(error) = M5Baseline::from_path(&options.output) {
        return CommandOutcome::failure(1, error.to_string());
    }

    CommandOutcome::success(format!(
        "M5 baseline generated: {} blockers -> {}",
        baseline.blockers.len(),
        options.output.display()
    ))
}

fn validate_m5_baseline_command(args: &[OsString]) -> CommandOutcome {
    let Some(path) = one_path_arg(args) else {
        return CommandOutcome::failure(2, "usage: terminal-cli validate-m5-baseline <baseline>");
    };

    let baseline = match M5Baseline::from_path(&path) {
        Ok(baseline) => baseline,
        Err(error) => return CommandOutcome::failure(1, error.to_string()),
    };

    CommandOutcome::success(format!(
        "M5 baseline valid: {} blockers, {} dependencies",
        baseline.blockers().len(),
        baseline.dependencies().len()
    ))
}

fn validate_m5_evidence_command(args: &[OsString]) -> CommandOutcome {
    let Some(path) = one_path_arg(args) else {
        return CommandOutcome::failure(2, "usage: terminal-cli validate-m5-evidence <manifest>");
    };

    let manifest = match M5EvidenceManifest::from_path(&path) {
        Ok(manifest) => manifest,
        Err(error) => return CommandOutcome::failure(1, error.to_string()),
    };
    let repo_root = evidence_manifest_repo_root(&path);
    if let Err(error) = manifest.validate_public_artifact_files(&repo_root) {
        return CommandOutcome::failure(1, error.to_string());
    }

    CommandOutcome::success(format!(
        "M5 evidence manifest valid: {} artifacts",
        manifest.artifacts().len()
    ))
}

fn validate_m5_compatibility_command(args: &[OsString]) -> CommandOutcome {
    let Some(path) = one_path_arg(args) else {
        return CommandOutcome::failure(
            2,
            "usage: terminal-cli validate-m5-compatibility <matrix>",
        );
    };

    let matrix = match M5CompatibilityMatrix::from_path(&path) {
        Ok(matrix) => matrix,
        Err(error) => return CommandOutcome::failure(1, error.to_string()),
    };
    let repo_root = evidence_manifest_repo_root(&path);
    if let Err(error) = matrix.validate_referenced_artifacts(&path, &repo_root) {
        return CommandOutcome::failure(1, error.to_string());
    }

    CommandOutcome::success(format!(
        "M5 compatibility matrix valid: {} rows, {} pass, {} measured",
        matrix.rows().len(),
        matrix.pass_count(),
        matrix.measured_count()
    ))
}

fn validate_m5_go_no_go_command(args: &[OsString]) -> CommandOutcome {
    let Some(path) = one_path_arg(args) else {
        return CommandOutcome::failure(2, "usage: terminal-cli validate-m5-go-no-go <policy>");
    };

    let policy = match M5GoNoGoPolicy::from_path(&path) {
        Ok(policy) => policy,
        Err(error) => return CommandOutcome::failure(1, error.to_string()),
    };

    CommandOutcome::success(format!(
        "M5 go/no-go policy valid: {} outcomes",
        policy.outcomes().len()
    ))
}

fn generate_m5_replay_derivatives_command(args: &[OsString]) -> CommandOutcome {
    let options = match GenerateM5ReplayDerivativesOptions::parse(args) {
        Ok(options) => options,
        Err(message) => return CommandOutcome::failure(2, message),
    };

    let generated_at = utc_now();
    let fixtures = m5_default_replay_fixtures(&generated_at);
    for fixture in &fixtures {
        let path = options.output_dir.join(fixture.filename());
        let serialized = match serialize_json(fixture) {
            Ok(json) => json,
            Err(error) => return CommandOutcome::failure(1, error),
        };
        if let Err(error) = M5PublicReplayFixture::from_json_str(&path, &serialized) {
            return CommandOutcome::failure(1, error.to_string());
        }
        if let Err(error) = write_text(&path, &(serialized + "\n")) {
            return CommandOutcome::failure(1, error);
        }
    }

    CommandOutcome::success(format!(
        "M5 replay derivatives generated: {} fixtures -> {}",
        fixtures.len(),
        options.output_dir.display()
    ))
}

fn verify_m5_replay_command(args: &[OsString]) -> CommandOutcome {
    let options = match VerifyM5ReplayOptions::parse(args) {
        Ok(options) => options,
        Err(message) => return CommandOutcome::failure(2, message),
    };
    let paths = match collect_replay_json_paths(&options.path, "M5") {
        Ok(paths) => paths,
        Err(message) => return CommandOutcome::failure(1, message),
    };

    let mut reports = Vec::with_capacity(paths.len());
    for path in &paths {
        let fixture = match M5PublicReplayFixture::from_path(path) {
            Ok(fixture) => fixture,
            Err(error) => return CommandOutcome::failure(1, error.to_string()),
        };
        let report = match fixture.verify() {
            Ok(report) => report,
            Err(error) => return CommandOutcome::failure(1, error.to_string()),
        };
        reports.push(report);
    }

    let summary = M5ReplayVerificationSummary::from_reports(
        utc_now(),
        cli_command_line("verify-m5-replay", args),
        reports,
    );
    if let Some(output) = options.json_output.as_deref() {
        if let Err(error) = write_json(output, &summary) {
            return CommandOutcome::failure(1, error);
        }
    }
    if summary.status != M5ReplayVerificationStatus::Pass {
        return CommandOutcome::failure(
            1,
            format!(
                "M5 replay verification failed: {} fixtures",
                summary.fixtures.len()
            ),
        );
    }

    CommandOutcome::success(format!(
        "M5 replay verification passed: {} fixtures",
        summary.fixtures.len()
    ))
}

fn generate_m5_dogfood_report_command(args: &[OsString]) -> CommandOutcome {
    let options = match GenerateM5DogfoodReportOptions::parse(args) {
        Ok(options) => options,
        Err(message) => return CommandOutcome::failure(2, message),
    };
    let generated_at = utc_now();
    let report = m5_default_paneflow_dogfood_report(&generated_at);
    if let Err(error) = report.validate(&options.json_output) {
        return CommandOutcome::failure(1, error.to_string());
    }
    if let Err(error) = write_json(&options.json_output, &report) {
        return CommandOutcome::failure(1, error);
    }

    CommandOutcome::success(format!(
        "M5 Paneflow dogfood report generated: {}",
        options.json_output.display()
    ))
}

fn validate_m5_dogfood_command(args: &[OsString]) -> CommandOutcome {
    let Some(path) = one_path_arg(args) else {
        return CommandOutcome::failure(2, "usage: terminal-cli validate-m5-dogfood <report>");
    };

    let report = match M5PaneflowDogfoodReport::from_path(&path) {
        Ok(report) => report,
        Err(error) => return CommandOutcome::failure(1, error.to_string()),
    };

    CommandOutcome::success(format!(
        "M5 Paneflow dogfood report valid: {:?}, P0 mismatches {}",
        report.status, report.mismatch_summary.p0
    ))
}

fn measure_m5_platform_command(args: &[OsString]) -> CommandOutcome {
    let options = match MeasureM5PlatformOptions::parse(args) {
        Ok(options) => options,
        Err(message) => return CommandOutcome::failure(2, message),
    };
    let generated_at = utc_now();
    let source_command = cli_command_line("measure-m5-platform", args);
    let output_path = repo_path_string(&options.json_output);

    let bootstrap = build_m5_platform_evidence(
        &generated_at,
        &source_command,
        None,
        &output_path,
        "Bootstrap evidence is written before running cargo subcommands so workspace tests can validate this artifact during refresh.",
    );
    if let Err(error) = bootstrap.validate(&options.json_output) {
        return CommandOutcome::failure(1, error.to_string());
    }
    if let Err(error) = write_json(&options.json_output, &bootstrap) {
        return CommandOutcome::failure(1, error);
    }

    let Some(platform) = current_runtime_platform() else {
        let evidence = build_m5_platform_evidence(
            &generated_at,
            &source_command,
            None,
            &output_path,
            "Current OS is outside the M5 Windows/Linux/macOS platform set.",
        );
        if let Err(error) = write_json(&options.json_output, &evidence) {
            return CommandOutcome::failure(1, error);
        }
        return CommandOutcome::failure(1, "current platform is not supported by M5 evidence");
    };

    let row = measure_current_platform_row(platform, &generated_at, &output_path);
    let evidence = build_m5_platform_evidence(
        &generated_at,
        &source_command,
        Some(row),
        &output_path,
        "Linux and macOS rows remain blocked until this same runner is executed on those platforms or in CI.",
    );
    if let Err(error) = evidence.validate(&options.json_output) {
        return CommandOutcome::failure(1, error.to_string());
    }
    if let Err(error) = write_json(&options.json_output, &evidence) {
        return CommandOutcome::failure(1, error);
    }

    let current = evidence
        .platform_rows()
        .into_iter()
        .find(|row| row.platform == platform)
        .expect("current platform row must exist");
    let failed = current
        .commands
        .iter()
        .filter(|command| command.status == M5PlatformCommandStatus::Failed)
        .count();
    let blocked = current
        .commands
        .iter()
        .filter(|command| command.status == M5PlatformCommandStatus::Blocked)
        .count();

    if failed > 0 {
        return CommandOutcome::failure(
            1,
            format!(
                "M5 platform evidence generated with failed commands: {failed} failed -> {}",
                options.json_output.display()
            ),
        );
    }
    if blocked > 0 {
        return CommandOutcome::failure(
            1,
            format!(
                "M5 platform evidence generated with blocked commands: {blocked} blocked -> {}",
                options.json_output.display()
            ),
        );
    }

    CommandOutcome::success(format!(
        "M5 platform evidence generated: {:?} pass -> {}",
        platform,
        options.json_output.display()
    ))
}

fn validate_m5_platform_command(args: &[OsString]) -> CommandOutcome {
    let Some(path) = one_path_arg(args) else {
        return CommandOutcome::failure(2, "usage: terminal-cli validate-m5-platform <evidence>");
    };

    let evidence = match M5PlatformRuntimeEvidence::from_path(&path) {
        Ok(evidence) => evidence,
        Err(error) => return CommandOutcome::failure(1, error.to_string()),
    };
    let pass = evidence
        .platform_rows()
        .into_iter()
        .filter(|row| row.status == M5PlatformEvidenceStatus::Pass)
        .count();
    let blocked = evidence
        .platform_rows()
        .into_iter()
        .filter(|row| row.status == M5PlatformEvidenceStatus::Blocked)
        .count();

    CommandOutcome::success(format!(
        "M5 platform runtime evidence valid: 3 platforms, {} required commands, {pass} pass, {blocked} blocked",
        evidence.required_commands.len()
    ))
}

fn generate_m5_package_readiness_command(args: &[OsString]) -> CommandOutcome {
    let options = match GenerateM5PackageReadinessOptions::parse(args) {
        Ok(options) => options,
        Err(message) => return CommandOutcome::failure(2, message),
    };
    let generated_at = utc_now();
    let source_command = cli_command_line("generate-m5-package-readiness", args);
    let release_plan = build_m5_release_plan(&generated_at, &source_command);
    if let Err(error) = release_plan.validate(&options.release_plan_output) {
        return CommandOutcome::failure(1, error.to_string());
    }
    if let Err(error) = write_json(&options.release_plan_output, &release_plan) {
        return CommandOutcome::failure(1, error);
    }

    let mut crates = Vec::new();
    for spec in m5_public_crate_specs() {
        match build_m5_package_crate(spec) {
            Ok(krate) => crates.push(krate),
            Err(error) => return CommandOutcome::failure(1, error),
        }
    }
    let status = if crates
        .iter()
        .all(|krate| krate.package_status == terminal_fixtures::M5ReadinessStatus::Pass)
    {
        terminal_fixtures::M5ReadinessStatus::Pass
    } else {
        terminal_fixtures::M5ReadinessStatus::Blocked
    };
    let readiness = terminal_fixtures::M5PackageReadiness {
        schema: terminal_fixtures::M5_PACKAGE_READINESS_SCHEMA.to_owned(),
        version: terminal_fixtures::M5_RELEASE_EVIDENCE_VERSION,
        generated_at,
        source_command,
        status,
        release_plan_path: repo_path_string(&options.release_plan_output),
        crates,
        publish_actions: vec![terminal_fixtures::M5PublishAction {
            command: "cargo publish".to_owned(),
            status: terminal_fixtures::M5ActionStatus::OutOfScope,
            summary: "M5 performs readiness checks only. No cargo publish command was run."
                .to_owned(),
        }],
        notes: vec![
            "All six Hera crates are intended public pre-release surfaces for this readiness pass."
                .to_owned(),
            "Package archives are dry-run artifacts under target/package and are not release uploads."
                .to_owned(),
        ],
    };
    if let Err(error) = readiness.validate(&options.output) {
        return CommandOutcome::failure(1, error.to_string());
    }
    if let Err(error) = write_json(&options.output, &readiness) {
        return CommandOutcome::failure(1, error);
    }
    if let Err(error) = terminal_fixtures::M5PackageReadiness::from_path(&options.output) {
        return CommandOutcome::failure(1, error.to_string());
    }

    CommandOutcome::success(format!(
        "M5 package readiness generated: {} crates -> {}",
        readiness.crates.len(),
        options.output.display()
    ))
}

fn validate_m5_package_readiness_command(args: &[OsString]) -> CommandOutcome {
    let Some(path) = one_path_arg(args) else {
        return CommandOutcome::failure(
            2,
            "usage: terminal-cli validate-m5-package-readiness <readiness>",
        );
    };

    let readiness = match terminal_fixtures::M5PackageReadiness::from_path(&path) {
        Ok(readiness) => readiness,
        Err(error) => return CommandOutcome::failure(1, error.to_string()),
    };
    let public_count = readiness.public_crates().count();

    CommandOutcome::success(format!(
        "M5 package readiness valid: {public_count} public crates, status {:?}",
        readiness.status
    ))
}

fn validate_m5_release_plan_command(args: &[OsString]) -> CommandOutcome {
    let Some(path) = one_path_arg(args) else {
        return CommandOutcome::failure(2, "usage: terminal-cli validate-m5-release-plan <plan>");
    };

    let plan = match terminal_fixtures::M5ReleasePlan::from_path(&path) {
        Ok(plan) => plan,
        Err(error) => return CommandOutcome::failure(1, error.to_string()),
    };

    CommandOutcome::success(format!(
        "M5 release plan valid: {} public crates, publish out of scope {}",
        plan.publish_order.len(),
        plan.publish_out_of_scope
    ))
}

fn generate_m5_api_audit_command(args: &[OsString]) -> CommandOutcome {
    let options = match GenerateM5ApiAuditOptions::parse(args) {
        Ok(options) => options,
        Err(message) => return CommandOutcome::failure(2, message),
    };
    let generated_at = utc_now();
    let source_command = cli_command_line("generate-m5-api-audit", args);
    let audit = build_m5_api_audit(generated_at, source_command);
    if let Err(error) = audit.validate(&options.output) {
        return CommandOutcome::failure(1, error.to_string());
    }
    if let Err(error) = write_json(&options.output, &audit) {
        return CommandOutcome::failure(1, error);
    }
    if let Err(error) = terminal_fixtures::M5ApiAudit::from_path(&options.output) {
        return CommandOutcome::failure(1, error.to_string());
    }

    CommandOutcome::success(format!(
        "M5 API audit generated: {}",
        options.output.display()
    ))
}

fn validate_m5_api_audit_command(args: &[OsString]) -> CommandOutcome {
    let Some(path) = one_path_arg(args) else {
        return CommandOutcome::failure(2, "usage: terminal-cli validate-m5-api-audit <audit>");
    };

    let audit = match terminal_fixtures::M5ApiAudit::from_path(&path) {
        Ok(audit) => audit,
        Err(error) => return CommandOutcome::failure(1, error.to_string()),
    };

    CommandOutcome::success(format!(
        "M5 API audit valid: {} crates, semver {:?}",
        audit.public_crates.len(),
        audit.semver_baseline.status
    ))
}

fn generate_m5_security_baseline_command(args: &[OsString]) -> CommandOutcome {
    let options = match GenerateM5SecurityBaselineOptions::parse(args) {
        Ok(options) => options,
        Err(message) => return CommandOutcome::failure(2, message),
    };
    let generated_at = utc_now();
    let source_command = cli_command_line("generate-m5-security-baseline", args);
    let baseline = build_m5_security_baseline(generated_at, source_command);
    if let Err(error) = baseline.validate(&options.output) {
        return CommandOutcome::failure(1, error.to_string());
    }
    if let Err(error) = write_json(&options.output, &baseline) {
        return CommandOutcome::failure(1, error);
    }
    if let Err(error) = terminal_fixtures::M5SecurityBaseline::from_path(&options.output) {
        return CommandOutcome::failure(1, error.to_string());
    }

    CommandOutcome::success(format!(
        "M5 security baseline generated: {} tools, status {:?} -> {}",
        baseline.tools.len(),
        baseline.status,
        options.output.display()
    ))
}

fn validate_m5_security_baseline_command(args: &[OsString]) -> CommandOutcome {
    let Some(path) = one_path_arg(args) else {
        return CommandOutcome::failure(
            2,
            "usage: terminal-cli validate-m5-security-baseline <baseline>",
        );
    };

    let baseline = match terminal_fixtures::M5SecurityBaseline::from_path(&path) {
        Ok(baseline) => baseline,
        Err(error) => return CommandOutcome::failure(1, error.to_string()),
    };

    CommandOutcome::success(format!(
        "M5 security baseline valid: {} tools, {} blocked, {} failed, {} release blockers",
        baseline.tools.len(),
        baseline.summary.blocked_tools,
        baseline.summary.failed_tools,
        baseline.summary.release_blocking_findings
    ))
}

fn verify_m4_replay_command(args: &[OsString]) -> CommandOutcome {
    let options = match VerifyM4ReplayOptions::parse(args) {
        Ok(options) => options,
        Err(message) => return CommandOutcome::failure(2, message),
    };
    let paths = match collect_m4_replay_paths(&options.path) {
        Ok(paths) => paths,
        Err(message) => return CommandOutcome::failure(1, message),
    };

    let mut reports = Vec::with_capacity(paths.len());
    for path in &paths {
        let fixture = match M4PublicReplayFixture::from_path(path) {
            Ok(fixture) => fixture,
            Err(error) => return CommandOutcome::failure(1, error.to_string()),
        };
        let report = match fixture.verify() {
            Ok(report) => report,
            Err(error) => return CommandOutcome::failure(1, error.to_string()),
        };
        reports.push(report);
    }

    let summary = M4ReplayVerificationSummary {
        schema: M4_REPLAY_VERIFICATION_SCHEMA.to_owned(),
        version: M4_REPLAY_VERIFICATION_VERSION,
        generated_at: utc_now(),
        command: cli_command_line("verify-m4-replay", args),
        status: M4ReplayVerificationStatus::Pass,
        fixtures: reports,
    };
    if let Some(output) = options.json_output.as_deref() {
        if let Err(error) = write_json(output, &summary) {
            return CommandOutcome::failure(1, error);
        }
    }

    CommandOutcome::success(format!(
        "M4 replay verification passed: {} fixtures",
        summary.fixtures.len()
    ))
}

fn export_m4_event_stream_command(args: &[OsString]) -> CommandOutcome {
    let options = match ExportM4EventStreamOptions::parse(args) {
        Ok(options) => options,
        Err(message) => return CommandOutcome::failure(2, message),
    };
    let fixture = match M4PublicReplayFixture::from_path(&options.fixture) {
        Ok(fixture) => fixture,
        Err(error) => return CommandOutcome::failure(1, error.to_string()),
    };
    let stream = match fixture.to_public_event_stream() {
        Ok(stream) => stream,
        Err(error) => return CommandOutcome::failure(1, error.to_string()),
    };
    if let Err(error) = write_text(&options.output, &stream) {
        return CommandOutcome::failure(1, error);
    }

    CommandOutcome::success(format!(
        "wrote M4 public event stream v{}: {}",
        M4_PUBLIC_REPLAY_VERSION,
        options.output.display()
    ))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GenerateM5ReplayDerivativesOptions {
    output_dir: PathBuf,
}

impl GenerateM5ReplayDerivativesOptions {
    fn parse(args: &[OsString]) -> Result<Self, String> {
        let mut output_dir = PathBuf::from("crates/terminal-fixtures/fixtures/m5-replay");
        let mut index = 0usize;
        while index < args.len() {
            let Some(value) = args[index].to_str() else {
                return Err("M5 replay derivative arguments must be valid UTF-8".to_owned());
            };
            match value {
                "--output-dir" => {
                    output_dir = value_arg_path(args, index, "--output-dir")?;
                    index += 2;
                }
                _ => return Err(m5_replay_generate_usage()),
            }
        }

        Ok(Self { output_dir })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct VerifyM5ReplayOptions {
    path: PathBuf,
    json_output: Option<PathBuf>,
}

impl VerifyM5ReplayOptions {
    fn parse(args: &[OsString]) -> Result<Self, String> {
        if args.is_empty() {
            return Err(m5_replay_verify_usage());
        }

        let mut path = None;
        let mut json_output = None;
        let mut index = 0usize;
        while index < args.len() {
            let Some(value) = args[index].to_str() else {
                return Err("M5 replay arguments must be valid UTF-8".to_owned());
            };
            match value {
                "--json-output" => {
                    json_output = Some(value_arg_path(args, index, "--json-output")?);
                    index += 2;
                }
                flag if flag.starts_with("--") => return Err(m5_replay_verify_usage()),
                _ => {
                    if path.is_some() {
                        return Err(m5_replay_verify_usage());
                    }
                    path = Some(PathBuf::from(&args[index]));
                    index += 1;
                }
            }
        }

        Ok(Self {
            path: path.ok_or_else(m5_replay_verify_usage)?,
            json_output,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GenerateM5DogfoodReportOptions {
    json_output: PathBuf,
}

impl GenerateM5DogfoodReportOptions {
    fn parse(args: &[OsString]) -> Result<Self, String> {
        let mut json_output = None;
        let mut index = 0usize;
        while index < args.len() {
            let Some(value) = args[index].to_str() else {
                return Err("M5 dogfood arguments must be valid UTF-8".to_owned());
            };
            match value {
                "--json-output" => {
                    json_output = Some(value_arg_path(args, index, "--json-output")?);
                    index += 2;
                }
                _ => return Err(m5_dogfood_generate_usage()),
            }
        }

        Ok(Self {
            json_output: json_output.ok_or_else(m5_dogfood_generate_usage)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MeasureM5PlatformOptions {
    json_output: PathBuf,
}

impl MeasureM5PlatformOptions {
    fn parse(args: &[OsString]) -> Result<Self, String> {
        let mut json_output = None;
        let mut index = 0usize;
        while index < args.len() {
            let Some(value) = args[index].to_str() else {
                return Err("M5 platform arguments must be valid UTF-8".to_owned());
            };
            match value {
                "--json-output" => {
                    json_output = Some(value_arg_path(args, index, "--json-output")?);
                    index += 2;
                }
                _ => return Err(m5_platform_measure_usage()),
            }
        }

        Ok(Self {
            json_output: json_output.ok_or_else(m5_platform_measure_usage)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GenerateM5PackageReadinessOptions {
    output: PathBuf,
    release_plan_output: PathBuf,
}

impl GenerateM5PackageReadinessOptions {
    fn parse(args: &[OsString]) -> Result<Self, String> {
        let mut output = PathBuf::from("evidence/m5/m5-package-readiness.json");
        let mut release_plan_output = PathBuf::from("evidence/m5/m5-release-plan.json");
        let mut index = 0usize;
        while index < args.len() {
            let Some(value) = args[index].to_str() else {
                return Err("M5 package readiness arguments must be valid UTF-8".to_owned());
            };
            match value {
                "--output" => {
                    output = value_arg_path(args, index, "--output")?;
                    index += 2;
                }
                "--release-plan-output" => {
                    release_plan_output = value_arg_path(args, index, "--release-plan-output")?;
                    index += 2;
                }
                _ => return Err(m5_package_readiness_usage()),
            }
        }
        Ok(Self {
            output,
            release_plan_output,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GenerateM5ApiAuditOptions {
    output: PathBuf,
}

impl GenerateM5ApiAuditOptions {
    fn parse(args: &[OsString]) -> Result<Self, String> {
        let mut output = PathBuf::from("evidence/m5/m5-api-audit.json");
        let mut index = 0usize;
        while index < args.len() {
            let Some(value) = args[index].to_str() else {
                return Err("M5 API audit arguments must be valid UTF-8".to_owned());
            };
            match value {
                "--output" => {
                    output = value_arg_path(args, index, "--output")?;
                    index += 2;
                }
                _ => return Err(m5_api_audit_usage()),
            }
        }
        Ok(Self { output })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GenerateM5SecurityBaselineOptions {
    output: PathBuf,
}

impl GenerateM5SecurityBaselineOptions {
    fn parse(args: &[OsString]) -> Result<Self, String> {
        let mut output = PathBuf::from("evidence/m5/m5-security-baseline.json");
        let mut index = 0usize;
        while index < args.len() {
            let Some(value) = args[index].to_str() else {
                return Err("M5 security baseline arguments must be valid UTF-8".to_owned());
            };
            match value {
                "--output" => {
                    output = value_arg_path(args, index, "--output")?;
                    index += 2;
                }
                _ => return Err(m5_security_baseline_usage()),
            }
        }
        Ok(Self { output })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct VerifyM4ReplayOptions {
    path: PathBuf,
    json_output: Option<PathBuf>,
}

impl VerifyM4ReplayOptions {
    fn parse(args: &[OsString]) -> Result<Self, String> {
        if args.is_empty() {
            return Err(m4_replay_verify_usage());
        }

        let mut path = None;
        let mut json_output = None;
        let mut index = 0usize;
        while index < args.len() {
            let Some(value) = args[index].to_str() else {
                return Err("M4 replay arguments must be valid UTF-8".to_owned());
            };
            match value {
                "--json-output" => {
                    json_output = Some(value_arg_path(args, index, "--json-output")?);
                    index += 2;
                }
                flag if flag.starts_with("--") => return Err(m4_replay_verify_usage()),
                _ => {
                    if path.is_some() {
                        return Err(m4_replay_verify_usage());
                    }
                    path = Some(PathBuf::from(&args[index]));
                    index += 1;
                }
            }
        }

        Ok(Self {
            path: path.ok_or_else(m4_replay_verify_usage)?,
            json_output,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExportM4EventStreamOptions {
    fixture: PathBuf,
    output: PathBuf,
}

impl ExportM4EventStreamOptions {
    fn parse(args: &[OsString]) -> Result<Self, String> {
        if args.is_empty() {
            return Err(m4_event_stream_usage());
        }

        let mut fixture = None;
        let mut output = None;
        let mut index = 0usize;
        while index < args.len() {
            let Some(value) = args[index].to_str() else {
                return Err("M4 event stream arguments must be valid UTF-8".to_owned());
            };
            match value {
                "--output" => {
                    output = Some(value_arg_path(args, index, "--output")?);
                    index += 2;
                }
                flag if flag.starts_with("--") => return Err(m4_event_stream_usage()),
                _ => {
                    if fixture.is_some() {
                        return Err(m4_event_stream_usage());
                    }
                    fixture = Some(PathBuf::from(&args[index]));
                    index += 1;
                }
            }
        }

        Ok(Self {
            fixture: fixture.ok_or_else(m4_event_stream_usage)?,
            output: output.ok_or_else(m4_event_stream_usage)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GenerateM5BaselineOptions {
    output: PathBuf,
    m4_report: PathBuf,
    m4_status: PathBuf,
    m4_compatibility: PathBuf,
    m4_package_readiness: PathBuf,
    m4_security: PathBuf,
}

impl GenerateM5BaselineOptions {
    fn parse(args: &[OsString]) -> Result<Self, String> {
        let mut options = Self {
            output: PathBuf::from("evidence/m5/m5-baseline.json"),
            m4_report: PathBuf::from("docs/m4-public-proof-report.md"),
            m4_status: PathBuf::from("tasks/prd-m4-public-proof-status.json"),
            m4_compatibility: PathBuf::from("evidence/m4/compatibility-matrix.json"),
            m4_package_readiness: PathBuf::from("evidence/m4/m4-package-readiness.json"),
            m4_security: PathBuf::from("evidence/m4/m4-oss-security-baseline.json"),
        };

        let mut index = 0usize;
        while index < args.len() {
            let Some(value) = args[index].to_str() else {
                return Err("M5 baseline arguments must be valid UTF-8".to_owned());
            };
            match value {
                "--output" => {
                    options.output = value_arg_path(args, index, "--output")?;
                    index += 2;
                }
                "--m4-report" => {
                    options.m4_report = value_arg_path(args, index, "--m4-report")?;
                    index += 2;
                }
                "--m4-status" => {
                    options.m4_status = value_arg_path(args, index, "--m4-status")?;
                    index += 2;
                }
                "--m4-compatibility" => {
                    options.m4_compatibility = value_arg_path(args, index, "--m4-compatibility")?;
                    index += 2;
                }
                "--m4-package-readiness" => {
                    options.m4_package_readiness =
                        value_arg_path(args, index, "--m4-package-readiness")?;
                    index += 2;
                }
                "--m4-security" => {
                    options.m4_security = value_arg_path(args, index, "--m4-security")?;
                    index += 2;
                }
                _ => return Err(m5_baseline_usage()),
            }
        }

        Ok(options)
    }
}

struct M5SourceRead<T> {
    dependency: M5BaselineDependency,
    value: Option<T>,
}

fn build_m5_baseline(options: &GenerateM5BaselineOptions) -> M5Baseline {
    let generated_at = utc_now();
    let report = read_m5_text_source("m4_report", &options.m4_report);
    let status = read_m5_json_source("m4_status", &options.m4_status);
    let compatibility = read_m5_compatibility_source(&options.m4_compatibility);
    let package = read_m5_json_source("m4_package_readiness", &options.m4_package_readiness);
    let security = read_m5_json_source("m4_oss_security_baseline", &options.m4_security);

    let prd_status = status
        .value
        .as_ref()
        .and_then(|value| value.pointer("/prd/status"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("UNKNOWN")
        .to_owned();

    let mut dependencies = vec![
        report.dependency.clone(),
        status.dependency.clone(),
        compatibility.dependency.clone(),
        package.dependency.clone(),
        security.dependency.clone(),
    ];
    dependencies.sort_by(|left, right| left.id.cmp(&right.id));

    let blockers = vec![
        compatibility_blocker(
            "vt.cursor.csi_positioning",
            "CSI cursor positioning",
            "vt.cursor.csi_positioning",
            "US-004",
            "terminal-core",
            &compatibility,
            &[(
                "evidence/m5/compatibility-matrix.json",
                "M5 compatibility matrix contains cursor positioning row.",
            )],
        ),
        compatibility_blocker(
            "vt.screen.ed_el_ech",
            "ED, EL and ECH erasure",
            "vt.screen.erase_display_line",
            "US-005",
            "terminal-core",
            &compatibility,
            &[(
                "evidence/m5/compatibility-matrix.json",
                "M5 compatibility matrix contains erasure row.",
            )],
        ),
        compatibility_blocker(
            "xterm.private_modes.47_1047_1048",
            "DEC private modes 47, 1047 and 1048",
            "xterm.private_modes.1047_1048",
            "US-006",
            "terminal-core",
            &compatibility,
            &[(
                "evidence/m5/compatibility-matrix.json",
                "M5 compatibility matrix contains DEC private mode policy rows.",
            )],
        ),
        report_blocker(
            M5ReportBlockerSpec {
                id: "replay.real_session_derivatives",
                title: "Real-session replay derivatives",
                category: "replay",
                source_status: "blocked",
                m5_story: "US-008",
                owner: "terminal-fixtures",
                current_paths: &[
                    (
                        "crates/terminal-fixtures/fixtures/m5-replay/codex-session.json",
                        "Codex public-safe replay derivative exists.",
                    ),
                    (
                        "crates/terminal-fixtures/fixtures/m5-replay/claude-code-session.json",
                        "Claude Code public-safe replay derivative exists.",
                    ),
                ],
                note: "M4 reports that real Codex and Claude Code checked-in derivatives are missing.",
            },
            &report,
        ),
        report_blocker(
            M5ReportBlockerSpec {
                id: "platform.linux_macos_measurement",
                title: "Linux and macOS measurement",
                category: "platform",
                source_status: "not_measured",
                m5_story: "US-013",
                owner: "terminal-cli",
                current_paths: &[(
                    "evidence/m5/platform-evidence.json",
                    "M5 platform evidence contains Windows, Linux and macOS command rows.",
                )],
                note: "M4 measured Windows locally but left Linux and macOS runtime rows not measured.",
            },
            &report,
        ),
        json_status_blocker(
            M5JsonBlockerSpec {
                id: "release.package_metadata",
                title: "Package metadata",
                category: "release",
                fallback_status: "partial",
                source_status_override: None,
                m5_story: "US-014",
                owner: "workspace",
                current_paths: &[(
                    "evidence/m5/m5-package-readiness.json",
                    "M5 package readiness records metadata decisions.",
                )],
                note: "M4 package readiness records incomplete Cargo package metadata.",
            },
            &package,
        ),
        json_status_blocker(
            M5JsonBlockerSpec {
                id: "release.publish_order",
                title: "Publish order",
                category: "release",
                fallback_status: "blocked",
                source_status_override: Some("blocked"),
                m5_story: "US-015",
                owner: "workspace",
                current_paths: &[(
                    "evidence/m5/m5-release-plan.json",
                    "M5 release plan records dependency-safe publish order.",
                )],
                note: "M4 package dry-runs block on unpublished internal Hera dependencies.",
            },
            &package,
        ),
        json_status_blocker(
            M5JsonBlockerSpec {
                id: "security.openssf_scorecard",
                title: "OpenSSF Scorecard",
                category: "security",
                fallback_status: "not_measured",
                source_status_override: Some("not_measured"),
                m5_story: "US-017",
                owner: "terminal-cli",
                current_paths: &[(
                    "evidence/m5/m5-security-baseline.json",
                    "M5 security baseline records Scorecard pass or blocked state.",
                )],
                note: "M4 security baseline records OpenSSF Scorecard as not measured.",
            },
            &security,
        ),
    ];

    M5Baseline {
        schema: M5_BASELINE_SCHEMA.to_owned(),
        version: M5_BASELINE_VERSION,
        generated_at,
        m5_status: M5BaselineStatus::Ready,
        source_milestone: M5SourceMilestone {
            id: "M4".to_owned(),
            status_path: repo_relative_path(&options.m4_status),
            prd_status: prd_status.clone(),
            done_status_referenced: prd_status == "DONE",
            history_modified: false,
        },
        dependencies,
        blockers,
    }
}

fn read_m5_text_source(id: &str, path: &Path) -> M5SourceRead<()> {
    let dependency = match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => M5BaselineDependency {
            id: id.to_owned(),
            path: repo_relative_path(path),
            status: M5BaselineDependencyStatus::Available,
            reason: None,
        },
        Ok(_) => M5BaselineDependency {
            id: id.to_owned(),
            path: repo_relative_path(path),
            status: M5BaselineDependencyStatus::Malformed,
            reason: Some("path exists but is not a regular file".to_owned()),
        },
        Err(error) => M5BaselineDependency {
            id: id.to_owned(),
            path: repo_relative_path(path),
            status: M5BaselineDependencyStatus::Missing,
            reason: Some(error.to_string()),
        },
    };

    let value = (dependency.status == M5BaselineDependencyStatus::Available).then_some(());
    M5SourceRead { dependency, value }
}

fn read_m5_json_source(id: &str, path: &Path) -> M5SourceRead<serde_json::Value> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            return M5SourceRead {
                dependency: M5BaselineDependency {
                    id: id.to_owned(),
                    path: repo_relative_path(path),
                    status: M5BaselineDependencyStatus::Missing,
                    reason: Some(error.to_string()),
                },
                value: None,
            };
        }
    };

    match serde_json::from_str(&text) {
        Ok(value) => M5SourceRead {
            dependency: M5BaselineDependency {
                id: id.to_owned(),
                path: repo_relative_path(path),
                status: M5BaselineDependencyStatus::Available,
                reason: None,
            },
            value: Some(value),
        },
        Err(error) => M5SourceRead {
            dependency: M5BaselineDependency {
                id: id.to_owned(),
                path: repo_relative_path(path),
                status: M5BaselineDependencyStatus::Malformed,
                reason: Some(error.to_string()),
            },
            value: None,
        },
    }
}

fn read_m5_compatibility_source(path: &Path) -> M5SourceRead<M4CompatibilityMatrix> {
    match M4CompatibilityMatrix::from_path(path) {
        Ok(matrix) => M5SourceRead {
            dependency: M5BaselineDependency {
                id: "m4_compatibility_matrix".to_owned(),
                path: repo_relative_path(path),
                status: M5BaselineDependencyStatus::Available,
                reason: None,
            },
            value: Some(matrix),
        },
        Err(error) => {
            let status = if path.is_file() {
                M5BaselineDependencyStatus::Malformed
            } else {
                M5BaselineDependencyStatus::Missing
            };
            M5SourceRead {
                dependency: M5BaselineDependency {
                    id: "m4_compatibility_matrix".to_owned(),
                    path: repo_relative_path(path),
                    status,
                    reason: Some(error.to_string()),
                },
                value: None,
            }
        }
    }
}

fn compatibility_blocker(
    id: &str,
    title: &str,
    m4_row_id: &str,
    m5_story: &str,
    owner: &str,
    source: &M5SourceRead<M4CompatibilityMatrix>,
    current_paths: &[(&str, &str)],
) -> M5BaselineBlocker {
    let current_evidence = current_evidence(current_paths);
    let source_status = source
        .value
        .as_ref()
        .and_then(|matrix| {
            matrix
                .rows()
                .iter()
                .find(|row| row.id == m4_row_id)
                .map(|row| m4_status_name(row.status))
        })
        .unwrap_or_else(|| "blocked".to_owned());

    baseline_blocker(
        M5BlockerSeed {
            id,
            title,
            category: "compatibility",
            source_status,
            m5_story,
            owner,
            note: format!("M4 row {m4_row_id} is carried into the M5 compatibility baseline."),
        },
        &source.dependency,
        current_evidence,
    )
}

struct M5ReportBlockerSpec<'a> {
    id: &'a str,
    title: &'a str,
    category: &'a str,
    source_status: &'a str,
    m5_story: &'a str,
    owner: &'a str,
    current_paths: &'a [(&'a str, &'a str)],
    note: &'a str,
}

fn report_blocker(spec: M5ReportBlockerSpec<'_>, source: &M5SourceRead<()>) -> M5BaselineBlocker {
    baseline_blocker(
        M5BlockerSeed {
            id: spec.id,
            title: spec.title,
            category: spec.category,
            source_status: spec.source_status.to_owned(),
            m5_story: spec.m5_story,
            owner: spec.owner,
            note: spec.note.to_owned(),
        },
        &source.dependency,
        current_evidence(spec.current_paths),
    )
}

struct M5JsonBlockerSpec<'a> {
    id: &'a str,
    title: &'a str,
    category: &'a str,
    fallback_status: &'a str,
    source_status_override: Option<&'a str>,
    m5_story: &'a str,
    owner: &'a str,
    current_paths: &'a [(&'a str, &'a str)],
    note: &'a str,
}

fn json_status_blocker(
    spec: M5JsonBlockerSpec<'_>,
    source: &M5SourceRead<serde_json::Value>,
) -> M5BaselineBlocker {
    let source_status = spec
        .source_status_override
        .or_else(|| {
            source
                .value
                .as_ref()
                .and_then(|value| value.get("status"))
                .and_then(serde_json::Value::as_str)
        })
        .unwrap_or(spec.fallback_status)
        .to_owned();

    baseline_blocker(
        M5BlockerSeed {
            id: spec.id,
            title: spec.title,
            category: spec.category,
            source_status,
            m5_story: spec.m5_story,
            owner: spec.owner,
            note: spec.note.to_owned(),
        },
        &source.dependency,
        current_evidence(spec.current_paths),
    )
}

struct M5BlockerSeed<'a> {
    id: &'a str,
    title: &'a str,
    category: &'a str,
    source_status: String,
    m5_story: &'a str,
    owner: &'a str,
    note: String,
}

fn baseline_blocker(
    seed: M5BlockerSeed<'_>,
    source: &M5BaselineDependency,
    current_evidence: Vec<M5CurrentEvidence>,
) -> M5BaselineBlocker {
    let (m5_disposition, blocked_reason) = if source.status != M5BaselineDependencyStatus::Available
    {
        (
            M5BaselineDisposition::BlockedDependency,
            Some(format!(
                "M4 artifact unavailable: {} ({:?})",
                source.path, source.status
            )),
        )
    } else if current_evidence.is_empty() {
        (M5BaselineDisposition::CarriedForward, None)
    } else {
        (M5BaselineDisposition::CurrentEvidence, None)
    };

    M5BaselineBlocker {
        id: seed.id.to_owned(),
        title: seed.title.to_owned(),
        category: seed.category.to_owned(),
        source_path: source.path.clone(),
        source_status: seed.source_status,
        m5_disposition,
        owner: seed.owner.to_owned(),
        m5_story: seed.m5_story.to_owned(),
        current_evidence,
        blocked_reason,
        notes: vec![seed.note],
    }
}

fn current_evidence(paths: &[(&str, &str)]) -> Vec<M5CurrentEvidence> {
    paths
        .iter()
        .filter(|(path, _)| Path::new(path).exists())
        .map(|(path, summary)| M5CurrentEvidence {
            path: (*path).to_owned(),
            status: "current".to_owned(),
            summary: (*summary).to_owned(),
        })
        .collect()
}

fn m4_status_name(status: M4CompatibilityStatus) -> String {
    match status {
        M4CompatibilityStatus::Implemented => "implemented",
        M4CompatibilityStatus::Partial => "partial",
        M4CompatibilityStatus::ManualOnly => "manual_only",
        M4CompatibilityStatus::NotMeasured => "not_measured",
        M4CompatibilityStatus::NotImplemented => "not_implemented",
        M4CompatibilityStatus::OutOfScope => "out_of_scope",
    }
    .to_owned()
}

fn repo_relative_path(path: &Path) -> String {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };
    let display_path = absolute.strip_prefix(&cwd).unwrap_or(path);
    display_path.to_string_lossy().replace('\\', "/")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RunOptions {
    shell: bool,
    shell_command: Option<OsString>,
    command: Vec<OsString>,
    input_file: Option<PathBuf>,
    output_path: Option<PathBuf>,
    record_path: Option<PathBuf>,
    columns: u16,
    rows: u16,
    resize_script: Vec<PtySize>,
    timeout_ms: u64,
}

impl RunOptions {
    fn parse(args: &[OsString]) -> Result<Self, RunParseError> {
        let mut options = Self {
            shell: false,
            shell_command: None,
            command: Vec::new(),
            input_file: None,
            output_path: None,
            record_path: None,
            columns: DEFAULT_COLUMNS,
            rows: DEFAULT_ROWS,
            resize_script: Vec::new(),
            timeout_ms: M2_DEFAULT_COMMAND_TIMEOUT_MS,
        };

        let mut index = 0;
        while index < args.len() {
            let arg = &args[index];
            if arg == "--" {
                options.command.extend_from_slice(&args[index + 1..]);
                break;
            }

            if let Some(flag) = arg.to_str().filter(|value| value.starts_with("--")) {
                match flag {
                    "--shell" => options.shell = true,
                    "--command" => {
                        index += 1;
                        options.shell_command = Some(required_value(args, index, "--command")?);
                    }
                    "--input-file" => {
                        index += 1;
                        options.input_file =
                            Some(PathBuf::from(required_value(args, index, "--input-file")?));
                    }
                    "--output" => {
                        index += 1;
                        options.output_path =
                            Some(PathBuf::from(required_value(args, index, "--output")?));
                    }
                    "--record" => {
                        index += 1;
                        options.record_path =
                            Some(PathBuf::from(required_value(args, index, "--record")?));
                    }
                    "--cols" => {
                        index += 1;
                        options.columns = parse_dimension(required_value(args, index, "--cols")?)?;
                    }
                    "--rows" => {
                        index += 1;
                        options.rows = parse_dimension(required_value(args, index, "--rows")?)?;
                    }
                    "--resize" => {
                        index += 1;
                        options
                            .resize_script
                            .push(parse_resize(required_value(args, index, "--resize")?)?);
                    }
                    "--timeout-ms" => {
                        index += 1;
                        options.timeout_ms =
                            parse_timeout_ms(required_value(args, index, "--timeout-ms")?)?;
                    }
                    _ => {
                        return Err(RunParseError::Usage(
                            "usage: terminal-cli run [options] <command> [args...]",
                        ));
                    }
                }
            } else {
                options.command.push(arg.clone());
                options.command.extend_from_slice(&args[index + 1..]);
                break;
            }

            index += 1;
        }

        if !options.shell && options.shell_command.is_some() {
            return Err(RunParseError::Usage(
                "usage: terminal-cli run --shell --command <command>",
            ));
        }

        if !options.shell && options.command.is_empty() {
            return Err(RunParseError::Usage("usage: terminal-cli run <command>"));
        }

        if let Some(output_path) = options.output_path.as_deref() {
            ensure_output_parent_exists(output_path)?;
        }

        if let Some(input_file) = options.input_file.as_deref() {
            let metadata = fs::metadata(input_file).map_err(|error| {
                RunParseError::Message(format!("{}: {error}", input_file.display()))
            })?;
            if !metadata.is_file() {
                return Err(RunParseError::Message(format!(
                    "{}: expected a regular input file",
                    input_file.display()
                )));
            }
            if metadata.len() > M2_CLI_MAX_INPUT_BYTES as u64 {
                return Err(RunParseError::Message(format!(
                    "{}: input file is {} bytes, maximum is {M2_CLI_MAX_INPUT_BYTES}",
                    input_file.display(),
                    metadata.len()
                )));
            }
        }

        Ok(options)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RunParseError {
    Usage(&'static str),
    Message(String),
}

#[derive(Debug)]
enum RunError {
    Message { code: u8, message: String },
}

impl RunError {
    fn message(message: impl std::fmt::Display) -> Self {
        Self::Message {
            code: 1,
            message: message.to_string(),
        }
    }

    const fn exit_code(&self) -> u8 {
        match self {
            Self::Message { code, .. } => *code,
        }
    }
}

impl std::fmt::Display for RunError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Message { message, .. } => formatter.write_str(message),
        }
    }
}

impl std::error::Error for RunError {}

struct RunExecution {
    run: RunArtifact,
    recording: PtyRecording,
}

fn execute_run(options: &RunOptions) -> Result<RunExecution, RunError> {
    let size = PtySize::new(options.columns, options.rows).map_err(RunError::message)?;
    let command_plan = build_command_plan(options)?;
    let runtime = PtyRuntimeConfig::default()
        .with_command_timeout(Duration::from_millis(options.timeout_ms))
        .map_err(RunError::message)?;
    let input_bytes = read_optional_input(options.input_file.as_deref())?;
    let input_byte_count = input_bytes.as_ref().map_or(0, Vec::len);
    let backend = PortablePtyBackend::new();
    let mut bridge = PtyBridge::new(CoreRunSink::new(size).map_err(RunError::message)?);
    let config = PtySessionConfig::new(command_plan.command, size);

    let session = backend.spawn(&config).map_err(|error| {
        RunError::message(format!("run {}: {error}", command_plan.metadata.program))
    })?;
    let platform = session.metadata().clone();
    let mut runner = PtySessionRunner::new(session, runtime).map_err(RunError::message)?;
    let started = Instant::now();
    let mut recording = RecordingState::new(options.record_path.is_some());

    for resize in &options.resize_script {
        bridge
            .resize_session(&mut runner, *resize)
            .map_err(format_bridge_error)?;
        recording.push_resize(started, *resize);
    }

    if let Some(bytes) = input_bytes {
        runner.write_input(&bytes).map_err(RunError::message)?;
        recording.push_input(
            started,
            bytes,
            options
                .input_file
                .as_deref()
                .map(|path| path.display().to_string()),
        );
        runner.close_input();
    } else if command_plan.close_input_after_spawn {
        runner.close_input();
    }

    let mut event_error = None;
    let mut query_responder = TerminalQueryResponder::default();
    let outcome = runner
        .run_until_exit_with_responses(|event| {
            let mut response = None;
            if event_error.is_none() {
                if let Err(error) = bridge.apply_event(&event) {
                    event_error = Some(error.to_string());
                } else if let PtyEvent::Output(bytes) = &event {
                    response = query_responder.response_for_output(bytes, bridge.sink());
                }
            }
            recording.push_pty_event(started, &event);
            if let Some(bytes) = response.as_ref() {
                recording.push_input(
                    started,
                    bytes.clone(),
                    Some("terminal_query_response".to_owned()),
                );
            }
            Ok(response)
        })
        .map_err(RunError::message)?;

    if let Some(error) = event_error {
        return Err(RunError::message(error));
    }

    let snapshot = bridge.sink_mut().snapshot();
    let metadata = run_metadata(
        command_plan.metadata,
        platform,
        outcome,
        options,
        size,
        input_byte_count,
        recording.metadata(),
    );
    let run = RunArtifact {
        snapshot: snapshot.clone(),
        metadata: metadata.clone(),
    };
    let recording = PtyRecording {
        schema: terminal_fixtures::M2_PTY_RECORDING_SCHEMA.to_owned(),
        version: terminal_fixtures::M2_PTY_RECORDING_VERSION,
        metadata,
        events: recording.into_events(),
        final_snapshot: snapshot,
    };

    Ok(RunExecution { run, recording })
}

fn build_command_plan(options: &RunOptions) -> Result<CommandPlan, RunError> {
    if options.shell {
        let selection = ShellSelection::default_shell().map_err(RunError::message)?;
        let args = selection.args_for(options.shell_command.as_deref());
        let command = PtyCommand::new(selection.program.clone()).args(args.clone());
        let metadata = PtyRecordingCommandMetadata {
            mode: PtyRecordingCommandMode::Shell,
            program: os_to_string(&selection.program),
            args: args.iter().map(os_to_string).collect(),
            shell_command: options.shell_command.as_deref().map(os_to_string),
        };

        return Ok(CommandPlan {
            command,
            metadata,
            close_input_after_spawn: options.shell_command.is_none(),
        });
    }

    let mut command_args = options.command.iter();
    let program = command_args
        .next()
        .ok_or_else(|| RunError::message("usage: terminal-cli run <command>"))?;
    let args = command_args.cloned().collect::<Vec<_>>();
    let command = PtyCommand::new(program.clone()).args(args.clone());
    let metadata = PtyRecordingCommandMetadata {
        mode: PtyRecordingCommandMode::Direct,
        program: os_to_string(program),
        args: args.iter().map(os_to_string).collect(),
        shell_command: None,
    };

    Ok(CommandPlan {
        command,
        metadata,
        close_input_after_spawn: false,
    })
}

struct CommandPlan {
    command: PtyCommand,
    metadata: PtyRecordingCommandMetadata,
    close_input_after_spawn: bool,
}

struct CoreRunSink {
    terminal: Terminal,
}

impl CoreRunSink {
    fn new(size: PtySize) -> Result<Self, TerminalError> {
        Ok(Self {
            terminal: Terminal::new(usize::from(size.columns()), usize::from(size.rows()))?,
        })
    }

    fn snapshot(&mut self) -> TerminalSnapshot {
        snapshot_terminal(&mut self.terminal)
    }

    fn cursor_position_response(&self) -> Vec<u8> {
        let cursor = self.terminal.cursor();
        format!(
            "\x1b[{};{}R",
            cursor.row().saturating_add(1),
            cursor.column().saturating_add(1)
        )
        .into_bytes()
    }
}

impl PtyEventSink for CoreRunSink {
    type Error = TerminalError;

    fn apply_output(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        self.terminal.advance_bytes(bytes);
        Ok(())
    }

    fn resize(&mut self, size: PtySize) -> Result<(), Self::Error> {
        self.terminal
            .resize(usize::from(size.columns()), usize::from(size.rows()))
    }

    fn current_size(&self) -> Option<PtySize> {
        let dimensions = self.terminal.dimensions();
        PtySize::new(
            u16::try_from(dimensions.columns()).ok()?,
            u16::try_from(dimensions.rows()).ok()?,
        )
        .ok()
    }
}

#[derive(Debug, Clone, Serialize)]
struct RunArtifact {
    snapshot: TerminalSnapshot,
    metadata: PtyRecordingMetadata,
}

struct RecordingState {
    enabled: bool,
    events: Vec<PtyRecordingEvent>,
    output_bytes: usize,
    output_truncated: bool,
}

impl RecordingState {
    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            events: Vec::new(),
            output_bytes: 0,
            output_truncated: false,
        }
    }

    fn push_resize(&mut self, started: Instant, size: PtySize) {
        if !self.enabled {
            return;
        }

        self.events.push(PtyRecordingEvent::Resize {
            elapsed_ms: elapsed_ms(started),
            columns: size.columns(),
            rows: size.rows(),
        });
    }

    fn push_input(&mut self, started: Instant, bytes: Vec<u8>, source: Option<String>) {
        if !self.enabled {
            return;
        }

        self.events.push(PtyRecordingEvent::Input {
            elapsed_ms: elapsed_ms(started),
            bytes,
            source,
        });
    }

    fn push_pty_event(&mut self, started: Instant, event: &PtyEvent) {
        if !self.enabled {
            return;
        }

        match event {
            PtyEvent::Output(bytes) => self.push_output(started, bytes),
            PtyEvent::Eof => self.events.push(PtyRecordingEvent::Eof {
                elapsed_ms: elapsed_ms(started),
            }),
            PtyEvent::Exit(exit) => self.events.push(PtyRecordingEvent::Exit {
                elapsed_ms: elapsed_ms(started),
                exit: recording_exit(exit),
            }),
            PtyEvent::Resize(size) => self.push_resize(started, *size),
        }
    }

    fn push_output(&mut self, started: Instant, bytes: &[u8]) {
        if self.output_truncated {
            return;
        }

        let remaining = M2_CLI_MAX_RECORDING_OUTPUT_BYTES.saturating_sub(self.output_bytes);
        if remaining == 0 {
            self.output_truncated = true;
            return;
        }

        let retained = bytes.len().min(remaining);
        self.output_bytes = self.output_bytes.saturating_add(retained);
        self.events.push(PtyRecordingEvent::Output {
            elapsed_ms: elapsed_ms(started),
            bytes: bytes[..retained].to_vec(),
        });

        if retained < bytes.len() {
            self.output_truncated = true;
        }
    }

    const fn metadata(&self) -> PtyRecordingStorageMetadata {
        PtyRecordingStorageMetadata {
            enabled: self.enabled,
            output_bytes: self.output_bytes,
            output_truncated: self.output_truncated,
            max_output_bytes: M2_CLI_MAX_RECORDING_OUTPUT_BYTES,
        }
    }

    fn into_events(self) -> Vec<PtyRecordingEvent> {
        self.events
    }
}

#[derive(Default)]
struct TerminalQueryResponder {
    tail: Vec<u8>,
}

impl TerminalQueryResponder {
    fn response_for_output(&mut self, bytes: &[u8], sink: &CoreRunSink) -> Option<Vec<u8>> {
        const CURSOR_POSITION_QUERY: &[u8] = b"\x1b[6n";

        let mut combined = Vec::with_capacity(self.tail.len() + bytes.len());
        combined.extend_from_slice(&self.tail);
        combined.extend_from_slice(bytes);

        let found = contains_subsequence(&combined, CURSOR_POSITION_QUERY);
        let tail_len = CURSOR_POSITION_QUERY.len().saturating_sub(1);
        let start = combined.len().saturating_sub(tail_len);
        self.tail = combined[start..].to_vec();

        found.then(|| sink.cursor_position_response())
    }
}

fn run_metadata(
    command: PtyRecordingCommandMetadata,
    platform: PtyPlatformMetadata,
    outcome: PtyRunOutcome,
    options: &RunOptions,
    initial_size: PtySize,
    input_byte_count: usize,
    recording: PtyRecordingStorageMetadata,
) -> PtyRecordingMetadata {
    PtyRecordingMetadata {
        command,
        platform: PtyRecordingPlatformMetadata {
            pty_backend: platform.backend().to_owned(),
            process_id: platform.process_id(),
            initial_size: recording_size(platform.size()),
        },
        exit: recording_exit(outcome.exit()),
        runtime: PtyRecordingRuntimeMetadata {
            output_bytes: outcome.output_bytes(),
            output_chunks: outcome.output_chunks(),
            saw_eof: outcome.saw_eof(),
            drain_timed_out: outcome.drain_timed_out(),
            timeout_ms: options.timeout_ms,
        },
        initial_size: recording_size(initial_size),
        input: PtyRecordingInputMetadata {
            path: options
                .input_file
                .as_deref()
                .map(|path| path.display().to_string()),
            bytes: input_byte_count,
            max_bytes: M2_CLI_MAX_INPUT_BYTES,
            stdin_closed_after_input: options.input_file.is_some(),
        },
        resizes: options
            .resize_script
            .iter()
            .copied()
            .map(recording_size)
            .collect(),
        recording,
    }
}

fn recording_size(size: PtySize) -> PtyRecordingSize {
    PtyRecordingSize {
        columns: size.columns(),
        rows: size.rows(),
    }
}

fn recording_exit(exit: &PtyExit) -> PtyRecordingExitMetadata {
    PtyRecordingExitMetadata {
        code: exit.code(),
        signal: exit.signal().map(ToOwned::to_owned),
        success: exit.success(),
    }
}

fn emit_run_output(options: &RunOptions, artifact: &RunArtifact) -> Result<String, String> {
    let json = serialize_json(artifact)?;
    if let Some(path) = options.output_path.as_deref() {
        fs::write(path, format!("{json}\n"))
            .map_err(|error| format!("output write failed: {}: {error}", path.display()))?;
        Ok(format!("wrote run snapshot to {}", path.display()))
    } else {
        Ok(json)
    }
}

fn write_recording(path: &Path, recording: &PtyRecording) -> Result<(), String> {
    let json = serialize_pty_recording_pretty(recording)
        .map_err(|error| format!("record serialization failed: {error}"))?;
    fs::write(path, format!("{json}\n"))
        .map_err(|error| format!("record serialization failed: {}: {error}", path.display()))
}

fn serialize_json<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string_pretty(value).map_err(|error| error.to_string())
}

fn read_optional_input(path: Option<&Path>) -> Result<Option<Vec<u8>>, RunError> {
    path.map(|path| {
        read_bytes_capped(path, M2_CLI_MAX_INPUT_BYTES)
            .map_err(|message| RunError::message(format!("input file {message}")))
    })
    .transpose()
}

fn format_bridge_error(error: PtyBridgeError<TerminalError>) -> RunError {
    RunError::message(error)
}

fn child_exit_code(code: u32) -> u8 {
    u8::try_from(code).unwrap_or(1)
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn contains_subsequence(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

#[derive(Debug, Clone, Copy)]
struct M5PlatformCommandSpec {
    id: M5PlatformCommandId,
    program: &'static str,
    args: &'static [&'static str],
    artifact_paths: &'static [&'static str],
}

fn build_m5_platform_evidence(
    generated_at: &str,
    source_command: &str,
    measured_row: Option<M5PlatformRow>,
    output_path: &str,
    note: &str,
) -> M5PlatformRuntimeEvidence {
    let windows = measured_row
        .clone()
        .filter(|row| row.platform == M5RuntimePlatform::Windows)
        .unwrap_or_else(|| {
            blocked_platform_row(
                M5RuntimePlatform::Windows,
                generated_at,
                output_path,
                "No Windows runner is available in this local session.",
            )
        });
    let linux = measured_row
        .clone()
        .filter(|row| row.platform == M5RuntimePlatform::Linux)
        .unwrap_or_else(|| {
            blocked_platform_row(
                M5RuntimePlatform::Linux,
                generated_at,
                output_path,
                "No Linux runner is available in this local session.",
            )
        });
    let macos = measured_row
        .filter(|row| row.platform == M5RuntimePlatform::Macos)
        .unwrap_or_else(|| {
            blocked_platform_row(
                M5RuntimePlatform::Macos,
                generated_at,
                output_path,
                "No macOS runner is available in this local session.",
            )
        });

    M5PlatformRuntimeEvidence {
        schema: terminal_fixtures::M5_PLATFORM_RUNTIME_EVIDENCE_SCHEMA.to_owned(),
        version: terminal_fixtures::M5_PLATFORM_RUNTIME_EVIDENCE_VERSION,
        generated_at: generated_at.to_owned(),
        source_command: source_command.to_owned(),
        freshness_policy: M5PlatformFreshnessPolicy {
            generated_after: "2026-07-04T00:00:00Z".to_owned(),
            stale_after_days: 7,
        },
        required_commands: m5_required_platform_commands(),
        platforms: M5PlatformRows {
            windows,
            linux,
            macos,
        },
        notes: vec![note.to_owned()],
    }
}

fn measure_current_platform_row(
    platform: M5RuntimePlatform,
    generated_at: &str,
    output_path: &str,
) -> M5PlatformRow {
    let rustc = rustc_metadata();
    let commands = m5_platform_command_specs()
        .iter()
        .map(run_m5_platform_command)
        .collect::<Vec<_>>();
    let status = rollup_platform_status(&commands);
    let blocked_reason = (status == M5PlatformEvidenceStatus::Blocked)
        .then(|| "At least one required command was unavailable on this platform.".to_owned());
    let mut artifact_paths = vec![output_path.to_owned()];
    for command in &commands {
        for path in &command.artifact_paths {
            if !artifact_paths.contains(path) {
                artifact_paths.push(path.clone());
            }
        }
    }
    let mut notes = Vec::new();
    if let Some(error) = rustc.error {
        notes.push(format!("rustc metadata warning: {error}"));
    }

    M5PlatformRow {
        platform,
        status,
        os: std::env::consts::OS.to_owned(),
        target_triple: rustc.host,
        rustc_version: rustc.version,
        generated_at: generated_at.to_owned(),
        commands,
        artifact_paths,
        blocked_reason,
        notes,
    }
}

fn blocked_platform_row(
    platform: M5RuntimePlatform,
    generated_at: &str,
    output_path: &str,
    reason: &str,
) -> M5PlatformRow {
    let commands = m5_platform_command_specs()
        .iter()
        .map(|spec| blocked_m5_platform_command(spec, reason))
        .collect::<Vec<_>>();
    M5PlatformRow {
        platform,
        status: M5PlatformEvidenceStatus::Blocked,
        os: platform_label(platform).to_owned(),
        target_triple: "not_measured".to_owned(),
        rustc_version: "not_measured".to_owned(),
        generated_at: generated_at.to_owned(),
        commands,
        artifact_paths: vec![output_path.to_owned()],
        blocked_reason: Some(reason.to_owned()),
        notes: vec![format!(
            "Run `terminal-cli measure-m5-platform --json-output {output_path}` on {} or CI to replace this blocked row.",
            platform_label(platform)
        )],
    }
}

fn run_m5_platform_command(spec: &M5PlatformCommandSpec) -> M5PlatformCommandResult {
    let started = Instant::now();
    match ProcessCommand::new(spec.program).args(spec.args).output() {
        Ok(output) => {
            let exit_code = output.status.code().unwrap_or(1);
            let status = if output.status.success() {
                M5PlatformCommandStatus::Pass
            } else {
                M5PlatformCommandStatus::Failed
            };
            M5PlatformCommandResult {
                id: spec.id,
                command: command_string(spec.program, spec.args),
                status,
                exit_code: Some(exit_code),
                duration_ms: Some(elapsed_ms(started)),
                stdout_summary: public_output_summary(&output.stdout),
                stderr_summary: public_output_summary(&output.stderr),
                artifact_paths: spec
                    .artifact_paths
                    .iter()
                    .map(|path| (*path).to_owned())
                    .collect(),
                blocked_reason: None,
            }
        }
        Err(error) => M5PlatformCommandResult {
            id: spec.id,
            command: command_string(spec.program, spec.args),
            status: M5PlatformCommandStatus::Blocked,
            exit_code: None,
            duration_ms: Some(elapsed_ms(started)),
            stdout_summary: String::new(),
            stderr_summary: sanitize_public_summary(&error.to_string()),
            artifact_paths: Vec::new(),
            blocked_reason: Some(format!("command unavailable: {error}")),
        },
    }
}

fn blocked_m5_platform_command(
    spec: &M5PlatformCommandSpec,
    reason: &str,
) -> M5PlatformCommandResult {
    M5PlatformCommandResult {
        id: spec.id,
        command: command_string(spec.program, spec.args),
        status: M5PlatformCommandStatus::Blocked,
        exit_code: None,
        duration_ms: None,
        stdout_summary: String::new(),
        stderr_summary: reason.to_owned(),
        artifact_paths: spec
            .artifact_paths
            .iter()
            .map(|path| (*path).to_owned())
            .collect(),
        blocked_reason: Some(reason.to_owned()),
    }
}

fn rollup_platform_status(commands: &[M5PlatformCommandResult]) -> M5PlatformEvidenceStatus {
    if commands
        .iter()
        .any(|command| command.status == M5PlatformCommandStatus::Failed)
    {
        M5PlatformEvidenceStatus::Failed
    } else if commands
        .iter()
        .any(|command| command.status == M5PlatformCommandStatus::Blocked)
    {
        M5PlatformEvidenceStatus::Blocked
    } else {
        M5PlatformEvidenceStatus::Pass
    }
}

fn m5_platform_command_specs() -> [M5PlatformCommandSpec; 5] {
    [
        M5PlatformCommandSpec {
            id: M5PlatformCommandId::CargoCheckWorkspace,
            program: "cargo",
            args: &["check", "--workspace"],
            artifact_paths: &[],
        },
        M5PlatformCommandSpec {
            id: M5PlatformCommandId::CargoTestWorkspace,
            program: "cargo",
            args: &["test", "--workspace"],
            artifact_paths: &[],
        },
        M5PlatformCommandSpec {
            id: M5PlatformCommandId::CargoDocWorkspace,
            program: "cargo",
            args: &["doc", "--workspace", "--no-deps"],
            artifact_paths: &[],
        },
        M5PlatformCommandSpec {
            id: M5PlatformCommandId::ValidateM5Compatibility,
            program: "cargo",
            args: &[
                "run",
                "-p",
                "terminal-cli",
                "--",
                "validate-m5-compatibility",
                "evidence/m5/compatibility-matrix.json",
            ],
            artifact_paths: &[],
        },
        M5PlatformCommandSpec {
            id: M5PlatformCommandId::VerifyM5Replay,
            program: "cargo",
            args: &[
                "run",
                "-p",
                "terminal-cli",
                "--",
                "verify-m5-replay",
                "crates/terminal-fixtures/fixtures/m5-replay",
                "--json-output",
                "evidence/m5/m5-replay-verification.json",
            ],
            artifact_paths: &["evidence/m5/m5-replay-verification.json"],
        },
    ]
}

#[derive(Debug, Clone)]
struct RustcMetadata {
    version: String,
    host: String,
    error: Option<String>,
}

fn rustc_metadata() -> RustcMetadata {
    match ProcessCommand::new("rustc").arg("-Vv").output() {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let version = stdout
                .lines()
                .next()
                .filter(|line| !line.trim().is_empty())
                .unwrap_or("rustc unknown")
                .to_owned();
            let host = stdout
                .lines()
                .find_map(|line| line.strip_prefix("host: "))
                .unwrap_or("unknown")
                .to_owned();
            RustcMetadata {
                version,
                host,
                error: None,
            }
        }
        Ok(output) => RustcMetadata {
            version: "rustc not_measured".to_owned(),
            host: "not_measured".to_owned(),
            error: Some(public_output_summary(&output.stderr)),
        },
        Err(error) => RustcMetadata {
            version: "rustc not_measured".to_owned(),
            host: "not_measured".to_owned(),
            error: Some(error.to_string()),
        },
    }
}

fn current_runtime_platform() -> Option<M5RuntimePlatform> {
    if cfg!(windows) {
        Some(M5RuntimePlatform::Windows)
    } else if cfg!(target_os = "linux") {
        Some(M5RuntimePlatform::Linux)
    } else if cfg!(target_os = "macos") {
        Some(M5RuntimePlatform::Macos)
    } else {
        None
    }
}

fn platform_label(platform: M5RuntimePlatform) -> &'static str {
    match platform {
        M5RuntimePlatform::Windows => "Windows",
        M5RuntimePlatform::Linux => "Linux",
        M5RuntimePlatform::Macos => "macOS",
    }
}

fn command_string(program: &str, args: &[&str]) -> String {
    let mut parts = Vec::with_capacity(args.len() + 1);
    parts.push(program.to_owned());
    parts.extend(args.iter().map(|arg| (*arg).to_owned()));
    parts.join(" ")
}

fn repo_path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn public_output_summary(bytes: &[u8]) -> String {
    sanitize_public_summary(&String::from_utf8_lossy(bytes))
}

fn sanitize_public_summary(raw: &str) -> String {
    let mut text = raw.replace("\r\n", "\n").replace('\r', "\n");
    if let Ok(current_dir) = std::env::current_dir() {
        let current = current_dir.to_string_lossy();
        text = text.replace(current.as_ref(), ".");
        text = text.replace(&current.replace('\\', "/"), ".");
    }
    if let Some(profile) = std::env::var_os("USERPROFILE") {
        let profile = profile.to_string_lossy();
        text = text.replace(profile.as_ref(), "%USERPROFILE%");
        text = text.replace(&profile.replace('\\', "/"), "%USERPROFILE%");
    }
    if let Some(home) = std::env::var_os("HOME") {
        let home = home.to_string_lossy();
        text = text.replace(home.as_ref(), "$HOME");
        text = text.replace(&home.replace('\\', "/"), "$HOME");
    }
    text = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    truncate_chars(&text, 1200)
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }
    let mut truncated = text.chars().take(max_chars).collect::<String>();
    truncated.push_str("...");
    truncated
}

#[derive(Debug, Clone, Copy)]
struct M5CrateSpec {
    name: &'static str,
    manifest_path: &'static str,
    depends_on: &'static [&'static str],
    boundary: &'static str,
    exported_surface: &'static [&'static str],
    release_rationale: &'static str,
}

const M5_PUBLIC_CRATES: [M5CrateSpec; 6] = [
    M5CrateSpec {
        name: "terminal-protocol",
        manifest_path: "crates/terminal-protocol/Cargo.toml",
        depends_on: &[],
        boundary: "Hera-owned protocol and replay data types.",
        exported_surface: &["TerminalAction", "CsiSequence", "OscCommand", "Payload"],
        release_rationale: "Leaf protocol crate that other public crates depend on.",
    },
    M5CrateSpec {
        name: "terminal-render-model",
        manifest_path: "crates/terminal-render-model/Cargo.toml",
        depends_on: &[],
        boundary: "Renderer-neutral cells, rows, cursor and snapshot data.",
        exported_surface: &[
            "RenderSnapshot",
            "RenderCell",
            "ViewportRow",
            "ImagePlaceholder",
        ],
        release_rationale: "Leaf render contract used by terminal-core embedders.",
    },
    M5CrateSpec {
        name: "terminal-core",
        manifest_path: "crates/terminal-core/Cargo.toml",
        depends_on: &["terminal-protocol", "terminal-render-model"],
        boundary: "Headless terminal state and parser integration with no PTY or renderer runtime.",
        exported_surface: &[
            "Terminal",
            "TerminalConfig",
            "TerminalError",
            "ScrollbackConfig",
        ],
        release_rationale: "Core engine depends on protocol and render-model surfaces.",
    },
    M5CrateSpec {
        name: "terminal-fixtures",
        manifest_path: "crates/terminal-fixtures/Cargo.toml",
        depends_on: &["terminal-core"],
        boundary: "Fixture, replay and milestone evidence validation utilities.",
        exported_surface: &["FixtureRunner", "M5EvidenceManifest", "M5PackageReadiness"],
        release_rationale: "Fixture harness depends on terminal-core behavior.",
    },
    M5CrateSpec {
        name: "terminal-pty",
        manifest_path: "crates/terminal-pty/Cargo.toml",
        depends_on: &["terminal-core"],
        boundary: "PTY runtime abstraction that keeps portable-pty out of public Hera state.",
        exported_surface: &[
            "PtyCommand",
            "PtySessionRunner",
            "PtyRuntimeConfig",
            "PtyError",
        ],
        release_rationale: "Runtime crate packages after terminal-core because its package dry-run includes that dev dependency.",
    },
    M5CrateSpec {
        name: "terminal-cli",
        manifest_path: "crates/terminal-cli/Cargo.toml",
        depends_on: &["terminal-core", "terminal-fixtures", "terminal-pty"],
        boundary: "Debug and evidence commands over the public Hera crates.",
        exported_surface: &["terminal-cli binary commands"],
        release_rationale: "CLI packages last because it depends on core, fixtures and PTY runtime crates.",
    },
];

fn m5_public_crate_specs() -> &'static [M5CrateSpec] {
    &M5_PUBLIC_CRATES
}

fn build_m5_release_plan(generated_at: &str, source_command: &str) -> M5ReleasePlan {
    terminal_fixtures::M5ReleasePlan {
        schema: terminal_fixtures::M5_RELEASE_PLAN_SCHEMA.to_owned(),
        version: terminal_fixtures::M5_RELEASE_EVIDENCE_VERSION,
        generated_at: generated_at.to_owned(),
        source_command: source_command.to_owned(),
        publish_out_of_scope: true,
        publish_order: m5_public_crate_specs()
            .iter()
            .map(|spec| spec.name.to_owned())
            .collect(),
        crates: m5_public_crate_specs()
            .iter()
            .enumerate()
            .map(|(index, spec)| terminal_fixtures::M5ReleasePlanCrate {
                name: spec.name.to_owned(),
                publish_intent: terminal_fixtures::M5PublishIntent::IntendedPublic,
                publish_config: "publish = true".to_owned(),
                release_step: u32::try_from(index + 1).unwrap_or(u32::MAX),
                depends_on: spec
                    .depends_on
                    .iter()
                    .map(|dependency| (*dependency).to_owned())
                    .collect(),
                rationale: spec.release_rationale.to_owned(),
            })
            .collect(),
        notes: vec![
            "Publish order is derived from Hera workspace dependencies and keeps dependencies before dependents."
                .to_owned(),
            "cargo publish is not run by M5 and remains a future maintainer action.".to_owned(),
        ],
    }
}

fn build_m5_package_crate(spec: &M5CrateSpec) -> Result<terminal_fixtures::M5PackageCrate, String> {
    let manifest = read_manifest_value(Path::new(spec.manifest_path))?;
    let package = manifest
        .get("package")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{}: missing [package]", spec.manifest_path))?;
    let description = toml_string(package, "description", spec.manifest_path)?;
    let license = toml_string(package, "license", spec.manifest_path)?;
    let repository = toml_string(package, "repository", spec.manifest_path)?;
    let documentation = toml_string(package, "documentation", spec.manifest_path)?;
    let readme = toml_string(package, "readme", spec.manifest_path)?;
    let keywords = toml_string_array(package, "keywords", spec.manifest_path)?;
    let categories = toml_string_array(package, "categories", spec.manifest_path)?;
    let docs_rs = docs_rs_policy(&manifest, spec.manifest_path)?;
    let dry_run = run_m5_package_dry_run(spec.name);
    let package_status = if dry_run.status == terminal_fixtures::M5DryRunStatus::Pass {
        terminal_fixtures::M5ReadinessStatus::Pass
    } else {
        terminal_fixtures::M5ReadinessStatus::Blocked
    };

    Ok(terminal_fixtures::M5PackageCrate {
        name: spec.name.to_owned(),
        manifest_path: spec.manifest_path.to_owned(),
        publish_intent: terminal_fixtures::M5PublishIntent::IntendedPublic,
        package_status,
        description,
        license,
        repository,
        documentation,
        readme,
        keywords,
        categories,
        docs_rs,
        metadata_omissions: vec![terminal_fixtures::M5MetadataOmission {
            field: "homepage".to_owned(),
            reason: "No dedicated crate homepage exists beyond repository and docs.rs at M5."
                .to_owned(),
        }],
        dry_run,
        notes: vec![
            "Crate is intended for public pre-release packaging, but not published by M5."
                .to_owned(),
        ],
    })
}

fn run_m5_package_dry_run(crate_name: &str) -> terminal_fixtures::M5PackageDryRun {
    let args = ["package", "-p", crate_name, "--allow-dirty", "--no-verify"];
    let command = command_string("cargo", &args);
    let started = Instant::now();
    match ProcessCommand::new("cargo").args(args).output() {
        Ok(output) => {
            let exit_code = output.status.code().unwrap_or(1);
            let status = if output.status.success() {
                terminal_fixtures::M5DryRunStatus::Pass
            } else {
                terminal_fixtures::M5DryRunStatus::Failed
            };
            terminal_fixtures::M5PackageDryRun {
                command,
                status,
                exit_code: Some(exit_code),
                duration_ms: Some(elapsed_ms(started)),
                stdout_summary: public_output_summary(&output.stdout),
                stderr_summary: public_output_summary(&output.stderr),
                package_archive: package_archive_path(crate_name),
                blocked_reason: None,
            }
        }
        Err(error) => terminal_fixtures::M5PackageDryRun {
            command,
            status: terminal_fixtures::M5DryRunStatus::Blocked,
            exit_code: None,
            duration_ms: Some(elapsed_ms(started)),
            stdout_summary: String::new(),
            stderr_summary: sanitize_public_summary(&error.to_string()),
            package_archive: None,
            blocked_reason: Some(format!("cargo package could not be executed: {error}")),
        },
    }
}

fn package_archive_path(crate_name: &str) -> Option<String> {
    let path = PathBuf::from("target")
        .join("package")
        .join(format!("{crate_name}-0.1.0.crate"));
    path.exists().then(|| repo_path_string(&path))
}

fn read_manifest_value(path: &Path) -> Result<toml::Value, String> {
    let raw = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    toml::from_str(&raw).map_err(|error| format!("{}: {error}", path.display()))
}

fn toml_string(table: &toml::Table, key: &str, manifest_path: &str) -> Result<String, String> {
    table
        .get(key)
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("{manifest_path}: [package].{key} must be a string"))
}

fn toml_string_array(
    table: &toml::Table,
    key: &str,
    manifest_path: &str,
) -> Result<Vec<String>, String> {
    table
        .get(key)
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{manifest_path}: [package].{key} must be an array"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("{manifest_path}: [package].{key} must contain strings"))
        })
        .collect()
}

fn docs_rs_policy(
    manifest: &toml::Value,
    manifest_path: &str,
) -> Result<terminal_fixtures::M5DocsRsPolicy, String> {
    let docs = manifest
        .get("package")
        .and_then(|package| package.get("metadata"))
        .and_then(|metadata| metadata.get("docs"))
        .and_then(|docs| docs.get("rs"))
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{manifest_path}: missing [package.metadata.docs.rs]"))?;
    let all_features = docs
        .get("all-features")
        .and_then(toml::Value::as_bool)
        .unwrap_or(false);
    let default_target = docs
        .get("default-target")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("{manifest_path}: docs.rs default-target is required"))?
        .to_owned();
    let targets = toml_string_array(docs, "targets", manifest_path)?;

    Ok(terminal_fixtures::M5DocsRsPolicy {
        all_features,
        default_target,
        targets,
        cargo_args: Vec::new(),
        rustdoc_args: Vec::new(),
    })
}

fn build_m5_api_audit(
    generated_at: String,
    source_command: String,
) -> terminal_fixtures::M5ApiAudit {
    let semver_baseline = run_semver_baseline();
    let semver_pass = matches!(
        semver_baseline.status,
        terminal_fixtures::M5ToolStatus::Pass
    );
    let audit_status = if matches!(
        semver_baseline.status,
        terminal_fixtures::M5ToolStatus::Failed
    ) {
        terminal_fixtures::M5ReadinessStatus::Blocked
    } else {
        terminal_fixtures::M5ReadinessStatus::Pass
    };
    terminal_fixtures::M5ApiAudit {
        schema: terminal_fixtures::M5_API_AUDIT_SCHEMA.to_owned(),
        version: terminal_fixtures::M5_RELEASE_EVIDENCE_VERSION,
        generated_at,
        source_command,
        status: audit_status,
        sources: vec![
            terminal_fixtures::M5AuditSource {
                label: "Rust API Guidelines checklist".to_owned(),
                url: "https://rust-lang.github.io/api-guidelines/checklist.html".to_owned(),
            },
            terminal_fixtures::M5AuditSource {
                label: "Cargo publishing guidance".to_owned(),
                url: "https://doc.rust-lang.org/cargo/reference/publishing.html".to_owned(),
            },
            terminal_fixtures::M5AuditSource {
                label: "docs.rs metadata".to_owned(),
                url: "https://docs.rs/about/metadata".to_owned(),
            },
        ],
        public_crates: m5_public_crate_specs()
            .iter()
            .map(|spec| terminal_fixtures::M5ApiPublicCrate {
                name: spec.name.to_owned(),
                boundary: spec.boundary.to_owned(),
                exported_surface: spec
                    .exported_surface
                    .iter()
                    .map(|surface| (*surface).to_owned())
                    .collect(),
                docs_status: terminal_fixtures::M5CheckStatus::Pass,
                errors_status: terminal_fixtures::M5CheckStatus::Pass,
                common_traits_status: terminal_fixtures::M5CheckStatus::Pass,
                boundary_leak_status: terminal_fixtures::M5CheckStatus::Pass,
            })
            .collect(),
        boundary_checks: vec![
            terminal_fixtures::M5ApiBoundaryCheck {
                id: "api.parser_boundary".to_owned(),
                status: terminal_fixtures::M5CheckStatus::Pass,
                evidence: "terminal-core public API tests reject vte parser type exposure.".to_owned(),
            },
            terminal_fixtures::M5ApiBoundaryCheck {
                id: "api.pty_boundary".to_owned(),
                status: terminal_fixtures::M5CheckStatus::Pass,
                evidence: "terminal-core manifest stays PTY-free and terminal-pty tests reject portable-pty type exposure.".to_owned(),
            },
            terminal_fixtures::M5ApiBoundaryCheck {
                id: "api.host_renderer_boundary".to_owned(),
                status: terminal_fixtures::M5CheckStatus::Pass,
                evidence: "No public crate exposes Paneflow, GPUI, windowing or platform renderer types.".to_owned(),
            },
        ],
        semver_baseline,
        findings: vec![
            terminal_fixtures::M5ApiFinding {
                id: "api.boundary_leaks".to_owned(),
                severity: terminal_fixtures::M5FindingSeverity::P0,
                status: terminal_fixtures::M5FindingStatus::Resolved,
                summary: "No vte, portable-pty, Paneflow, GPUI or platform type leak is present across the public boundary.".to_owned(),
                m6_blocker: false,
            },
            terminal_fixtures::M5ApiFinding {
                id: "api.semver_baseline_tooling".to_owned(),
                severity: terminal_fixtures::M5FindingSeverity::P1,
                status: if semver_pass {
                    terminal_fixtures::M5FindingStatus::Resolved
                } else {
                    terminal_fixtures::M5FindingStatus::AcceptedM6Blocker
                },
                summary: "cargo-semver-checks coverage is recorded, but unavailable local tooling remains an M6 release blocker if it cannot run before publication.".to_owned(),
                m6_blocker: !semver_pass,
            },
            terminal_fixtures::M5ApiFinding {
                id: "api.milestone_prefixed_public_names".to_owned(),
                severity: terminal_fixtures::M5FindingSeverity::P2,
                status: terminal_fixtures::M5FindingStatus::AcceptedM6Blocker,
                summary: "M1/M2/M5-prefixed constants remain useful evidence names now, but should be renamed or explicitly accepted before a semver-stable release.".to_owned(),
                m6_blocker: true,
            },
        ],
        notes: vec![
            "API audit checks release boundaries rather than promising semver stability.".to_owned(),
            "cargo doc --workspace --no-deps remains the documentation render gate.".to_owned(),
        ],
    }
}

fn run_semver_baseline() -> terminal_fixtures::M5SemverBaseline {
    let command = "cargo semver-checks check-release --baseline-rev HEAD~1".to_owned();
    match ProcessCommand::new("cargo")
        .args(["semver-checks", "--version"])
        .output()
    {
        Ok(output) if output.status.success() => {
            match ProcessCommand::new("cargo")
                .args(["semver-checks", "check-release", "--baseline-rev", "HEAD~1"])
                .output()
            {
                Ok(check) if check.status.success() => terminal_fixtures::M5SemverBaseline {
                    command,
                    status: terminal_fixtures::M5ToolStatus::Pass,
                    exit_code: Some(0),
                    summary: public_output_summary(&check.stdout),
                    blocked_reason: None,
                },
                Ok(check) => terminal_fixtures::M5SemverBaseline {
                    command,
                    status: terminal_fixtures::M5ToolStatus::Failed,
                    exit_code: Some(check.status.code().unwrap_or(1)),
                    summary: public_output_summary(&check.stderr),
                    blocked_reason: None,
                },
                Err(error) => terminal_fixtures::M5SemverBaseline {
                    command,
                    status: terminal_fixtures::M5ToolStatus::Blocked,
                    exit_code: None,
                    summary: "cargo-semver-checks command could not be executed.".to_owned(),
                    blocked_reason: Some(error.to_string()),
                },
            }
        }
        Ok(output) => terminal_fixtures::M5SemverBaseline {
            command,
            status: terminal_fixtures::M5ToolStatus::Blocked,
            exit_code: None,
            summary: public_output_summary(&output.stderr),
            blocked_reason: Some(
                "cargo-semver-checks is not installed as a cargo subcommand.".to_owned(),
            ),
        },
        Err(error) => terminal_fixtures::M5SemverBaseline {
            command,
            status: terminal_fixtures::M5ToolStatus::Blocked,
            exit_code: None,
            summary: "cargo command could not be executed.".to_owned(),
            blocked_reason: Some(error.to_string()),
        },
    }
}

fn build_m5_security_baseline(
    generated_at: String,
    source_command: String,
) -> terminal_fixtures::M5SecurityBaseline {
    let tools = vec![
        run_cargo_audit_security_check(),
        run_cargo_deny_security_check(),
        run_openssf_scorecard_security_check(),
    ];
    let summary = terminal_fixtures::M5SecuritySummary {
        passed_tools: tools
            .iter()
            .filter(|tool| tool.status == terminal_fixtures::M5SecurityCheckStatus::Pass)
            .count() as u32,
        blocked_tools: tools
            .iter()
            .filter(|tool| tool.status == terminal_fixtures::M5SecurityCheckStatus::Blocked)
            .count() as u32,
        failed_tools: tools
            .iter()
            .filter(|tool| tool.status == terminal_fixtures::M5SecurityCheckStatus::Failed)
            .count() as u32,
        release_blocking_findings: tools
            .iter()
            .flat_map(|tool| tool.findings.iter())
            .filter(|finding| finding.release_blocking)
            .count() as u32,
    };
    let status = if summary.failed_tools > 0 || summary.release_blocking_findings > 0 {
        terminal_fixtures::M5SecurityBaselineStatus::Failed
    } else if summary.blocked_tools > 0 {
        terminal_fixtures::M5SecurityBaselineStatus::Blocked
    } else {
        terminal_fixtures::M5SecurityBaselineStatus::Pass
    };

    terminal_fixtures::M5SecurityBaseline {
        schema: terminal_fixtures::M5_SECURITY_BASELINE_SCHEMA.to_owned(),
        version: terminal_fixtures::M5_SECURITY_BASELINE_VERSION,
        generated_at,
        source_command,
        status,
        tools,
        summary,
        notes: vec![
            "M5 records local supply-chain tooling posture without installing tools or treating Scorecard score alone as release approval.".to_owned(),
            "Unavailable tools keep release hardening blocked until they are installed or represented by CI evidence.".to_owned(),
        ],
    }
}

fn run_cargo_audit_security_check() -> terminal_fixtures::M5SecurityToolCheck {
    run_m5_security_tool(
        terminal_fixtures::M5SecurityToolId::CargoAudit,
        "cargo-audit",
        "cargo",
        &["audit", "--version"],
        "cargo",
        &["audit", "--json"],
        &[terminal_fixtures::M5SecurityCoverageCategory::Advisories],
    )
}

fn run_cargo_deny_security_check() -> terminal_fixtures::M5SecurityToolCheck {
    run_m5_security_tool(
        terminal_fixtures::M5SecurityToolId::CargoDeny,
        "cargo-deny",
        "cargo",
        &["deny", "--version"],
        "cargo",
        &["deny", "check", "advisories", "licenses", "bans", "sources"],
        &[
            terminal_fixtures::M5SecurityCoverageCategory::Advisories,
            terminal_fixtures::M5SecurityCoverageCategory::Licenses,
            terminal_fixtures::M5SecurityCoverageCategory::Bans,
            terminal_fixtures::M5SecurityCoverageCategory::DuplicateVersions,
            terminal_fixtures::M5SecurityCoverageCategory::Sources,
        ],
    )
}

fn run_openssf_scorecard_security_check() -> terminal_fixtures::M5SecurityToolCheck {
    run_m5_security_tool(
        terminal_fixtures::M5SecurityToolId::OpenSsfScorecard,
        "openssf-scorecard",
        "scorecard",
        &["--version"],
        "scorecard",
        &["--repo=github.com/arthjean/hera-terminal", "--format=json"],
        &[terminal_fixtures::M5SecurityCoverageCategory::ScorecardChecks],
    )
}

fn run_m5_security_tool(
    id: terminal_fixtures::M5SecurityToolId,
    tool_name: &str,
    version_program: &str,
    version_args: &[&str],
    check_program: &str,
    check_args: &[&str],
    coverage: &[terminal_fixtures::M5SecurityCoverageCategory],
) -> terminal_fixtures::M5SecurityToolCheck {
    let attempted_command = command_string(check_program, check_args);
    match ProcessCommand::new(version_program)
        .args(version_args)
        .output()
    {
        Ok(version) if version.status.success() => {
            let start = Instant::now();
            match ProcessCommand::new(check_program).args(check_args).output() {
                Ok(check) if check.status.success() => terminal_fixtures::M5SecurityToolCheck {
                    id,
                    tool_name: tool_name.to_owned(),
                    install_status: terminal_fixtures::M5SecurityInstallStatus::Available,
                    attempted_command,
                    status: terminal_fixtures::M5SecurityCheckStatus::Pass,
                    exit_code: Some(0),
                    duration_ms: Some(duration_ms(start.elapsed())),
                    stdout_summary: security_output_summary(&check.stdout),
                    stderr_summary: security_output_summary(&check.stderr),
                    blocked_reason: None,
                    coverage: security_coverage_rows(
                        coverage,
                        terminal_fixtures::M5SecurityCoverageStatus::Pass,
                        "Tool completed without release-blocking findings.",
                    ),
                    findings: Vec::new(),
                    release_blocking: false,
                },
                Ok(check) => failed_security_tool(
                    id,
                    tool_name,
                    attempted_command,
                    FailedSecurityRun {
                        duration_ms: duration_ms(start.elapsed()),
                        exit_code: check.status.code().unwrap_or(1),
                        stdout_summary: security_output_summary(&check.stdout),
                        stderr_summary: security_output_summary(&check.stderr),
                    },
                    coverage,
                ),
                Err(error) => blocked_security_tool(
                    id,
                    tool_name,
                    attempted_command,
                    coverage,
                    "security check command could not be executed".to_owned(),
                    error.to_string(),
                ),
            }
        }
        Ok(version) => blocked_security_tool(
            id,
            tool_name,
            attempted_command,
            coverage,
            format!("{tool_name} is not installed or not available as a command"),
            if version.stderr.is_empty() {
                security_output_summary(&version.stdout)
            } else {
                security_output_summary(&version.stderr)
            },
        ),
        Err(error) => blocked_security_tool(
            id,
            tool_name,
            attempted_command,
            coverage,
            format!("{tool_name} version probe could not be executed"),
            error.to_string(),
        ),
    }
}

struct FailedSecurityRun {
    duration_ms: u64,
    exit_code: i32,
    stdout_summary: String,
    stderr_summary: String,
}

fn failed_security_tool(
    id: terminal_fixtures::M5SecurityToolId,
    tool_name: &str,
    attempted_command: String,
    run: FailedSecurityRun,
    coverage: &[terminal_fixtures::M5SecurityCoverageCategory],
) -> terminal_fixtures::M5SecurityToolCheck {
    let coverage_rows = security_coverage_rows_for_failure(coverage, &run.stdout_summary);
    let summary = if run.stderr_summary.trim().is_empty() {
        run.stdout_summary.clone()
    } else {
        run.stderr_summary.clone()
    };
    terminal_fixtures::M5SecurityToolCheck {
        id,
        tool_name: tool_name.to_owned(),
        install_status: terminal_fixtures::M5SecurityInstallStatus::Available,
        attempted_command,
        status: terminal_fixtures::M5SecurityCheckStatus::Failed,
        exit_code: Some(run.exit_code),
        duration_ms: Some(run.duration_ms),
        stdout_summary: run.stdout_summary,
        stderr_summary: run.stderr_summary,
        blocked_reason: None,
        coverage: coverage_rows,
        findings: vec![terminal_fixtures::M5SecurityFinding {
            id: format!("{}.failed", security_tool_slug(id)),
            severity: terminal_fixtures::M5SecurityFindingSeverity::Unknown,
            release_blocking: true,
            summary: if summary.trim().is_empty() {
                "Security tool returned a non-zero status without public output.".to_owned()
            } else {
                summary
            },
        }],
        release_blocking: true,
    }
}

fn blocked_security_tool(
    id: terminal_fixtures::M5SecurityToolId,
    tool_name: &str,
    attempted_command: String,
    coverage: &[terminal_fixtures::M5SecurityCoverageCategory],
    blocked_reason: String,
    stderr_summary: String,
) -> terminal_fixtures::M5SecurityToolCheck {
    terminal_fixtures::M5SecurityToolCheck {
        id,
        tool_name: tool_name.to_owned(),
        install_status: terminal_fixtures::M5SecurityInstallStatus::Unavailable,
        attempted_command,
        status: terminal_fixtures::M5SecurityCheckStatus::Blocked,
        exit_code: None,
        duration_ms: None,
        stdout_summary: String::new(),
        stderr_summary,
        blocked_reason: Some(blocked_reason),
        coverage: security_coverage_rows(
            coverage,
            terminal_fixtures::M5SecurityCoverageStatus::Blocked,
            "Tool unavailable in this local M5 session.",
        ),
        findings: Vec::new(),
        release_blocking: false,
    }
}

fn security_coverage_rows(
    coverage: &[terminal_fixtures::M5SecurityCoverageCategory],
    status: terminal_fixtures::M5SecurityCoverageStatus,
    summary: &str,
) -> Vec<terminal_fixtures::M5SecurityCoverage> {
    coverage
        .iter()
        .copied()
        .map(|category| terminal_fixtures::M5SecurityCoverage {
            category,
            status,
            summary: summary.to_owned(),
        })
        .collect()
}

fn security_coverage_rows_for_failure(
    coverage: &[terminal_fixtures::M5SecurityCoverageCategory],
    stdout_summary: &str,
) -> Vec<terminal_fixtures::M5SecurityCoverage> {
    coverage
        .iter()
        .copied()
        .map(|category| {
            let status = security_category_status_from_summary(category, stdout_summary)
                .unwrap_or(terminal_fixtures::M5SecurityCoverageStatus::Failed);
            let summary = match status {
                terminal_fixtures::M5SecurityCoverageStatus::Pass => {
                    "Tool reported this category as ok before the non-zero exit."
                }
                terminal_fixtures::M5SecurityCoverageStatus::Failed => {
                    "Tool reported this category as failed or did not provide per-category status."
                }
                terminal_fixtures::M5SecurityCoverageStatus::Blocked => {
                    "Tool did not execute this category."
                }
            };
            terminal_fixtures::M5SecurityCoverage {
                category,
                status,
                summary: summary.to_owned(),
            }
        })
        .collect()
}

fn security_category_status_from_summary(
    category: terminal_fixtures::M5SecurityCoverageCategory,
    stdout_summary: &str,
) -> Option<terminal_fixtures::M5SecurityCoverageStatus> {
    let lower = stdout_summary.to_ascii_lowercase();
    let token = match category {
        terminal_fixtures::M5SecurityCoverageCategory::Advisories => "advisories",
        terminal_fixtures::M5SecurityCoverageCategory::Licenses => "licenses",
        terminal_fixtures::M5SecurityCoverageCategory::Bans
        | terminal_fixtures::M5SecurityCoverageCategory::DuplicateVersions => "bans",
        terminal_fixtures::M5SecurityCoverageCategory::Sources => "sources",
        terminal_fixtures::M5SecurityCoverageCategory::ScorecardChecks => "scorecard",
    };

    if lower.contains(&format!("{token} ok")) {
        Some(terminal_fixtures::M5SecurityCoverageStatus::Pass)
    } else if lower.contains(&format!("{token} failed")) {
        Some(terminal_fixtures::M5SecurityCoverageStatus::Failed)
    } else {
        None
    }
}

fn security_tool_slug(id: terminal_fixtures::M5SecurityToolId) -> &'static str {
    match id {
        terminal_fixtures::M5SecurityToolId::CargoAudit => "cargo_audit",
        terminal_fixtures::M5SecurityToolId::CargoDeny => "cargo_deny",
        terminal_fixtures::M5SecurityToolId::OpenSsfScorecard => "openssf_scorecard",
    }
}

fn security_output_summary(bytes: &[u8]) -> String {
    let summary = public_output_summary(bytes)
        .replace("%USERPROFILE%", "$HOME")
        .replace("\\.cargo\\registry\\src", "/.cargo/registry/src");
    summary
        .chars()
        .map(|ch| {
            if ch.is_ascii() {
                ch
            } else if ch.is_whitespace() {
                ' '
            } else {
                '.'
            }
        })
        .collect()
}

fn duration_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn read_snapshot(path: &Path) -> Result<terminal_fixtures::TerminalSnapshot, String> {
    let bytes = read_bytes_capped(path, M1_MAX_SNAPSHOT_BYTES)?;
    deserialize_snapshot(&bytes).map_err(|error| format!("{}: {error}", path.display()))
}

fn read_bytes_capped(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let metadata = fs::metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!("{}: expected a regular file", path.display()));
    }
    if metadata.len() > limit as u64 {
        return Err(format!(
            "{}: file is {} bytes, maximum is {limit}",
            path.display(),
            metadata.len()
        ));
    }

    let file = fs::File::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut reader = file.take(limit as u64 + 1);
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|error| format!("{}: {error}", path.display()))?;

    if bytes.len() > limit {
        return Err(format!(
            "{}: file exceeded maximum of {limit} bytes while reading",
            path.display()
        ));
    }

    Ok(bytes)
}

fn one_path_arg(args: &[OsString]) -> Option<PathBuf> {
    (args.len() == 1).then(|| PathBuf::from(&args[0]))
}

fn collect_m4_replay_paths(path: &Path) -> Result<Vec<PathBuf>, String> {
    collect_replay_json_paths(path, "M4")
}

fn collect_replay_json_paths(path: &Path, label: &str) -> Result<Vec<PathBuf>, String> {
    let metadata = fs::metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if metadata.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }
    if !metadata.is_dir() {
        return Err(format!("{}: expected a file or directory", path.display()));
    }

    let mut paths = fs::read_dir(path)
        .map_err(|error| format!("{}: {error}", path.display()))?
        .map(|entry| entry.map_err(|error| format!("{}: {error}", path.display())))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|entry| entry.path())
        .filter(|entry_path| entry_path.extension().and_then(OsStr::to_str) == Some("json"))
        .collect::<Vec<_>>();
    paths.sort();
    if paths.is_empty() {
        return Err(format!(
            "{}: no {label} replay JSON fixtures found",
            path.display()
        ));
    }

    Ok(paths)
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let json = serialize_json(value)?;
    write_text(path, &(json + "\n"))
}

fn write_text(path: &Path, text: &str) -> Result<(), String> {
    ensure_parent(path)?;
    fs::write(path, text).map_err(|error| format!("{}: {error}", path.display()))
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

fn value_arg_path(args: &[OsString], index: usize, flag: &str) -> Result<PathBuf, String> {
    args.get(index + 1)
        .map(PathBuf::from)
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn cli_command_line(command: &str, args: &[OsString]) -> String {
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

fn m4_replay_verify_usage() -> String {
    "usage: terminal-cli verify-m4-replay <fixture-or-directory> [--json-output <path>]".to_owned()
}

fn m4_event_stream_usage() -> String {
    "usage: terminal-cli export-m4-event-stream <fixture> --output <path>".to_owned()
}

fn m5_baseline_usage() -> String {
    "usage: terminal-cli generate-m5-baseline [--output <path>] [--m4-report <path>] [--m4-status <path>] [--m4-compatibility <path>] [--m4-package-readiness <path>] [--m4-security <path>]".to_owned()
}

fn m5_replay_generate_usage() -> String {
    "usage: terminal-cli generate-m5-replay-derivatives [--output-dir <dir>]".to_owned()
}

fn m5_replay_verify_usage() -> String {
    "usage: terminal-cli verify-m5-replay <fixture-or-directory> [--json-output <path>]".to_owned()
}

fn m5_dogfood_generate_usage() -> String {
    "usage: terminal-cli generate-m5-dogfood-report --json-output <path>".to_owned()
}

fn m5_platform_measure_usage() -> String {
    "usage: terminal-cli measure-m5-platform --json-output <path>".to_owned()
}

fn m5_package_readiness_usage() -> String {
    "usage: terminal-cli generate-m5-package-readiness [--output <path>] [--release-plan-output <path>]".to_owned()
}

fn m5_api_audit_usage() -> String {
    "usage: terminal-cli generate-m5-api-audit [--output <path>]".to_owned()
}

fn m5_security_baseline_usage() -> String {
    "usage: terminal-cli generate-m5-security-baseline [--output <path>]".to_owned()
}

fn usage() -> &'static str {
    "usage: terminal-cli <inject|replay|compare|run|validate-m4-evidence|validate-m4-compatibility|generate-m5-baseline|validate-m5-baseline|validate-m5-evidence|validate-m5-compatibility|validate-m5-go-no-go|generate-m5-replay-derivatives|verify-m5-replay|generate-m5-dogfood-report|validate-m5-dogfood|measure-m5-platform|validate-m5-platform|generate-m5-package-readiness|validate-m5-package-readiness|validate-m5-release-plan|generate-m5-api-audit|validate-m5-api-audit|generate-m5-security-baseline|validate-m5-security-baseline|verify-m4-replay|export-m4-event-stream|m4-benchmark|m4-memory-profile|m4-performance-report> ...\n\nexamples:\n  terminal-cli run -- <program> [args...]\n  terminal-cli run --shell --command \"<command>\"\n  terminal-cli validate-m4-evidence evidence/m4/evidence-manifest.json\n  terminal-cli validate-m4-compatibility evidence/m4/compatibility-matrix.json\n  terminal-cli generate-m5-baseline --output evidence/m5/m5-baseline.json\n  terminal-cli validate-m5-baseline evidence/m5/m5-baseline.json\n  terminal-cli validate-m5-evidence evidence/m5/evidence-manifest.json\n  terminal-cli validate-m5-compatibility evidence/m5/compatibility-matrix.json\n  terminal-cli validate-m5-go-no-go evidence/m5/m5-go-no-go-thresholds.json\n  terminal-cli generate-m5-replay-derivatives --output-dir crates/terminal-fixtures/fixtures/m5-replay\n  terminal-cli verify-m5-replay crates/terminal-fixtures/fixtures/m5-replay --json-output evidence/m5/m5-replay-verification.json\n  terminal-cli generate-m5-dogfood-report --json-output evidence/m5/paneflow-shadow-dogfood.json\n  terminal-cli validate-m5-dogfood evidence/m5/paneflow-shadow-dogfood.json\n  terminal-cli measure-m5-platform --json-output evidence/m5/platform-runtime-evidence.json\n  terminal-cli validate-m5-platform evidence/m5/platform-runtime-evidence.json\n  terminal-cli generate-m5-package-readiness --output evidence/m5/m5-package-readiness.json --release-plan-output evidence/m5/m5-release-plan.json\n  terminal-cli validate-m5-package-readiness evidence/m5/m5-package-readiness.json\n  terminal-cli validate-m5-release-plan evidence/m5/m5-release-plan.json\n  terminal-cli generate-m5-api-audit --output evidence/m5/m5-api-audit.json\n  terminal-cli validate-m5-api-audit evidence/m5/m5-api-audit.json\n  terminal-cli generate-m5-security-baseline --output evidence/m5/m5-security-baseline.json\n  terminal-cli validate-m5-security-baseline evidence/m5/m5-security-baseline.json\n  terminal-cli verify-m4-replay crates/terminal-fixtures/fixtures/m4-replay --json-output evidence/m4/m4-replay-verification.json\n  terminal-cli export-m4-event-stream crates/terminal-fixtures/fixtures/m4-replay/basic-shell.json --output evidence/m4/replay-event-streams/basic-shell.jsonl\n  terminal-cli m4-benchmark --output evidence/m4/m4-benchmark-summary.json\n  terminal-cli m4-memory-profile --output evidence/m4/m4-memory-profile.json\n  terminal-cli m4-performance-report --bench evidence/m4/m4-benchmark-summary.json --memory evidence/m4/m4-memory-profile.json --thresholds evidence/m4/m4-performance-thresholds.json --json-output evidence/m4/m4-performance-report.json --markdown-output docs/m4-benchmarks-and-memory.md"
}

fn required_value(
    args: &[OsString],
    index: usize,
    flag: &'static str,
) -> Result<OsString, RunParseError> {
    args.get(index)
        .cloned()
        .ok_or(RunParseError::Usage(match flag {
            "--command" => "usage: terminal-cli run --shell --command <command>",
            "--input-file" => "usage: terminal-cli run --input-file <path> <command>",
            "--output" => "usage: terminal-cli run --output <path> <command>",
            "--record" => "usage: terminal-cli run --record <path> <command>",
            "--cols" => "usage: terminal-cli run --cols <columns> <command>",
            "--rows" => "usage: terminal-cli run --rows <rows> <command>",
            "--resize" => "usage: terminal-cli run --resize <columns>x<rows> <command>",
            "--timeout-ms" => "usage: terminal-cli run --timeout-ms <milliseconds> <command>",
            _ => "usage: terminal-cli run [options] <command> [args...]",
        }))
}

fn parse_dimension(value: OsString) -> Result<u16, RunParseError> {
    let raw = value.to_str().ok_or(RunParseError::Message(
        "dimension must be valid UTF-8".to_owned(),
    ))?;
    raw.parse::<u16>()
        .map_err(|error| RunParseError::Message(format!("invalid PTY dimension {raw}: {error}")))
}

fn parse_timeout_ms(value: OsString) -> Result<u64, RunParseError> {
    let raw = value.to_str().ok_or(RunParseError::Message(
        "timeout must be valid UTF-8".to_owned(),
    ))?;
    let timeout_ms = raw
        .parse::<u64>()
        .map_err(|error| RunParseError::Message(format!("invalid timeout {raw}: {error}")))?;

    if timeout_ms == 0 || timeout_ms > M2_MAX_COMMAND_TIMEOUT_MS {
        return Err(RunParseError::Message(format!(
            "timeout must be between 1 and {M2_MAX_COMMAND_TIMEOUT_MS}ms"
        )));
    }

    Ok(timeout_ms)
}

fn parse_resize(value: OsString) -> Result<PtySize, RunParseError> {
    let raw = value.to_str().ok_or(RunParseError::Message(
        "resize must be valid UTF-8".to_owned(),
    ))?;
    let Some((columns, rows)) = raw.split_once(['x', 'X']) else {
        return Err(RunParseError::Usage(
            "usage: terminal-cli run --resize <columns>x<rows> <command>",
        ));
    };
    let columns = columns
        .parse::<u16>()
        .map_err(|error| RunParseError::Message(format!("invalid resize columns: {error}")))?;
    let rows = rows
        .parse::<u16>()
        .map_err(|error| RunParseError::Message(format!("invalid resize rows: {error}")))?;
    PtySize::new(columns, rows).map_err(|error| RunParseError::Message(error.to_string()))
}

fn ensure_output_parent_exists(path: &Path) -> Result<(), RunParseError> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    if parent.as_os_str().is_empty() || parent.is_dir() {
        return Ok(());
    }

    Err(RunParseError::Message(format!(
        "output directory not found: {}",
        parent.display()
    )))
}

fn m4_manifest_repo_root(path: &Path) -> PathBuf {
    evidence_manifest_repo_root(path)
}

fn evidence_manifest_repo_root(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };

    let Some(manifest_dir) = absolute.parent() else {
        return std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    };
    let Some(evidence_dir) = manifest_dir.parent() else {
        return std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    };
    if evidence_dir.file_name().and_then(OsStr::to_str) == Some("evidence") {
        return evidence_dir
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    }

    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn os_to_string(value: impl AsRef<OsStr>) -> String {
    value.as_ref().to_string_lossy().into_owned()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShellKind {
    Cmd,
    PowerShell,
    #[cfg(unix)]
    Unix,
}

#[derive(Debug, Clone)]
struct ShellSelection {
    program: OsString,
    kind: ShellKind,
}

impl ShellSelection {
    fn default_shell() -> Result<Self, String> {
        default_shell()
    }

    fn args_for(&self, command: Option<&OsStr>) -> Vec<OsString> {
        match (self.kind, command) {
            (ShellKind::Cmd, Some(command)) => {
                vec![
                    OsString::from("/D"),
                    OsString::from("/C"),
                    command.to_owned(),
                ]
            }
            (ShellKind::Cmd, None) => vec![OsString::from("/D")],
            (ShellKind::PowerShell, Some(command)) => vec![
                OsString::from("-NoLogo"),
                OsString::from("-NoProfile"),
                OsString::from("-Command"),
                command.to_owned(),
            ],
            (ShellKind::PowerShell, None) => {
                vec![OsString::from("-NoLogo"), OsString::from("-NoProfile")]
            }
            #[cfg(unix)]
            (ShellKind::Unix, Some(command)) => vec![OsString::from("-lc"), command.to_owned()],
            #[cfg(unix)]
            (ShellKind::Unix, None) => Vec::new(),
        }
    }
}

#[cfg(windows)]
fn default_shell() -> Result<ShellSelection, String> {
    let mut candidates = Vec::new();
    if let Some(comspec) = std::env::var_os("COMSPEC").filter(|value| !value.is_empty()) {
        candidates.push(comspec);
    }
    candidates.extend([
        OsString::from("cmd.exe"),
        OsString::from("powershell.exe"),
        OsString::from("pwsh.exe"),
    ]);

    for candidate in candidates {
        if let Some(program) = find_executable(&candidate) {
            return Ok(ShellSelection {
                kind: shell_kind_for_windows(&program),
                program,
            });
        }
    }

    Err("default shell not found".to_owned())
}

#[cfg(windows)]
fn shell_kind_for_windows(program: &OsStr) -> ShellKind {
    let lower = Path::new(program)
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if lower == "pwsh" || lower == "powershell" {
        ShellKind::PowerShell
    } else {
        ShellKind::Cmd
    }
}

#[cfg(unix)]
fn default_shell() -> Result<ShellSelection, String> {
    let mut candidates = Vec::new();
    if let Some(shell) = std::env::var_os("SHELL").filter(|value| !value.is_empty()) {
        candidates.push(shell);
    }
    candidates.push(OsString::from("/bin/sh"));

    for candidate in candidates {
        if let Some(program) = find_executable(&candidate) {
            return Ok(ShellSelection {
                kind: ShellKind::Unix,
                program,
            });
        }
    }

    Err("default shell not found".to_owned())
}

#[cfg(not(any(unix, windows)))]
fn default_shell() -> Result<ShellSelection, String> {
    Err("default shell not found".to_owned())
}

fn find_executable(candidate: &OsStr) -> Option<OsString> {
    let path = Path::new(candidate);
    if path.is_absolute() || has_path_separator(candidate) {
        return path.is_file().then(|| candidate.to_owned());
    }

    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .flat_map(|dir| executable_candidates(&dir, candidate))
            .find(|path| path.is_file())
            .map(PathBuf::into_os_string)
    })
}

fn has_path_separator(candidate: &OsStr) -> bool {
    let value = candidate.to_string_lossy();
    value.contains('/') || value.contains('\\')
}

#[cfg(windows)]
fn executable_candidates(dir: &Path, candidate: &OsStr) -> Vec<PathBuf> {
    let base = dir.join(candidate);
    if Path::new(candidate).extension().is_some() {
        return vec![base];
    }

    let pathext = std::env::var_os("PATHEXT")
        .map(|value| {
            value
                .to_string_lossy()
                .split(';')
                .filter(|extension| !extension.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec![".EXE".to_owned(), ".CMD".to_owned(), ".BAT".to_owned()]);

    let mut candidates = Vec::with_capacity(pathext.len() + 1);
    candidates.push(base.clone());
    candidates.extend(pathext.into_iter().map(|extension| {
        let mut path = base.clone();
        let mut file = candidate.to_os_string();
        file.push(extension);
        path.set_file_name(file);
        path
    }));
    candidates
}

#[cfg(not(windows))]
fn executable_candidates(dir: &Path, candidate: &OsStr) -> Vec<PathBuf> {
    vec![dir.join(candidate)]
}

#[cfg(test)]
mod tests {
    use super::{RecordingState, RunOptions, TerminalQueryResponder, run};
    use std::ffi::OsString;
    use std::fs;
    use std::path::PathBuf;

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    fn temp_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        fs::create_dir_all(&path).expect("temp dir should be writable");
        path
    }

    #[test]
    fn direct_command_preserves_option_like_child_args() {
        let options = RunOptions::parse(&args(&["child", "--flag", "value"]))
            .expect("direct child args should parse");

        assert_eq!(
            options.command,
            vec![
                OsString::from("child"),
                OsString::from("--flag"),
                OsString::from("value"),
            ]
        );
    }

    #[test]
    fn terminal_query_responder_handles_split_dsr_with_core_cursor() {
        let size = terminal_pty::PtySize::new(80, 24).expect("valid size");
        let mut sink = super::CoreRunSink::new(size).expect("valid sink");
        terminal_pty::PtyEventSink::apply_output(&mut sink, b"abc").expect("output applies");
        let mut responder = TerminalQueryResponder::default();

        assert_eq!(responder.response_for_output(b"\x1b[", &sink), None);
        assert_eq!(
            responder.response_for_output(b"6n", &sink),
            Some(b"\x1b[1;4R".to_vec())
        );
    }

    #[test]
    fn recording_state_caps_output_events() {
        let mut recording = RecordingState::new(true);
        let started = std::time::Instant::now();
        let output = vec![b'x'; super::M2_CLI_MAX_RECORDING_OUTPUT_BYTES + 1];

        recording.push_pty_event(started, &terminal_pty::PtyEvent::Output(output));
        let metadata = recording.metadata();

        assert_eq!(
            metadata.output_bytes,
            super::M2_CLI_MAX_RECORDING_OUTPUT_BYTES
        );
        assert!(metadata.output_truncated);
    }

    #[test]
    fn top_level_without_args_returns_usage_success() {
        let outcome = run(args(&[]));

        assert_eq!(outcome.code, 0);
        assert!(outcome.stdout.starts_with("usage: terminal-cli"));
        assert!(outcome.stderr.is_empty());
    }

    #[test]
    fn run_without_command_returns_usage_exit_2() {
        let outcome = run(args(&["run"]));

        assert_eq!(outcome.code, 2);
        assert_eq!(outcome.stderr, "usage: terminal-cli run <command>");
    }

    #[test]
    fn run_rejects_command_option_without_shell() {
        let outcome = run(args(&["run", "--command", "echo hi"]));

        assert_eq!(outcome.code, 2);
        assert_eq!(
            outcome.stderr,
            "usage: terminal-cli run --shell --command <command>"
        );
    }

    #[test]
    fn run_preflights_missing_output_parent_before_spawn() {
        let missing = std::env::temp_dir()
            .join(format!("hera-cli-missing-{}", std::process::id()))
            .join("out.json");
        let outcome = run(vec![
            OsString::from("run"),
            OsString::from("--output"),
            missing.into_os_string(),
            OsString::from("definitely-missing-hera-command"),
        ]);

        assert_eq!(outcome.code, 1);
        assert!(outcome.stderr.contains("output directory not found"));
        assert!(!outcome.stderr.contains("spawn_command"));
    }

    #[test]
    fn run_rejects_oversized_input_file_before_spawn() {
        let dir = temp_dir("hera-cli-input-cap");
        let input = dir.join("input.bin");
        fs::write(
            &input,
            vec![b'x'; terminal_pty::M2_MAX_WRITE_CHUNK_BYTES + 1],
        )
        .expect("input fixture should be writable");

        let outcome = run(vec![
            OsString::from("run"),
            OsString::from("--input-file"),
            input.into_os_string(),
            OsString::from("definitely-missing-hera-command"),
        ]);
        let _ = fs::remove_dir_all(&dir);

        assert_eq!(outcome.code, 1);
        assert!(outcome.stderr.contains("input file is"));
        assert!(!outcome.stderr.contains("spawn_command"));
    }

    #[test]
    fn run_rejects_invalid_resize_before_spawn() {
        let outcome = run(args(&[
            "run",
            "--resize",
            "0x24",
            "definitely-missing-hera-command",
        ]));

        assert_eq!(outcome.code, 1);
        assert!(outcome.stderr.contains("invalid PTY dimensions"));
        assert!(!outcome.stderr.contains("spawn_command"));
    }

    #[test]
    fn validate_m4_evidence_checks_manifest_and_artifact_files() {
        let root = temp_dir("hera-cli-m4-evidence");
        let evidence_dir = root.join("evidence").join("m4");
        let docs_dir = root.join("docs");
        fs::create_dir_all(&evidence_dir).expect("evidence dir should be writable");
        fs::create_dir_all(&docs_dir).expect("docs dir should be writable");
        fs::write(
            docs_dir.join("m4-public-proof-report.md"),
            "public summary\n",
        )
        .expect("artifact should be writable");
        let manifest_path = evidence_dir.join("evidence-manifest.json");
        fs::write(
            &manifest_path,
            r#"{
              "schema": "hera.m4_evidence_manifest",
              "version": 1,
              "generated_at": "2026-07-04T00:00:00Z",
              "hera_commit": "d897c20",
              "redaction": {
                "version": 1,
                "updated_at": "2026-07-04T00:00:00Z",
                "reject_patterns": ["C:\\Users\\", "/home/"],
                "public_privacy_classes": ["public_summary", "scrubbed_public"]
              },
              "artifacts": [{
                "id": "report",
                "type": "report",
                "path": "docs/m4-public-proof-report.md",
                "source_command": "manual",
                "generated_at": "2026-07-04T00:00:00Z",
                "redaction_checked_at": "2026-07-04T00:00:00Z",
                "privacy": "public_summary",
                "reproducible": true
              }]
            }"#,
        )
        .expect("manifest should be writable");

        let outcome = run(vec![
            OsString::from("validate-m4-evidence"),
            manifest_path.into_os_string(),
        ]);
        let _ = fs::remove_dir_all(&root);

        assert_eq!(outcome.code, 0);
        assert!(outcome.stdout.contains("1 artifacts"));
    }

    #[test]
    fn validate_m4_compatibility_checks_matrix_and_fixture_links() {
        let root = temp_dir("hera-cli-m4-compatibility");
        let evidence_dir = root.join("evidence").join("m4");
        let fixture_dir = root
            .join("crates")
            .join("terminal-fixtures")
            .join("fixtures");
        fs::create_dir_all(&evidence_dir).expect("evidence dir should be writable");
        fs::create_dir_all(&fixture_dir).expect("fixture dir should be writable");
        fs::write(
            fixture_dir.join("m1-golden.json"),
            r#"{"fixtures":[{"name":"plain-text","terminal":{"columns":2,"rows":1},"chunks":[{"bytes":[111,107]}],"expected":{"viewport_lines":["ok"]}}]}"#,
        )
        .expect("fixture pack should be writable");
        let matrix_path = evidence_dir.join("compatibility-matrix.json");
        fs::write(
            &matrix_path,
            r#"{
              "schema": "hera.m4_compatibility_matrix",
              "version": 1,
              "generated_at": "2026-07-04T00:00:00Z",
              "rows": [{
                "id": "vt.text.plain",
                "category": "cursor_movement",
                "behavior": "Plain printable input advances the cursor.",
                "status": "implemented",
                "fixture_coverage": {
                  "status": "fixture_backed",
                  "artifacts": [{
                    "kind": "fixture",
                    "path": "crates/terminal-fixtures/fixtures/m1-golden.json",
                    "name": "plain-text"
                  }]
                },
                "source_reference": {
                  "kind": "vttest",
                  "label": "VTTEST",
                  "url": "https://invisible-island.net/vttest/"
                },
                "platform_measurements": {
                  "windows": "pass",
                  "linux": "not_measured",
                  "macos": "not_measured",
                  "notes": []
                },
                "notes": [],
                "owner": "terminal-core"
              }]
            }"#,
        )
        .expect("matrix should be writable");

        let outcome = run(vec![
            OsString::from("validate-m4-compatibility"),
            matrix_path.into_os_string(),
        ]);
        let _ = fs::remove_dir_all(&root);

        assert_eq!(outcome.code, 0);
        assert!(outcome.stdout.contains("1 rows"));
    }

    #[test]
    fn generate_m5_baseline_records_missing_sources_without_failing() {
        let dir = temp_dir("hera-cli-m5-missing-baseline");
        let output = dir.join("m5-baseline.json");

        let outcome = run(vec![
            OsString::from("generate-m5-baseline"),
            OsString::from("--output"),
            output.clone().into_os_string(),
            OsString::from("--m4-report"),
            OsString::from("target/missing-m4-report.md"),
            OsString::from("--m4-status"),
            OsString::from("target/missing-m4-status.json"),
            OsString::from("--m4-compatibility"),
            OsString::from("target/missing-m4-compatibility.json"),
            OsString::from("--m4-package-readiness"),
            OsString::from("target/missing-m4-package-readiness.json"),
            OsString::from("--m4-security"),
            OsString::from("target/missing-m4-security.json"),
        ]);
        let baseline = fs::read_to_string(&output).expect("baseline should be written");
        let _ = fs::remove_dir_all(&dir);

        assert_eq!(outcome.code, 0);
        assert!(baseline.contains(r#""m5_status": "READY""#));
        assert!(baseline.contains(r#""status": "missing""#));
        assert!(baseline.contains(r#""m5_disposition": "blocked_dependency""#));
    }

    #[test]
    fn validate_m5_contract_commands_accept_checked_in_artifacts() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        for (command, path, expected) in [
            (
                "validate-m5-baseline",
                "evidence/m5/m5-baseline.json",
                "8 blockers",
            ),
            (
                "validate-m5-evidence",
                "evidence/m5/evidence-manifest.json",
                "20 artifacts",
            ),
            (
                "validate-m5-compatibility",
                "evidence/m5/compatibility-matrix.json",
                "19 rows",
            ),
            (
                "validate-m5-go-no-go",
                "evidence/m5/m5-go-no-go-thresholds.json",
                "3 outcomes",
            ),
            (
                "validate-m5-platform",
                "evidence/m5/platform-runtime-evidence.json",
                "3 platforms",
            ),
            (
                "validate-m5-package-readiness",
                "evidence/m5/m5-package-readiness.json",
                "6 public crates",
            ),
            (
                "validate-m5-release-plan",
                "evidence/m5/m5-release-plan.json",
                "6 public crates",
            ),
            (
                "validate-m5-api-audit",
                "evidence/m5/m5-api-audit.json",
                "6 crates",
            ),
            (
                "validate-m5-security-baseline",
                "evidence/m5/m5-security-baseline.json",
                "3 tools",
            ),
        ] {
            let outcome = run(vec![
                OsString::from(command),
                root.join(path).into_os_string(),
            ]);

            assert_eq!(outcome.code, 0, "{command}: {}", outcome.stderr);
            assert!(outcome.stdout.contains(expected), "{command}");
        }
    }

    #[test]
    fn security_failure_rollup_preserves_passing_cargo_deny_categories() {
        let rows = super::security_coverage_rows_for_failure(
            &[
                terminal_fixtures::M5SecurityCoverageCategory::Advisories,
                terminal_fixtures::M5SecurityCoverageCategory::Licenses,
                terminal_fixtures::M5SecurityCoverageCategory::Bans,
                terminal_fixtures::M5SecurityCoverageCategory::DuplicateVersions,
                terminal_fixtures::M5SecurityCoverageCategory::Sources,
            ],
            "advisories ok, bans ok, licenses FAILED, sources ok",
        );

        let status = |category| {
            rows.iter()
                .find(|row| row.category == category)
                .map(|row| row.status)
        };

        assert_eq!(
            status(terminal_fixtures::M5SecurityCoverageCategory::Licenses),
            Some(terminal_fixtures::M5SecurityCoverageStatus::Failed)
        );
        for category in [
            terminal_fixtures::M5SecurityCoverageCategory::Advisories,
            terminal_fixtures::M5SecurityCoverageCategory::Bans,
            terminal_fixtures::M5SecurityCoverageCategory::DuplicateVersions,
            terminal_fixtures::M5SecurityCoverageCategory::Sources,
        ] {
            assert_eq!(
                status(category),
                Some(terminal_fixtures::M5SecurityCoverageStatus::Pass)
            );
        }
    }

    #[test]
    fn measure_m5_platform_rejects_absolute_output_before_write() {
        let dir = temp_dir("hera-cli-m5-platform-absolute");
        let output = dir.join("platform.json");

        let outcome = run(vec![
            OsString::from("measure-m5-platform"),
            OsString::from("--json-output"),
            output.clone().into_os_string(),
        ]);
        let exists = output.exists();
        let _ = fs::remove_dir_all(&dir);

        assert_eq!(outcome.code, 1);
        assert!(outcome.stderr.contains("artifact paths"));
        assert!(!exists);
    }

    #[test]
    fn inject_prints_deterministic_snapshot_json() {
        let path = std::env::temp_dir().join(format!("hera-cli-inject-{}.txt", std::process::id()));
        fs::write(&path, b"abc").expect("temp input should be writable");

        let outcome = run(vec![
            OsString::from("inject"),
            path.clone().into_os_string(),
        ]);
        let _ = fs::remove_file(&path);

        assert_eq!(outcome.code, 0);
        assert!(outcome.stdout.contains("\"columns\""));
        assert!(outcome.stdout.contains("\"ch\": \"a\""));
        assert!(outcome.stdout.contains("\"ch\": \"b\""));
        assert!(outcome.stdout.contains("\"ch\": \"c\""));
    }

    #[test]
    fn replay_reports_pass_and_assertion_failure() {
        let dir = temp_dir("hera-cli-replay");
        let pass_fixture = dir.join("pass.json");
        let fail_fixture = dir.join("fail.json");
        fs::write(
            &pass_fixture,
            r#"{"fixtures":[{"name":"ok","terminal":{"columns":2,"rows":1},"chunks":[{"bytes":[97]}],"expected":{"viewport_lines":["a"]}}]}"#,
        )
        .expect("pass fixture should be writable");
        fs::write(
            &fail_fixture,
            r#"{"fixtures":[{"name":"bad","terminal":{"columns":2,"rows":1},"chunks":[{"bytes":[97]}],"expected":{"viewport_lines":["b"]}}]}"#,
        )
        .expect("fail fixture should be writable");

        let pass = run(vec![
            OsString::from("replay"),
            pass_fixture.clone().into_os_string(),
        ]);
        let fail = run(vec![
            OsString::from("replay"),
            fail_fixture.clone().into_os_string(),
        ]);
        let _ = fs::remove_dir_all(&dir);

        assert_eq!(pass.code, 0);
        assert!(pass.stdout.contains("fixture ok: pass"));
        assert_eq!(fail.code, 1);
        assert!(fail.stderr.contains("snapshot mismatch"));
    }

    #[test]
    fn verify_m4_replay_writes_summary_for_directory() {
        let dir = temp_dir("hera-cli-m4-replay");
        let output = dir.join("summary.json");
        let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("terminal-fixtures")
            .join("fixtures/m4-replay");

        let outcome = run(vec![
            OsString::from("verify-m4-replay"),
            corpus.into_os_string(),
            OsString::from("--json-output"),
            output.clone().into_os_string(),
        ]);
        let summary = fs::read_to_string(&output).expect("summary should be writable");
        let _ = fs::remove_dir_all(&dir);

        assert_eq!(outcome.code, 0);
        assert!(outcome.stdout.contains("3 fixtures"));
        assert!(summary.contains("\"schema\": \"hera.m4_replay_verification\""));
        assert!(summary.contains("\"fixture_id\": \"basic-shell\""));
    }

    #[test]
    fn export_m4_event_stream_writes_jsonl() {
        let dir = temp_dir("hera-cli-m4-event-stream");
        let output = dir.join("basic-shell.jsonl");
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("terminal-fixtures")
            .join("fixtures/m4-replay")
            .join("basic-shell.json");

        let outcome = run(vec![
            OsString::from("export-m4-event-stream"),
            fixture.into_os_string(),
            OsString::from("--output"),
            output.clone().into_os_string(),
        ]);
        let stream = fs::read_to_string(&output).expect("stream should be writable");
        let _ = fs::remove_dir_all(&dir);

        assert_eq!(outcome.code, 0);
        assert!(
            stream
                .lines()
                .next()
                .unwrap()
                .contains("hera.m4_event_stream")
        );
        assert!(stream.contains("\"kind\":\"output\""));
        assert!(!stream.contains("raw_bytes"));
    }

    #[test]
    fn compare_reports_match_and_first_difference() {
        let dir = temp_dir("hera-cli-compare");
        let input_a = dir.join("a.bin");
        let input_b = dir.join("b.bin");
        let snapshot_a = dir.join("a.json");
        let snapshot_b = dir.join("b.json");
        fs::write(&input_a, b"abc").expect("input a should be writable");
        fs::write(&input_b, b"abd").expect("input b should be writable");

        let injected_a = run(vec![
            OsString::from("inject"),
            input_a.clone().into_os_string(),
        ]);
        let injected_b = run(vec![
            OsString::from("inject"),
            input_b.clone().into_os_string(),
        ]);
        fs::write(&snapshot_a, &injected_a.stdout).expect("snapshot a should be writable");
        fs::write(&snapshot_b, &injected_b.stdout).expect("snapshot b should be writable");

        let same = run(vec![
            OsString::from("compare"),
            snapshot_a.clone().into_os_string(),
            snapshot_a.clone().into_os_string(),
        ]);
        let different = run(vec![
            OsString::from("compare"),
            snapshot_a.into_os_string(),
            snapshot_b.into_os_string(),
        ]);
        let _ = fs::remove_dir_all(&dir);

        assert_eq!(same.code, 0);
        assert_eq!(same.stdout, "snapshots match");
        assert_eq!(different.code, 1);
        assert!(different.stderr.contains("$.viewport_rows[0].cells[2].ch"));
    }
}
