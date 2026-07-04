use terminal_core::Terminal;
use terminal_pty::{
    PortablePtyBackend, PtyCommand, PtyEvent, PtyRuntimeConfig, PtySessionConfig, PtySessionRunner,
    PtySize,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let size = PtySize::new(80, 24)?;
    let command = demo_command();
    let session = PtySessionConfig::new(command, size);
    let runtime = PtyRuntimeConfig::default();
    let backend = PortablePtyBackend::new();
    let mut terminal = Terminal::new(usize::from(size.columns()), usize::from(size.rows()))?;

    let mut runner = PtySessionRunner::spawn(&backend, &session, runtime)?;
    let outcome = runner.run_until_exit(|event| {
        if let PtyEvent::Output(bytes) = event {
            terminal.advance_bytes(&bytes);
        }
    })?;

    let snapshot = terminal.render_snapshot();
    println!(
        "exit={} output_bytes={} viewport_rows={} screen={:?}",
        outcome.exit().code(),
        outcome.output_bytes(),
        snapshot.viewport_rows().len(),
        snapshot.active_screen()
    );

    Ok(())
}

#[cfg(windows)]
fn demo_command() -> PtyCommand {
    PtyCommand::new("cmd.exe").args(["/D", "/C", "echo hera-public-api"])
}

#[cfg(not(windows))]
fn demo_command() -> PtyCommand {
    PtyCommand::new("/bin/sh").args(["-lc", "printf 'hera-public-api\\n'"])
}
