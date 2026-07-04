//! Debug CLI boundary for Hera M1 and M2.

#![forbid(unsafe_code)]

mod m4_performance_cli;

use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use terminal_core::{Terminal, TerminalError};
use terminal_fixtures::{
    FixtureRunner, M1_MAX_FIXTURE_INPUT_BYTES, M1_MAX_SNAPSHOT_BYTES,
    M2_MAX_PTY_RECORDING_OUTPUT_BYTES, M4_PUBLIC_REPLAY_VERSION, M4_REPLAY_VERIFICATION_SCHEMA,
    M4_REPLAY_VERIFICATION_VERSION, M4CompatibilityMatrix, M4EvidenceManifest,
    M4PublicReplayFixture, M4ReplayVerificationStatus, M4ReplayVerificationSummary, PtyRecording,
    PtyRecordingCommandMetadata, PtyRecordingCommandMode, PtyRecordingEvent,
    PtyRecordingExitMetadata, PtyRecordingInputMetadata, PtyRecordingMetadata,
    PtyRecordingPlatformMetadata, PtyRecordingRuntimeMetadata, PtyRecordingSize,
    PtyRecordingStorageMetadata, TerminalSnapshot, deserialize_snapshot, first_snapshot_difference,
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
            "{}: no M4 replay JSON fixtures found",
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

fn usage() -> &'static str {
    "usage: terminal-cli <inject|replay|compare|run|validate-m4-evidence|validate-m4-compatibility|verify-m4-replay|export-m4-event-stream|m4-benchmark|m4-memory-profile|m4-performance-report> ...\n\nexamples:\n  terminal-cli run -- <program> [args...]\n  terminal-cli run --shell --command \"<command>\"\n  terminal-cli validate-m4-evidence evidence/m4/evidence-manifest.json\n  terminal-cli validate-m4-compatibility evidence/m4/compatibility-matrix.json\n  terminal-cli verify-m4-replay crates/terminal-fixtures/fixtures/m4-replay --json-output evidence/m4/m4-replay-verification.json\n  terminal-cli export-m4-event-stream crates/terminal-fixtures/fixtures/m4-replay/basic-shell.json --output evidence/m4/replay-event-streams/basic-shell.jsonl\n  terminal-cli m4-benchmark --output evidence/m4/m4-benchmark-summary.json\n  terminal-cli m4-memory-profile --output evidence/m4/m4-memory-profile.json\n  terminal-cli m4-performance-report --bench evidence/m4/m4-benchmark-summary.json --memory evidence/m4/m4-memory-profile.json --thresholds evidence/m4/m4-performance-thresholds.json --json-output evidence/m4/m4-performance-report.json --markdown-output docs/m4-benchmarks-and-memory.md"
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
    if manifest_dir.file_name().and_then(OsStr::to_str) == Some("m4")
        && evidence_dir.file_name().and_then(OsStr::to_str) == Some("evidence")
    {
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
