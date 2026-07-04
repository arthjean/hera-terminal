use terminal_core::{ScrollbackConfig, Terminal, TerminalConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config =
        TerminalConfig::with_scrollback(80, 24, ScrollbackConfig::new(10_000, 8 * 1024 * 1024))?;
    let mut terminal = Terminal::with_config(config);

    terminal.advance_bytes(b"cargo test\r\nrunning 1 test\r\ntest public_api_example ... ok\r\n");
    terminal.resize(100, 30)?;

    let snapshot = terminal.render_snapshot();
    let first_line = snapshot
        .viewport_rows()
        .first()
        .map(|row| row.cells().iter().map(|cell| cell.ch()).collect::<String>())
        .unwrap_or_default();

    println!(
        "{}x{} {:?} cursor={}x{} scrollback={} first_line={}",
        snapshot.columns(),
        snapshot.rows(),
        snapshot.active_screen(),
        snapshot.cursor().row() + 1,
        snapshot.cursor().column() + 1,
        snapshot.scrollback_rows().len(),
        first_line.trim_end()
    );

    Ok(())
}
