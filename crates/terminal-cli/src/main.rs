//! Debug CLI boundary for Hera M1 and M2.

#![forbid(unsafe_code)]

use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use serde::Serialize;
use terminal_core::{Terminal, TerminalError};
use terminal_fixtures::{
    FixtureRunner, M1_MAX_FIXTURE_INPUT_BYTES, M1_MAX_SNAPSHOT_BYTES,
    M2_MAX_PTY_RECORDING_OUTPUT_BYTES, PtyRecording, PtyRecordingCommandMetadata,
    PtyRecordingCommandMode, PtyRecordingEvent, PtyRecordingExitMetadata,
    PtyRecordingInputMetadata, PtyRecordingMetadata, PtyRecordingPlatformMetadata,
    PtyRecordingRuntimeMetadata, PtyRecordingSize, PtyRecordingStorageMetadata, TerminalSnapshot,
    deserialize_snapshot, first_snapshot_difference, serialize_pty_recording_pretty,
    serialize_snapshot_pretty, snapshot_terminal,
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
struct CommandOutcome {
    code: u8,
    stdout: String,
    stderr: String,
}

impl CommandOutcome {
    fn success(stdout: impl Into<String>) -> Self {
        Self {
            code: 0,
            stdout: stdout.into(),
            stderr: String::new(),
        }
    }

    fn failure(code: u8, stderr: impl Into<String>) -> Self {
        Self {
            code,
            stdout: String::new(),
            stderr: stderr.into(),
        }
    }

    fn complete(code: u8, stdout: impl Into<String>, stderr: impl Into<String>) -> Self {
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

fn usage() -> &'static str {
    "usage: terminal-cli <inject|replay|compare|run> ...\n\nexamples:\n  terminal-cli run -- <program> [args...]\n  terminal-cli run --shell --command \"<command>\""
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
