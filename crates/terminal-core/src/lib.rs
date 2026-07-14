//! Headless terminal state and correctness boundary for Hera M1.

#![forbid(unsafe_code)]

mod state;
mod vte_adapter;

pub use state::{
    Dimensions, M1_DEFAULT_COLUMNS, M1_DEFAULT_ROWS, M1_DEFAULT_SCROLLBACK_BYTES,
    M1_DEFAULT_SCROLLBACK_LINES, M1_MAX_COLUMNS, M1_MAX_ROWS, M1_MAX_SCROLLBACK_BYTES,
    M1_MAX_SCROLLBACK_LINES, M1_MAX_VIEWPORT_CELLS, RowLookupError, ScrollbackConfig,
    TerminalConfig, TerminalError,
};
pub use terminal_protocol::{
    C0Control, C0ControlKind, CsiParam, CsiSequence, DcsCommand, EscapeSequence,
    M1_PAYLOAD_LIMIT_BYTES, OscCommand, Payload, PayloadStatus, Printable, StringControl,
    StringControlKind, TerminalAction, UnsupportedSequence, UnsupportedSequenceKind,
};
pub use terminal_render_model::{
    CellStyle, Color, CursorState, DamageRegion, EMPTY_CELL_CHAR, ImagePlaceholder, ImageProtocol,
    RenderCell, RenderSnapshot, RowHandle, ScreenIdentity, ScrollbackRow, ViewportRow,
};

use state::TerminalState;
use vte_adapter::{ActionSink, VteAdapter};

/// Maximum actions retained between drains in the M1 recorder.
pub const M1_ACTION_BUFFER_LIMIT: usize = 8192;

/// Headless terminal spine: Hera-owned state plus a private parser adapter.
pub struct Terminal {
    parser: VteAdapter,
    actions: Vec<TerminalAction>,
    dropped_actions: usize,
    state: TerminalState,
}

impl Terminal {
    pub fn new(columns: usize, rows: usize) -> Result<Self, TerminalError> {
        Ok(Self::with_config(TerminalConfig::new(columns, rows)?))
    }

    #[must_use]
    pub fn with_config(config: TerminalConfig) -> Self {
        Self {
            parser: VteAdapter::new(),
            actions: Vec::new(),
            dropped_actions: 0,
            state: TerminalState::new(config),
        }
    }

    #[must_use]
    pub fn with_default_dimensions() -> Self {
        Self::with_config(TerminalConfig::default())
    }

    pub fn advance_bytes(&mut self, bytes: &[u8]) {
        let mut sink = TerminalActionSink {
            actions: &mut self.actions,
            dropped_actions: &mut self.dropped_actions,
            state: &mut self.state,
        };

        self.parser.advance(bytes, &mut sink);
    }

    #[must_use]
    pub fn actions(&self) -> &[TerminalAction] {
        &self.actions
    }

    #[must_use]
    pub fn drain_actions(&mut self) -> Vec<TerminalAction> {
        self.dropped_actions = 0;
        std::mem::take(&mut self.actions)
    }

    pub fn clear_actions(&mut self) {
        self.actions.clear();
        self.dropped_actions = 0;
    }

    #[must_use]
    pub const fn dropped_action_count(&self) -> usize {
        self.dropped_actions
    }

    #[must_use]
    pub fn dimensions(&self) -> Dimensions {
        self.state.dimensions()
    }

    #[must_use]
    pub fn active_screen(&self) -> ScreenIdentity {
        self.state.active_screen()
    }

    #[must_use]
    pub fn cursor(&self) -> CursorState {
        self.state.cursor()
    }

    pub fn render_snapshot(&mut self) -> RenderSnapshot {
        self.state.render_snapshot()
    }

    pub fn resize(&mut self, columns: usize, rows: usize) -> Result<(), TerminalError> {
        self.state.resize(columns, rows)
    }

    #[must_use]
    pub fn scrollback_rows(&self) -> Vec<ScrollbackRow> {
        self.state.scrollback_rows()
    }

    pub fn scrollback_row(&self, handle: RowHandle) -> Result<ScrollbackRow, RowLookupError> {
        self.state.scrollback_row(handle)
    }

    #[must_use]
    pub fn scrollback_len(&self) -> usize {
        self.state.scrollback_len()
    }

    #[must_use]
    pub fn scrollback_is_empty(&self) -> bool {
        self.state.scrollback_is_empty()
    }

    #[must_use]
    pub fn scrollback_byte_len(&self) -> usize {
        self.state.scrollback_byte_len()
    }

    #[cfg(test)]
    fn set_image_placeholder_for_test(
        &mut self,
        row: usize,
        column: usize,
        image: ImagePlaceholder,
    ) {
        self.state
            .set_image_placeholder_for_test(row, column, image);
    }
}

impl Default for Terminal {
    fn default() -> Self {
        Self::with_default_dimensions()
    }
}

struct TerminalActionSink<'a> {
    actions: &'a mut Vec<TerminalAction>,
    dropped_actions: &'a mut usize,
    state: &'a mut TerminalState,
}

impl TerminalActionSink<'_> {
    fn record_action(&mut self, action: TerminalAction) {
        if self.actions.len() < M1_ACTION_BUFFER_LIMIT {
            self.actions.push(action);
        } else {
            *self.dropped_actions = self.dropped_actions.saturating_add(1);
        }
    }
}

impl ActionSink for TerminalActionSink<'_> {
    fn push_action(&mut self, action: TerminalAction) {
        self.record_action(action.clone());

        for generated_action in self.state.apply_action(&action) {
            self.record_action(generated_action);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CellStyle, Color, CsiParam, CsiSequence, Dimensions, EMPTY_CELL_CHAR, ImagePlaceholder,
        ImageProtocol, M1_ACTION_BUFFER_LIMIT, M1_MAX_COLUMNS, M1_MAX_ROWS,
        M1_MAX_SCROLLBACK_BYTES, M1_MAX_SCROLLBACK_LINES, M1_MAX_VIEWPORT_CELLS,
        M1_PAYLOAD_LIMIT_BYTES, PayloadStatus, Printable, RenderCell, RowLookupError,
        ScreenIdentity, ScrollbackConfig, Terminal, TerminalAction, TerminalConfig, TerminalError,
        UnsupportedSequenceKind,
    };

    const MANIFEST: &str = include_str!("../Cargo.toml");
    const WORKSPACE_MANIFEST: &str = include_str!("../../../Cargo.toml");
    const TERMINAL_PROTOCOL_MANIFEST: &str = include_str!("../../terminal-protocol/Cargo.toml");
    const TERMINAL_RENDER_MODEL_MANIFEST: &str =
        include_str!("../../terminal-render-model/Cargo.toml");
    const TERMINAL_FIXTURES_MANIFEST: &str = include_str!("../../terminal-fixtures/Cargo.toml");
    const TERMINAL_CLI_MANIFEST: &str = include_str!("../../terminal-cli/Cargo.toml");
    const TERMINAL_PTY_MANIFEST: &str = include_str!("../../terminal-pty/Cargo.toml");
    const PUBLIC_CORE_SOURCE: &str = include_str!("lib.rs");
    const STATE_SOURCE: &str = include_str!("state.rs");
    const FORBIDDEN_BOUNDARY_DEPENDENCIES: &[&str] = &[
        "terminal-pty",
        "portable-pty",
        "gpui",
        "libc",
        "nix",
        "paneflow",
        "rustix",
        "tauri",
        "electron",
        "winapi",
        "winit",
        "windows",
        "windows-sys",
    ];
    const NON_PTY_MEMBER_MANIFESTS: &[(&str, &str, &[&str])] = &[
        ("terminal-core", MANIFEST, &[]),
        ("terminal-protocol", TERMINAL_PROTOCOL_MANIFEST, &[]),
        ("terminal-render-model", TERMINAL_RENDER_MODEL_MANIFEST, &[]),
        ("terminal-fixtures", TERMINAL_FIXTURES_MANIFEST, &[]),
        ("terminal-cli", TERMINAL_CLI_MANIFEST, &["terminal-pty"]),
    ];

    fn terminal(columns: usize, rows: usize) -> Terminal {
        Terminal::new(columns, rows).expect("test terminal dimensions must be valid")
    }

    fn viewport_text(row: &super::ViewportRow) -> String {
        row.cells().iter().map(RenderCell::ch).collect()
    }

    fn scrollback_text(row: &super::ScrollbackRow) -> String {
        row.cells().iter().map(RenderCell::ch).collect()
    }

    fn manifest_declares_dependency(manifest: &str, dependency: &str) -> bool {
        manifest.lines().any(|raw_line| {
            let line = raw_line
                .split_once('#')
                .map_or(raw_line, |(before_comment, _)| before_comment)
                .trim_start();
            let quoted_dependency = format!("\"{dependency}\"");
            let package_dependency = format!("package = \"{dependency}\"");

            line.contains(&package_dependency)
                || line
                    .strip_prefix(dependency)
                    .and_then(|rest| rest.as_bytes().first().copied())
                    .is_some_and(|byte| matches!(byte, b' ' | b'=' | b'.' | b'-'))
                || line
                    .strip_prefix(&quoted_dependency)
                    .and_then(|rest| rest.as_bytes().first().copied())
                    .is_some_and(|byte| matches!(byte, b' ' | b'=' | b'.'))
        })
    }

    #[test]
    fn manifest_has_no_platform_pty_or_renderer_dependencies() {
        for dependency in FORBIDDEN_BOUNDARY_DEPENDENCIES {
            assert!(
                !manifest_declares_dependency(MANIFEST, dependency),
                "terminal-core must keep the M1 headless boundary; forbidden dependency found: {dependency}"
            );
        }
    }

    #[test]
    fn workspace_keeps_platform_pty_dependencies_behind_terminal_pty() {
        assert!(WORKSPACE_MANIFEST.contains("\"crates/terminal-pty\""));
        assert!(manifest_declares_dependency(
            TERMINAL_PTY_MANIFEST,
            "portable-pty"
        ));

        for (crate_name, manifest, allowed_dependencies) in NON_PTY_MEMBER_MANIFESTS {
            for dependency in FORBIDDEN_BOUNDARY_DEPENDENCIES {
                if allowed_dependencies.contains(dependency) {
                    continue;
                }

                assert!(
                    !manifest_declares_dependency(manifest, dependency),
                    "{crate_name} must keep PTY, platform and product runtime dependencies behind terminal-pty; forbidden dependency found: {dependency}"
                );
            }
        }
    }

    #[test]
    fn public_core_source_does_not_expose_vte_types() {
        let parser_prefix = ["vte", "::"].concat();

        assert!(!PUBLIC_CORE_SOURCE.contains(&parser_prefix));
        assert!(!PUBLIC_CORE_SOURCE.contains(&format!("{parser_prefix}Perform")));
        assert!(!PUBLIC_CORE_SOURCE.contains(&format!("{parser_prefix}Parser")));
    }

    #[test]
    fn public_api_does_not_expose_raw_scrollback_indexes() {
        for forbidden in [
            &["pub fn scrollback_row", "_at"].concat(),
            &["pub fn row", "_at"].concat(),
            &["pub fn raw", "_row"].concat(),
            &["pub fn scrollback", "_index"].concat(),
        ] {
            assert!(!PUBLIC_CORE_SOURCE.contains(forbidden));
            assert!(!STATE_SOURCE.contains(forbidden));
        }
    }

    #[test]
    fn new_terminal_initializes_primary_and_alternate_screens() {
        let mut terminal = terminal(4, 2);

        assert_eq!(terminal.dimensions(), Dimensions::new(4, 2).unwrap());
        assert_eq!(terminal.active_screen(), ScreenIdentity::Primary);
        assert_eq!(terminal.cursor().row(), 0);
        assert_eq!(terminal.cursor().column(), 0);

        terminal.advance_bytes(b"\x1b[?1049h");
        let alternate = terminal.render_snapshot();

        assert_eq!(alternate.active_screen(), ScreenIdentity::Alternate);
        assert_eq!(alternate.cursor().row(), 0);
        assert_eq!(alternate.cursor().column(), 0);
        assert_eq!(alternate.viewport_rows()[0].cells()[0], RenderCell::empty());
    }

    #[test]
    fn invalid_dimensions_return_typed_error() {
        assert!(matches!(
            Terminal::new(0, 24),
            Err(TerminalError::InvalidDimensions { .. })
        ));
        assert!(matches!(
            Terminal::new(80, 0),
            Err(TerminalError::InvalidDimensions { .. })
        ));
        assert!(matches!(
            Terminal::new(M1_MAX_COLUMNS + 1, M1_MAX_ROWS),
            Err(TerminalError::InvalidDimensions { .. })
        ));
        assert!(matches!(
            Terminal::new(M1_MAX_COLUMNS, M1_MAX_ROWS + 1),
            Err(TerminalError::InvalidDimensions { .. })
        ));
        assert!(matches!(
            Terminal::new(M1_MAX_COLUMNS, M1_MAX_ROWS),
            Err(TerminalError::InvalidDimensions { .. })
        ));
        assert!(Dimensions::new(M1_MAX_COLUMNS, M1_MAX_VIEWPORT_CELLS / M1_MAX_COLUMNS).is_ok());
    }

    #[test]
    fn scrollback_config_is_clamped_to_m1_resource_caps() {
        let config = ScrollbackConfig::new(usize::MAX, usize::MAX);

        assert_eq!(config.max_lines(), M1_MAX_SCROLLBACK_LINES);
        assert_eq!(config.max_bytes(), M1_MAX_SCROLLBACK_BYTES);
    }

    #[test]
    fn empty_cell_snapshot_has_deterministic_defaults() {
        let mut terminal = terminal(2, 1);

        let snapshot = terminal.render_snapshot();
        let cell = &snapshot.viewport_rows()[0].cells()[0];

        assert_eq!(cell.ch(), EMPTY_CELL_CHAR);
        assert_eq!(cell.width(), 1);
        assert_eq!(cell.style(), CellStyle::default());
        assert!(cell.image().is_none());
    }

    #[test]
    fn advance_bytes_records_printable_ascii_in_order() {
        let mut terminal = terminal(4, 2);

        terminal.advance_bytes(b"abc");

        assert_eq!(
            terminal.actions(),
            &[
                TerminalAction::Print(Printable::new('a')),
                TerminalAction::Print(Printable::new('b')),
                TerminalAction::Print(Printable::new('c')),
            ]
        );
    }

    #[test]
    fn printable_ascii_mutates_primary_screen_in_row_major_order() {
        let mut terminal = terminal(3, 2);

        terminal.advance_bytes(b"abcd");
        let snapshot = terminal.render_snapshot();

        assert_eq!(snapshot.viewport_rows()[0].cells()[0].ch(), 'a');
        assert_eq!(snapshot.viewport_rows()[0].cells()[1].ch(), 'b');
        assert_eq!(snapshot.viewport_rows()[0].cells()[2].ch(), 'c');
        assert_eq!(snapshot.viewport_rows()[1].cells()[0].ch(), 'd');
        assert_eq!(snapshot.cursor().row(), 1);
        assert_eq!(snapshot.cursor().column(), 1);
    }

    #[test]
    fn wide_unicode_occupies_lead_and_spacer_cells() {
        let mut terminal = terminal(6, 1);

        terminal.advance_bytes("é漢字".as_bytes());

        let snapshot = terminal.render_snapshot();
        let cells = snapshot.viewport_rows()[0].cells();
        assert_eq!(viewport_text(&snapshot.viewport_rows()[0]), "é漢 字  ");
        assert_eq!(cells[0].width(), 1);
        assert_eq!(cells[1].width(), 2);
        assert_eq!(cells[2].width(), 0);
        assert_eq!(cells[3].width(), 2);
        assert_eq!(cells[4].width(), 0);
        assert_eq!(snapshot.cursor().column(), 5);
    }

    #[test]
    fn wide_unicode_prewraps_when_only_the_last_column_is_available() {
        let mut terminal = terminal(4, 2);

        terminal.advance_bytes("abc漢".as_bytes());

        let snapshot = terminal.render_snapshot();
        assert_eq!(viewport_text(&snapshot.viewport_rows()[0]), "abc ");
        assert!(snapshot.viewport_rows()[0].wrapped());
        assert_eq!(viewport_text(&snapshot.viewport_rows()[1]), "漢   ");
        assert_eq!(snapshot.viewport_rows()[1].cells()[0].width(), 2);
        assert_eq!(snapshot.viewport_rows()[1].cells()[1].width(), 0);
        assert_eq!(snapshot.cursor().row(), 1);
        assert_eq!(snapshot.cursor().column(), 2);
    }

    #[test]
    fn resize_keeps_wide_leads_and_spacers_on_the_same_visual_row() {
        let mut terminal = terminal(6, 2);
        terminal.advance_bytes("abcd漢".as_bytes());

        terminal.resize(5, 2).expect("resize should be valid");
        let snapshot = terminal.render_snapshot();

        assert_eq!(snapshot.scrollback_rows().len(), 1);
        assert_eq!(scrollback_text(&snapshot.scrollback_rows()[0]), "abcd ");
        assert_eq!(viewport_text(&snapshot.viewport_rows()[0]), "漢    ");
        assert_eq!(snapshot.viewport_rows()[0].cells()[0].width(), 2);
        assert_eq!(snapshot.viewport_rows()[0].cells()[1].width(), 0);
        assert_eq!(snapshot.cursor().row(), 0);
        assert_eq!(snapshot.cursor().column(), 0);
    }

    #[test]
    fn cr_lf_bs_and_tab_controls_move_cursor_by_m1_policy() {
        let mut terminal = terminal(8, 2);

        terminal.advance_bytes(b"ab\x08c\rz\tq\nx");
        let snapshot = terminal.render_snapshot();

        assert_eq!(snapshot.viewport_rows()[0].cells()[0].ch(), 'z');
        assert_eq!(snapshot.viewport_rows()[0].cells()[1].ch(), 'c');
        assert_eq!(snapshot.viewport_rows()[0].cells()[7].ch(), 'q');
        assert_eq!(snapshot.viewport_rows()[1].cells()[7].ch(), 'x');
        assert_eq!(snapshot.cursor().row(), 1);
        assert_eq!(snapshot.cursor().column(), 7);
    }

    #[test]
    fn sgr_truecolor_and_reset_mutate_printed_cell_style() {
        let mut terminal = terminal(2, 1);

        terminal.advance_bytes(b"\x1b[38:2:1:2:3mR\x1b[0mD");
        let snapshot = terminal.render_snapshot();

        assert_eq!(
            snapshot.viewport_rows()[0].cells()[0].style().foreground(),
            Some(Color::Rgb {
                red: 1,
                green: 2,
                blue: 3
            })
        );
        assert_eq!(
            snapshot.viewport_rows()[0].cells()[1].style(),
            CellStyle::default()
        );
    }

    #[test]
    fn sgr_basic_and_bright_colors_use_indexed_palette_slots() {
        let mut terminal = terminal(4, 1);

        terminal.advance_bytes(b"\x1b[34mB\x1b[91mR\x1b[42mG\x1b[104mH");
        let snapshot = terminal.render_snapshot();
        let row = &snapshot.viewport_rows()[0];

        assert_eq!(row.cells()[0].style().foreground(), Some(Color::Indexed(4)));
        assert_eq!(row.cells()[1].style().foreground(), Some(Color::Indexed(9)));
        assert_eq!(row.cells()[2].style().background(), Some(Color::Indexed(2)));
        assert_eq!(
            row.cells()[3].style().background(),
            Some(Color::Indexed(12))
        );
    }

    #[test]
    fn sgr_256_color_and_partial_resets_mutate_printed_cell_style() {
        let mut terminal = terminal(4, 1);

        terminal.advance_bytes(b"\x1b[38;5;196mF\x1b[48:5:22mB\x1b[39mR\x1b[49mD");
        let snapshot = terminal.render_snapshot();
        let row = &snapshot.viewport_rows()[0];

        assert_eq!(
            row.cells()[0].style().foreground(),
            Some(Color::Indexed(196))
        );
        assert_eq!(
            row.cells()[1].style().foreground(),
            Some(Color::Indexed(196))
        );
        assert_eq!(
            row.cells()[1].style().background(),
            Some(Color::Indexed(22))
        );
        assert_eq!(row.cells()[2].style().foreground(), None);
        assert_eq!(
            row.cells()[2].style().background(),
            Some(Color::Indexed(22))
        );
        assert_eq!(row.cells()[3].style(), CellStyle::default());
    }

    #[test]
    fn cup_and_hvp_position_printable_cells_with_one_based_coordinates() {
        let mut terminal = terminal(5, 3);

        terminal.advance_bytes(b".....\x1b[2;4HX\x1b[3;2fY");
        let snapshot = terminal.render_snapshot();

        assert_eq!(viewport_text(&snapshot.viewport_rows()[0]), ".....");
        assert_eq!(viewport_text(&snapshot.viewport_rows()[1]), "   X ");
        assert_eq!(viewport_text(&snapshot.viewport_rows()[2]), " Y   ");
        assert_eq!(snapshot.cursor().row(), 2);
        assert_eq!(snapshot.cursor().column(), 2);
    }

    #[test]
    fn csi_positioning_defaults_zero_and_clamps_without_panicking() {
        let mut terminal = terminal(4, 2);

        terminal.advance_bytes(b"abcd\x1b[HZ\x1b[;3fY\x1b[0;0HQ\x1b[99;99HW");
        let snapshot = terminal.render_snapshot();

        assert_eq!(viewport_text(&snapshot.viewport_rows()[0]), "QbYd");
        assert_eq!(viewport_text(&snapshot.viewport_rows()[1]), "   W");
        assert_eq!(snapshot.cursor().row(), 1);
        assert_eq!(snapshot.cursor().column(), 3);
    }

    #[test]
    fn csi_relative_and_axis_positioning_moves_printable_cells() {
        let mut terminal = terminal(12, 4);

        terminal.advance_bytes(b"L\x1b[5CR\x1b[2GQ\x1b[2ER\x1b[4CX\x1b[1FY\x1b[9`Z\x1b[3dV");
        let snapshot = terminal.render_snapshot();

        assert_eq!(viewport_text(&snapshot.viewport_rows()[0]), "LQ    R     ");
        assert_eq!(viewport_text(&snapshot.viewport_rows()[1]), "Y       Z   ");
        assert_eq!(viewport_text(&snapshot.viewport_rows()[2]), "R    X   V  ");
        assert_eq!(snapshot.cursor().row(), 2);
        assert_eq!(snapshot.cursor().column(), 10);
    }

    #[test]
    fn dec_private_cursor_visibility_mode_updates_snapshot_cursor() {
        let mut terminal = terminal(4, 2);

        terminal.advance_bytes(b"\x1b[?25lH");
        let hidden = terminal.render_snapshot();
        assert!(!hidden.cursor().visible());

        terminal.advance_bytes(b"\x1b[?25h");
        let visible = terminal.render_snapshot();
        assert!(visible.cursor().visible());
        assert_eq!(visible.cursor().row(), 0);
        assert_eq!(visible.cursor().column(), 1);
    }

    #[test]
    fn malformed_negative_positioning_does_not_panic_or_move_cursor() {
        let mut terminal = terminal(4, 2);

        terminal.advance_bytes(b"ab\x1b[-1;2HX");
        let snapshot = terminal.render_snapshot();

        assert_eq!(viewport_text(&snapshot.viewport_rows()[0]), "abX ");
        assert_eq!(snapshot.cursor().row(), 0);
        assert_eq!(snapshot.cursor().column(), 3);
    }

    #[test]
    fn auto_wrap_records_wrap_metadata() {
        let mut terminal = terminal(3, 2);

        terminal.advance_bytes(b"abcd");
        let snapshot = terminal.render_snapshot();

        assert!(snapshot.viewport_rows()[0].wrapped());
        assert_eq!(snapshot.viewport_rows()[1].cells()[0].ch(), 'd');
    }

    #[test]
    fn ed_below_and_above_clear_viewport_regions_without_touching_scrollback() {
        let mut erase_below = terminal(5, 3);
        erase_below.advance_bytes(b"abcde\r\nfghij\r\nklmno\x1b[2;3H\x1b[J");
        let snapshot = erase_below.render_snapshot();
        assert_eq!(viewport_text(&snapshot.viewport_rows()[0]), "abcde");
        assert_eq!(viewport_text(&snapshot.viewport_rows()[1]), "fg   ");
        assert_eq!(viewport_text(&snapshot.viewport_rows()[2]), "     ");
        assert_eq!(snapshot.cursor().row(), 1);
        assert_eq!(snapshot.cursor().column(), 2);
        assert!(snapshot.scrollback_rows().is_empty());

        let mut erase_above = terminal(5, 3);
        erase_above.advance_bytes(b"abcde\r\nfghij\r\nklmno\x1b[2;3H\x1b[1J");
        let snapshot = erase_above.render_snapshot();
        assert_eq!(viewport_text(&snapshot.viewport_rows()[0]), "     ");
        assert_eq!(viewport_text(&snapshot.viewport_rows()[1]), "   ij");
        assert_eq!(viewport_text(&snapshot.viewport_rows()[2]), "klmno");
    }

    #[test]
    fn ed_all_moves_occupied_primary_rows_into_scrollback() {
        let mut terminal = terminal(5, 3);
        terminal.advance_bytes(b"first\r\nnext\x1b[2J");

        let snapshot = terminal.render_snapshot();

        assert_eq!(snapshot.scrollback_rows().len(), 2);
        assert_eq!(scrollback_text(&snapshot.scrollback_rows()[0]), "first");
        assert_eq!(scrollback_text(&snapshot.scrollback_rows()[1]), "next ");
        assert!(
            snapshot
                .viewport_rows()
                .iter()
                .all(|row| viewport_text(row) == "     ")
        );
        assert_eq!(snapshot.cursor().row(), 1);
        assert_eq!(snapshot.cursor().column(), 4);
    }

    #[test]
    fn ed_all_moves_one_blank_primary_row_into_scrollback() {
        let mut terminal = terminal(5, 3);

        terminal.advance_bytes(b"\x1b[2J");

        let snapshot = terminal.render_snapshot();
        assert_eq!(snapshot.scrollback_rows().len(), 1);
        assert_eq!(scrollback_text(&snapshot.scrollback_rows()[0]), "     ");
    }

    #[test]
    fn ed_all_uses_the_current_background_for_the_new_viewport() {
        let mut terminal = terminal(4, 1);

        terminal.advance_bytes(b"\x1b[44m\x1b[2J");

        let snapshot = terminal.render_snapshot();
        assert_eq!(snapshot.scrollback_rows().len(), 1);
        for cell in snapshot.viewport_rows()[0].cells() {
            assert_eq!(cell.style().background(), Some(Color::Indexed(4)));
        }
    }

    #[test]
    fn ed_all_on_alternate_screen_does_not_grow_primary_scrollback() {
        let mut terminal = terminal(5, 3);
        terminal.advance_bytes(b"base\x1b[?1049hfirst\r\nnext\x1b[2J");

        let alternate = terminal.render_snapshot();

        assert_eq!(alternate.active_screen(), ScreenIdentity::Alternate);
        assert!(alternate.scrollback_rows().is_empty());
        assert_eq!(terminal.scrollback_len(), 0);
        assert!(
            alternate
                .viewport_rows()
                .iter()
                .all(|row| viewport_text(row) == "     ")
        );

        terminal.advance_bytes(b"\x1b[?1049l");
        let primary = terminal.render_snapshot();
        assert_eq!(primary.viewport_rows()[0].cells()[0].ch(), 'b');
        assert_eq!(terminal.scrollback_len(), 0);
    }

    #[test]
    fn el_modes_preserve_cursor_and_clear_only_active_row() {
        let mut erase_right = terminal(6, 2);
        erase_right.advance_bytes(b"abcdef\r\nuvwxyz\x1b[1;3H\x1b[K");
        let snapshot = erase_right.render_snapshot();
        assert_eq!(viewport_text(&snapshot.viewport_rows()[0]), "ab    ");
        assert_eq!(viewport_text(&snapshot.viewport_rows()[1]), "uvwxyz");
        assert_eq!(snapshot.cursor().row(), 0);
        assert_eq!(snapshot.cursor().column(), 2);

        let mut erase_left = terminal(6, 1);
        erase_left.advance_bytes(b"abcdef\x1b[1;3H\x1b[1K");
        let snapshot = erase_left.render_snapshot();
        assert_eq!(viewport_text(&snapshot.viewport_rows()[0]), "   def");
        assert_eq!(snapshot.cursor().column(), 2);

        let mut erase_all = terminal(6, 1);
        erase_all.advance_bytes(b"abcdef\x1b[1;3H\x1b[2K");
        let snapshot = erase_all.render_snapshot();
        assert_eq!(viewport_text(&snapshot.viewport_rows()[0]), "      ");
        assert_eq!(snapshot.cursor().column(), 2);
    }

    #[test]
    fn erase_line_uses_only_the_current_background_for_blank_cells() {
        let mut terminal = terminal(4, 1);

        terminal.advance_bytes(b"\x1b[31;47;1m\x1b[K");
        let snapshot = terminal.render_snapshot();

        for cell in snapshot.viewport_rows()[0].cells() {
            assert_eq!(cell.ch(), ' ');
            assert_eq!(cell.style().foreground(), None);
            assert_eq!(cell.style().background(), Some(Color::Indexed(7)));
            assert!(!cell.style().bold());
        }
    }

    #[test]
    fn erase_line_right_preserves_the_last_cell_while_autowrap_is_pending() {
        let mut terminal = terminal(4, 1);

        terminal.advance_bytes(b"abc]\x1b[K");
        let snapshot = terminal.render_snapshot();

        assert_eq!(viewport_text(&snapshot.viewport_rows()[0]), "abc]");
        assert_eq!(snapshot.cursor().column(), 3);
    }

    #[test]
    fn erase_through_the_last_column_clears_wrap_metadata() {
        let mut terminal = terminal(4, 2);

        terminal.advance_bytes(b"abcde\x1b[1;2H\x1b[K");
        let snapshot = terminal.render_snapshot();

        assert_eq!(viewport_text(&snapshot.viewport_rows()[0]), "a   ");
        assert!(!snapshot.viewport_rows()[0].wrapped());
    }

    #[test]
    fn ech_clears_characters_without_shifting_following_cells() {
        let mut terminal = terminal(6, 1);

        terminal.advance_bytes(b"abcdef\x1b[1;3H\x1b[2X");
        let snapshot = terminal.render_snapshot();

        assert_eq!(viewport_text(&snapshot.viewport_rows()[0]), "ab  ef");
        assert_eq!(snapshot.cursor().row(), 0);
        assert_eq!(snapshot.cursor().column(), 2);
    }

    #[test]
    fn unsupported_erasure_parameter_records_action_and_preserves_state() {
        let mut terminal = terminal(5, 1);

        terminal.advance_bytes(b"abc\x1b[99J");
        let snapshot = terminal.render_snapshot();

        assert_eq!(viewport_text(&snapshot.viewport_rows()[0]), "abc  ");
        assert!(terminal.actions().iter().any(|action| matches!(
            action,
            TerminalAction::Unsupported(sequence)
                if sequence.kind() == UnsupportedSequenceKind::Other
                    && sequence.diagnostic().contains("unsupported ED mode 99")
        )));
    }

    #[test]
    fn primary_scrolls_into_bounded_scrollback() {
        let mut terminal = terminal(3, 2);

        terminal.advance_bytes(b"a\r\nb\r\nc");

        assert_eq!(terminal.scrollback_len(), 1);
        let rows = terminal.scrollback_rows();
        assert_eq!(rows[0].cells()[0].ch(), 'a');

        let snapshot = terminal.render_snapshot();
        assert_eq!(snapshot.viewport_rows()[0].cells()[0].ch(), 'b');
        assert_eq!(snapshot.viewport_rows()[1].cells()[0].ch(), 'c');
    }

    #[test]
    fn dec_private_1049_switches_to_alternate_without_primary_scrollback_growth() {
        let mut terminal = terminal(3, 2);

        terminal.advance_bytes(b"p\r\nq\r\n\x1b[?1049ha");
        let alternate = terminal.render_snapshot();

        assert_eq!(alternate.active_screen(), ScreenIdentity::Alternate);
        assert_eq!(alternate.viewport_rows()[0].cells()[0].ch(), 'a');
        assert_eq!(terminal.scrollback_len(), 1);
        assert!(alternate.scrollback_rows().is_empty());

        terminal.advance_bytes(b"\x1b[?1049l");
        let primary = terminal.render_snapshot();

        assert_eq!(primary.active_screen(), ScreenIdentity::Primary);
        assert_eq!(primary.viewport_rows()[0].cells()[0].ch(), 'q');
        assert_eq!(primary.scrollback_rows()[0].cells()[0].ch(), 'p');
    }

    #[test]
    fn ignored_csi_does_not_switch_screen() {
        let mut terminal = terminal(3, 2);
        let ignored_1049 = TerminalAction::Csi(CsiSequence::new(
            vec![CsiParam::new([1049])],
            b"?",
            true,
            'h',
        ));

        let generated = terminal.state.apply_action(&ignored_1049);
        let snapshot = terminal.render_snapshot();

        assert_eq!(snapshot.active_screen(), ScreenIdentity::Primary);
        assert!(generated.iter().any(|action| matches!(
            action,
            TerminalAction::Unsupported(sequence)
                if sequence.kind() == UnsupportedSequenceKind::ParserIgnored
        )));
    }

    #[test]
    fn dec_private_47_switches_between_primary_and_alternate_screens() {
        let mut terminal = terminal(3, 2);

        terminal.advance_bytes(b"p\r\nq\x1b[?47ha");
        let alternate = terminal.render_snapshot();

        assert_eq!(alternate.active_screen(), ScreenIdentity::Alternate);
        assert_eq!(viewport_text(&alternate.viewport_rows()[0]), "a  ");
        assert!(alternate.scrollback_rows().is_empty());

        terminal.advance_bytes(b"\x1b[?47l");
        let primary = terminal.render_snapshot();

        assert_eq!(primary.active_screen(), ScreenIdentity::Primary);
        assert_eq!(viewport_text(&primary.viewport_rows()[0]), "p  ");
        assert_eq!(viewport_text(&primary.viewport_rows()[1]), "q  ");
    }

    #[test]
    fn dec_private_1047_returns_to_primary_and_clears_alternate_on_reset() {
        let mut terminal = terminal(3, 2);

        terminal.advance_bytes(b"p\r\nq\x1b[?1047ha\x1b[?1047l\x1b[?1047h");
        let alternate = terminal.render_snapshot();

        assert_eq!(alternate.active_screen(), ScreenIdentity::Alternate);
        assert_eq!(viewport_text(&alternate.viewport_rows()[0]), "   ");
        assert_eq!(terminal.scrollback_len(), 0);

        terminal.advance_bytes(b"\x1b[?1047l");
        let primary = terminal.render_snapshot();

        assert_eq!(primary.active_screen(), ScreenIdentity::Primary);
        assert_eq!(viewport_text(&primary.viewport_rows()[0]), "p  ");
        assert_eq!(viewport_text(&primary.viewport_rows()[1]), "q  ");
    }

    #[test]
    fn dec_private_1048_saves_and_restores_cursor_on_active_screen() {
        let mut terminal = terminal(5, 2);

        terminal.advance_bytes(b"ab\x1b[?1048h\x1b[2;5Hc\x1b[?1048lZ");
        let snapshot = terminal.render_snapshot();

        assert_eq!(snapshot.active_screen(), ScreenIdentity::Primary);
        assert_eq!(viewport_text(&snapshot.viewport_rows()[0]), "abZ  ");
        assert_eq!(viewport_text(&snapshot.viewport_rows()[1]), "    c");
        assert_eq!(snapshot.cursor().row(), 0);
        assert_eq!(snapshot.cursor().column(), 3);
    }

    #[test]
    fn dec_private_1049_restores_primary_cursor_after_alternate_screen() {
        let mut terminal = terminal(5, 2);

        terminal.advance_bytes(b"ab\x1b[?1049hxy\x1b[?1049lZ");
        let snapshot = terminal.render_snapshot();

        assert_eq!(snapshot.active_screen(), ScreenIdentity::Primary);
        assert_eq!(viewport_text(&snapshot.viewport_rows()[0]), "abZ  ");
        assert_eq!(snapshot.cursor().row(), 0);
        assert_eq!(snapshot.cursor().column(), 3);
    }

    #[test]
    fn cursor_visibility_changes_are_global_across_alternate_screen_switches() {
        let mut terminal = terminal(5, 2);

        terminal.advance_bytes(b"\x1b[?25l\x1b[?1049h\x1b[?25h\x1b[?1049l");

        let snapshot = terminal.render_snapshot();
        assert_eq!(snapshot.active_screen(), ScreenIdentity::Primary);
        assert!(snapshot.cursor().visible());
    }

    #[test]
    fn trimmed_scrollback_handle_returns_not_found() {
        let config = TerminalConfig::with_scrollback(2, 1, ScrollbackConfig::new(1, 1024)).unwrap();
        let mut terminal = Terminal::with_config(config);

        terminal.advance_bytes(b"a\r\n");
        let trimmed_handle = terminal.scrollback_rows()[0].handle();

        terminal.advance_bytes(b"b\r\nc");

        assert_eq!(terminal.scrollback_len(), 1);
        assert!(matches!(
            terminal.scrollback_row(trimmed_handle),
            Err(RowLookupError::NotFound { .. })
        ));
        assert_eq!(terminal.scrollback_rows()[0].cells()[0].ch(), 'b');
    }

    #[test]
    fn scrollback_byte_budget_stays_within_configured_budget() {
        let max_bytes = 2048;
        let config =
            TerminalConfig::with_scrollback(3, 1, ScrollbackConfig::new(10, max_bytes)).unwrap();
        let mut terminal = Terminal::with_config(config);

        terminal.advance_bytes(b"a\r\nb\r\nc\r\nd\r\ne");

        assert!(terminal.scrollback_byte_len() <= max_bytes + (max_bytes / 10));
    }

    #[test]
    fn render_snapshot_reports_and_drains_damage() {
        let mut terminal = terminal(3, 2);
        let _initial = terminal.render_snapshot();

        terminal.advance_bytes(b"x");
        let changed = terminal.render_snapshot();
        assert_eq!(changed.damage().len(), 1);
        assert_eq!(changed.damage()[0].row(), 0);

        let unchanged = terminal.render_snapshot();
        assert!(unchanged.damage().is_empty());
    }

    #[test]
    fn image_metadata_renders_as_placeholder_without_decoded_bytes() {
        let mut terminal = terminal(2, 1);
        let image = ImagePlaceholder::new(
            ImageProtocol::Kitty,
            Some("img-1".to_owned()),
            2048,
            "unsupported image payload preserved as metadata",
        );

        terminal.set_image_placeholder_for_test(0, 1, image);
        let snapshot = terminal.render_snapshot();
        let placeholder = snapshot.viewport_rows()[0].cells()[1]
            .image()
            .expect("placeholder should be present");

        assert_eq!(placeholder.protocol(), ImageProtocol::Kitty);
        assert_eq!(placeholder.id(), Some("img-1"));
        assert_eq!(placeholder.byte_len(), 2048);
        assert_eq!(
            placeholder.diagnostic(),
            "unsupported image payload preserved as metadata"
        );
    }

    #[test]
    fn primary_resize_reflows_wrapped_rows_and_updates_handle_generation() {
        let mut terminal = terminal(4, 2);

        terminal.advance_bytes(b"abcdef");
        let before = terminal.render_snapshot();
        let original_handle = before.viewport_rows()[0].handle();

        terminal.resize(3, 2).expect("resize should be valid");
        let narrow = terminal.render_snapshot();

        assert_eq!(narrow.columns(), 3);
        assert_eq!(narrow.rows(), 2);
        assert_eq!(viewport_text(&narrow.viewport_rows()[0]), "abc");
        assert_eq!(viewport_text(&narrow.viewport_rows()[1]), "def");
        assert!(narrow.viewport_rows()[0].wrapped());
        assert_eq!(narrow.cursor().row(), 1);
        assert_eq!(narrow.cursor().column(), 2);
        assert_eq!(
            narrow.viewport_rows()[0].handle().id(),
            original_handle.id()
        );
        assert!(narrow.viewport_rows()[0].handle().generation() > original_handle.generation());

        terminal.resize(6, 2).expect("resize should be valid");
        let wide = terminal.render_snapshot();

        assert_eq!(wide.columns(), 6);
        assert_eq!(wide.rows(), 2);
        assert_eq!(viewport_text(&wide.viewport_rows()[0]), "abcdef");
        assert_eq!(viewport_text(&wide.viewport_rows()[1]), "      ");
        assert!(!wide.viewport_rows()[0].wrapped());
        assert_eq!(wide.cursor().row(), 0);
        assert_eq!(wide.cursor().column(), 5);
    }

    #[test]
    fn invalid_resize_returns_error_without_mutating_state() {
        let mut terminal = terminal(3, 2);

        terminal.advance_bytes(b"abcde");
        let before = terminal.render_snapshot();
        let error = terminal.resize(0, 2).expect_err("zero columns must fail");
        let after = terminal.render_snapshot();

        assert!(matches!(
            error,
            TerminalError::InvalidDimensions {
                columns: 0,
                rows: 2,
                ..
            }
        ));
        assert_eq!(after.columns(), before.columns());
        assert_eq!(after.rows(), before.rows());
        assert_eq!(after.cursor(), before.cursor());
        assert_eq!(
            viewport_text(&after.viewport_rows()[0]),
            viewport_text(&before.viewport_rows()[0])
        );
        assert_eq!(
            viewport_text(&after.viewport_rows()[1]),
            viewport_text(&before.viewport_rows()[1])
        );
    }

    #[test]
    fn primary_resize_height_shrink_keeps_cursor_row_visible() {
        let mut terminal = terminal(3, 4);

        terminal.advance_bytes(b"ab");
        terminal.resize(3, 1).expect("resize should be valid");
        let snapshot = terminal.render_snapshot();

        assert_eq!(snapshot.rows(), 1);
        assert_eq!(viewport_text(&snapshot.viewport_rows()[0]), "ab ");
        assert_eq!(snapshot.cursor().row(), 0);
        assert_eq!(snapshot.cursor().column(), 2);
        assert!(snapshot.scrollback_rows().is_empty());
    }

    #[test]
    fn primary_resize_shrinks_height_before_reflowing_width() {
        let mut terminal = terminal(8, 6);

        terminal.advance_bytes(b"\x1b[1;8HX\x1b[2;1H");
        terminal.resize(4, 4).expect("resize should be valid");
        let snapshot = terminal.render_snapshot();

        assert_eq!(snapshot.scrollback_rows().len(), 1);
        assert_eq!(viewport_text(&snapshot.viewport_rows()[0]), "   X");
        assert_eq!(snapshot.cursor().row(), 1);
        assert_eq!(snapshot.cursor().column(), 0);
    }

    #[test]
    fn primary_resize_height_growth_pulls_rows_from_scrollback() {
        let mut terminal = terminal(3, 2);

        terminal.advance_bytes(b"a\r\nb\r\nc");
        assert_eq!(terminal.scrollback_len(), 1);

        terminal.resize(3, 3).expect("resize should be valid");
        let snapshot = terminal.render_snapshot();

        assert!(snapshot.scrollback_rows().is_empty());
        assert_eq!(viewport_text(&snapshot.viewport_rows()[0]), "a  ");
        assert_eq!(viewport_text(&snapshot.viewport_rows()[1]), "b  ");
        assert_eq!(viewport_text(&snapshot.viewport_rows()[2]), "c  ");
        assert_eq!(snapshot.cursor().row(), 2);
    }

    #[test]
    fn primary_resize_keeps_the_cursor_row_visible_when_lower_rows_reflow() {
        let mut terminal = terminal(8, 3);

        terminal.advance_bytes(b"\x1b[2;1Habcdefgh\x1b[3;1HABCDEFGH\x1b[1;1H");
        terminal.resize(4, 3).expect("resize should be valid");
        let snapshot = terminal.render_snapshot();

        assert!(snapshot.scrollback_rows().is_empty());
        assert_eq!(viewport_text(&snapshot.viewport_rows()[0]), "    ");
        assert_eq!(viewport_text(&snapshot.viewport_rows()[1]), "abcd");
        assert_eq!(viewport_text(&snapshot.viewport_rows()[2]), "efgh");
        assert_eq!(snapshot.cursor().row(), 0);
        assert_eq!(snapshot.cursor().column(), 0);
    }

    #[test]
    fn alternate_resize_does_not_pollute_primary_scrollback() {
        let mut terminal = terminal(2, 1);

        terminal.advance_bytes(b"a\r\nb\x1b[?1049hx");
        let initial_scrollback_len = terminal.scrollback_len();

        terminal.resize(4, 2).expect("resize should be valid");
        let alternate = terminal.render_snapshot();

        assert_eq!(initial_scrollback_len, 1);
        assert_eq!(terminal.scrollback_len(), initial_scrollback_len);
        assert_eq!(alternate.active_screen(), ScreenIdentity::Alternate);
        assert_eq!(alternate.columns(), 4);
        assert_eq!(alternate.rows(), 2);
        assert_eq!(viewport_text(&alternate.viewport_rows()[0]), "x   ");
        assert!(alternate.scrollback_rows().is_empty());

        terminal.advance_bytes(b"\x1b[?1049l");
        let primary = terminal.render_snapshot();

        assert_eq!(terminal.scrollback_len(), initial_scrollback_len);
        assert_eq!(primary.active_screen(), ScreenIdentity::Primary);
        assert_eq!(viewport_text(&primary.viewport_rows()[0]), "b   ");
        assert_eq!(viewport_text(&primary.viewport_rows()[1]), "    ");
        assert_eq!(scrollback_text(&primary.scrollback_rows()[0]), "a   ");
    }

    #[test]
    fn split_utf8_reaches_state_once() {
        let mut terminal = terminal(4, 2);

        terminal.advance_bytes(&[0xc3]);
        assert!(terminal.actions().is_empty());

        terminal.advance_bytes(&[0xa9]);
        assert_eq!(
            terminal.actions(),
            &[TerminalAction::Print(Printable::new('é'))]
        );

        let snapshot = terminal.render_snapshot();
        assert_eq!(snapshot.viewport_rows()[0].cells()[0].ch(), 'é');
    }

    #[test]
    fn malformed_utf8_does_not_panic_and_emits_replacement() {
        let mut terminal = terminal(4, 2);

        terminal.advance_bytes(&[0xf0, 0x28, 0x8c, 0x28]);

        assert!(terminal.actions().iter().any(|action| {
            matches!(
                action,
                TerminalAction::Print(printable) if printable.ch() == char::REPLACEMENT_CHARACTER
            )
        }));
    }

    #[test]
    fn csi_subparameters_are_preserved() {
        let mut terminal = terminal(4, 2);

        terminal.advance_bytes(b"\x1b[38:2:1:2:3m");

        let [TerminalAction::Csi(sequence)] = terminal.actions() else {
            panic!("expected one CSI action, got {:?}", terminal.actions());
        };
        assert_eq!(sequence.action(), 'm');
        assert_eq!(sequence.params()[0].subparameters(), &[38, 2, 1, 2, 3]);
    }

    #[test]
    fn dcs_payload_is_bounded() {
        let mut input = b"\x1bP1;2q".to_vec();
        input.extend(std::iter::repeat_n(b'x', M1_PAYLOAD_LIMIT_BYTES + 5));
        input.extend_from_slice(b"\x1b\\");

        let mut terminal = terminal(4, 2);
        terminal.advance_bytes(&input);

        let dcs_actions: Vec<_> = terminal
            .actions()
            .iter()
            .filter_map(|action| match action {
                TerminalAction::Dcs(command) => Some(command),
                _ => None,
            })
            .collect();

        assert_eq!(dcs_actions.len(), 1);
        let command = dcs_actions[0];
        assert_eq!(command.action(), 'q');
        assert_eq!(command.payload().bytes().len(), M1_PAYLOAD_LIMIT_BYTES);
        assert_eq!(
            command.payload().status(),
            &PayloadStatus::Truncated {
                original_len: M1_PAYLOAD_LIMIT_BYTES + 5,
                retained_len: M1_PAYLOAD_LIMIT_BYTES,
                limit: M1_PAYLOAD_LIMIT_BYTES,
            }
        );
    }

    #[test]
    fn split_dcs_payload_is_recorded_across_advance_calls() {
        let mut terminal = terminal(4, 2);

        terminal.advance_bytes(b"\x1bP1;2q");
        terminal.advance_bytes(&vec![b'x'; M1_PAYLOAD_LIMIT_BYTES + 5]);
        terminal.advance_bytes(b"\x1b\\");

        let dcs_actions: Vec<_> = terminal
            .actions()
            .iter()
            .filter_map(|action| match action {
                TerminalAction::Dcs(command) => Some(command),
                _ => None,
            })
            .collect();

        assert_eq!(dcs_actions.len(), 1);
        assert_eq!(dcs_actions[0].action(), 'q');
        assert_eq!(
            dcs_actions[0].payload().bytes().len(),
            M1_PAYLOAD_LIMIT_BYTES
        );
        assert_eq!(
            dcs_actions[0].payload().status(),
            &PayloadStatus::Truncated {
                original_len: M1_PAYLOAD_LIMIT_BYTES + 5,
                retained_len: M1_PAYLOAD_LIMIT_BYTES,
                limit: M1_PAYLOAD_LIMIT_BYTES,
            }
        );
    }

    #[test]
    fn dcs_before_escape_osc_marker_is_not_lost() {
        let mut terminal = terminal(4, 2);

        terminal.advance_bytes(b"\x1bPqabc\x1b]not-osc\x1b\\");

        let dcs_count = terminal
            .actions()
            .iter()
            .filter(|action| matches!(action, TerminalAction::Dcs(_)))
            .count();

        assert_eq!(dcs_count, 1);
    }

    #[test]
    fn split_dcs_escape_before_osc_marker_matches_unsplit_parse() {
        let mut unsplit = terminal(4, 2);
        let mut split = terminal(4, 2);

        unsplit.advance_bytes(b"\x1bPqabc\x1b]not-osc\x1b\\");
        split.advance_bytes(b"\x1bPqabc\x1b");
        split.advance_bytes(b"]not-osc\x1b\\");

        assert_eq!(split.actions(), unsplit.actions());
    }

    #[test]
    fn osc_payload_is_bounded_and_st_terminator_is_private() {
        let prefix = b"1337;";
        let mut input = b"\x1b]".to_vec();
        input.extend_from_slice(prefix);
        input.extend(std::iter::repeat_n(b'x', M1_PAYLOAD_LIMIT_BYTES + 5));
        input.extend_from_slice(b"\x1b\\");

        let mut terminal = terminal(4, 2);
        terminal.advance_bytes(&input);

        let [TerminalAction::Osc(command)] = terminal.actions() else {
            panic!("expected one OSC action, got {:?}", terminal.actions());
        };
        assert_eq!(command.payload().bytes().len(), M1_PAYLOAD_LIMIT_BYTES);
        assert_eq!(
            command.payload().status(),
            &PayloadStatus::Truncated {
                original_len: prefix.len() + M1_PAYLOAD_LIMIT_BYTES + 5,
                retained_len: M1_PAYLOAD_LIMIT_BYTES,
                limit: M1_PAYLOAD_LIMIT_BYTES,
            }
        );
    }

    #[test]
    fn action_buffer_is_bounded_without_limiting_state_mutation() {
        let mut terminal = terminal(80, 24);
        let input = vec![b'x'; M1_ACTION_BUFFER_LIMIT + 3];

        terminal.advance_bytes(&input);

        assert_eq!(terminal.actions().len(), M1_ACTION_BUFFER_LIMIT);
        assert_eq!(terminal.dropped_action_count(), 3);

        let snapshot = terminal.render_snapshot();
        assert_eq!(snapshot.viewport_rows()[6].cells()[36].ch(), 'x');
    }

    #[test]
    fn apc_and_pm_payloads_are_recorded_without_vte_leakage() {
        let mut terminal = terminal(4, 2);

        terminal.advance_bytes(b"\x1b_payload\x1b\\\x1b^private\x1b\\");

        let [TerminalAction::Apc(apc), TerminalAction::Pm(pm)] = terminal.actions() else {
            panic!("expected APC and PM actions, got {:?}", terminal.actions());
        };
        assert_eq!(apc.payload().bytes(), b"payload");
        assert_eq!(pm.payload().bytes(), b"private");
    }
}
