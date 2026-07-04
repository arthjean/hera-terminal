use std::collections::{BTreeSet, VecDeque};
use std::{fmt, mem};

use terminal_protocol::{
    C0ControlKind, CsiSequence, Printable, TerminalAction, UnsupportedSequence,
    UnsupportedSequenceKind,
};
use terminal_render_model::{
    CellStyle, Color, CursorState, DamageRegion, RenderCell, RenderSnapshot, RowHandle,
    ScreenIdentity, ScrollbackRow, ViewportRow,
};

#[cfg(test)]
use terminal_render_model::ImagePlaceholder;

pub const M1_DEFAULT_COLUMNS: usize = 80;
pub const M1_DEFAULT_ROWS: usize = 24;
pub const M1_MAX_COLUMNS: usize = 4096;
pub const M1_MAX_ROWS: usize = 4096;
pub const M1_MAX_VIEWPORT_CELLS: usize = 1_048_576;
pub const M1_DEFAULT_SCROLLBACK_LINES: usize = 10_000;
pub const M1_DEFAULT_SCROLLBACK_BYTES: usize = 8 * 1024 * 1024;
pub const M1_MAX_SCROLLBACK_LINES: usize = 1_000_000;
pub const M1_MAX_SCROLLBACK_BYTES: usize = 512 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dimensions {
    columns: usize,
    rows: usize,
}

impl Dimensions {
    pub fn new(columns: usize, rows: usize) -> Result<Self, TerminalError> {
        let cells = columns.checked_mul(rows);

        if columns == 0
            || rows == 0
            || columns > M1_MAX_COLUMNS
            || rows > M1_MAX_ROWS
            || cells.is_none_or(|cells| cells > M1_MAX_VIEWPORT_CELLS)
        {
            return Err(TerminalError::InvalidDimensions {
                columns,
                rows,
                max_columns: M1_MAX_COLUMNS,
                max_rows: M1_MAX_ROWS,
                max_cells: M1_MAX_VIEWPORT_CELLS,
            });
        }

        Ok(Self { columns, rows })
    }

    pub(crate) const fn unchecked(columns: usize, rows: usize) -> Self {
        Self { columns, rows }
    }

    #[must_use]
    pub const fn columns(self) -> usize {
        self.columns
    }

    #[must_use]
    pub const fn rows(self) -> usize {
        self.rows
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollbackConfig {
    max_lines: usize,
    max_bytes: usize,
}

impl ScrollbackConfig {
    #[must_use]
    pub const fn new(max_lines: usize, max_bytes: usize) -> Self {
        Self {
            max_lines: if max_lines > M1_MAX_SCROLLBACK_LINES {
                M1_MAX_SCROLLBACK_LINES
            } else {
                max_lines
            },
            max_bytes: if max_bytes > M1_MAX_SCROLLBACK_BYTES {
                M1_MAX_SCROLLBACK_BYTES
            } else {
                max_bytes
            },
        }
    }

    #[must_use]
    pub const fn max_lines(self) -> usize {
        self.max_lines
    }

    #[must_use]
    pub const fn max_bytes(self) -> usize {
        self.max_bytes
    }
}

impl Default for ScrollbackConfig {
    fn default() -> Self {
        Self::new(M1_DEFAULT_SCROLLBACK_LINES, M1_DEFAULT_SCROLLBACK_BYTES)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalConfig {
    dimensions: Dimensions,
    scrollback: ScrollbackConfig,
}

impl TerminalConfig {
    pub fn new(columns: usize, rows: usize) -> Result<Self, TerminalError> {
        Ok(Self {
            dimensions: Dimensions::new(columns, rows)?,
            scrollback: ScrollbackConfig::default(),
        })
    }

    pub fn with_scrollback(
        columns: usize,
        rows: usize,
        scrollback: ScrollbackConfig,
    ) -> Result<Self, TerminalError> {
        Ok(Self {
            dimensions: Dimensions::new(columns, rows)?,
            scrollback,
        })
    }

    #[must_use]
    pub const fn dimensions(self) -> Dimensions {
        self.dimensions
    }

    #[must_use]
    pub const fn scrollback(self) -> ScrollbackConfig {
        self.scrollback
    }
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            dimensions: Dimensions::unchecked(M1_DEFAULT_COLUMNS, M1_DEFAULT_ROWS),
            scrollback: ScrollbackConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalError {
    InvalidDimensions {
        columns: usize,
        rows: usize,
        max_columns: usize,
        max_rows: usize,
        max_cells: usize,
    },
}

impl fmt::Display for TerminalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDimensions {
                columns,
                rows,
                max_columns,
                max_rows,
                max_cells,
            } => write!(
                formatter,
                "invalid terminal dimensions: {columns} columns, {rows} rows; maximum is {max_columns} columns, {max_rows} rows and {max_cells} cells"
            ),
        }
    }
}

impl std::error::Error for TerminalError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowLookupError {
    NotFound { handle: RowHandle },
}

impl fmt::Display for RowLookupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { handle } => write!(
                formatter,
                "row handle not found: id={}, generation={}",
                handle.id(),
                handle.generation()
            ),
        }
    }
}

impl std::error::Error for RowLookupError {}

pub(crate) struct TerminalState {
    config: TerminalConfig,
    primary: Screen,
    alternate: Screen,
    active_screen: ScreenIdentity,
    scrollback: Scrollback,
    next_row_id: u64,
    damage: DamageTracker,
    style: CellStyle,
}

impl TerminalState {
    pub(crate) fn new(config: TerminalConfig) -> Self {
        let mut next_row_id = 1;
        let primary = Screen::new(config.dimensions, &mut next_row_id);
        let alternate = Screen::new(config.dimensions, &mut next_row_id);

        Self {
            config,
            primary,
            alternate,
            active_screen: ScreenIdentity::Primary,
            scrollback: Scrollback::new(config.scrollback),
            next_row_id,
            damage: DamageTracker::dirty_all(config.dimensions.rows()),
            style: CellStyle::default(),
        }
    }

    pub(crate) fn dimensions(&self) -> Dimensions {
        self.config.dimensions
    }

    pub(crate) fn active_screen(&self) -> ScreenIdentity {
        self.active_screen
    }

    pub(crate) fn cursor(&self) -> CursorState {
        self.active_screen_ref().cursor
    }

    pub(crate) fn apply_action(&mut self, action: &TerminalAction) -> Vec<TerminalAction> {
        match action {
            TerminalAction::Print(printable) => self.print(*printable),
            TerminalAction::Control(control) => self.apply_control(control.kind()),
            TerminalAction::Csi(sequence) => return self.apply_csi(sequence),
            _ => {}
        }

        Vec::new()
    }

    pub(crate) fn render_snapshot(&mut self) -> RenderSnapshot {
        let damage = self.damage.take(self.config.dimensions.columns());
        self.snapshot_with_damage(damage)
    }

    pub(crate) fn resize(&mut self, columns: usize, rows: usize) -> Result<(), TerminalError> {
        let dimensions = Dimensions::new(columns, rows)?;
        if dimensions == self.config.dimensions {
            return Ok(());
        }

        match self.active_screen {
            ScreenIdentity::Primary => self.resize_active_primary(dimensions),
            ScreenIdentity::Alternate => self.resize_while_alternate_active(dimensions),
        }

        self.config = TerminalConfig {
            dimensions,
            scrollback: self.config.scrollback,
        };
        self.damage = DamageTracker::dirty_all(dimensions.rows());
        Ok(())
    }

    pub(crate) fn scrollback_rows(&self) -> Vec<ScrollbackRow> {
        self.scrollback.rows()
    }

    pub(crate) fn scrollback_row(
        &self,
        handle: RowHandle,
    ) -> Result<ScrollbackRow, RowLookupError> {
        self.scrollback
            .row(handle)
            .ok_or(RowLookupError::NotFound { handle })
    }

    pub(crate) fn scrollback_len(&self) -> usize {
        self.scrollback.len()
    }

    pub(crate) fn scrollback_is_empty(&self) -> bool {
        self.scrollback.is_empty()
    }

    pub(crate) fn scrollback_byte_len(&self) -> usize {
        self.scrollback.byte_len()
    }

    #[cfg(test)]
    pub(crate) fn set_image_placeholder_for_test(
        &mut self,
        row: usize,
        column: usize,
        image: ImagePlaceholder,
    ) {
        if row >= self.config.dimensions.rows() || column >= self.config.dimensions.columns() {
            return;
        }

        self.active_screen_mut().rows[row].cells[column] =
            RenderCell::image_placeholder(CellStyle::default(), image);
        self.damage.mark(row);
    }

    fn snapshot_with_damage(&self, damage: Vec<DamageRegion>) -> RenderSnapshot {
        let active = self.active_screen_ref();
        let scrollback = if self.active_screen == ScreenIdentity::Primary {
            self.scrollback.rows()
        } else {
            Vec::new()
        };

        RenderSnapshot::new(
            self.config.dimensions.columns(),
            self.config.dimensions.rows(),
            self.active_screen,
            active.cursor,
            active.viewport_rows(),
            scrollback,
            damage,
        )
    }

    fn print(&mut self, printable: Printable) {
        if self.active_screen_ref().pending_wrap {
            self.wrap_pending();
        }

        let ch = printable.ch();
        let width = printable_width(ch);
        let columns = self.config.dimensions.columns();
        let row = self.active_screen_ref().cursor.row();
        let column = self.active_screen_ref().cursor.column();

        {
            let style = self.style;
            let screen = self.active_screen_mut();
            screen.rows[row].cells[column] = RenderCell::text(ch, width, style);

            if column + usize::from(width) >= columns {
                screen.pending_wrap = true;
                screen.set_cursor(row, columns - 1);
            } else {
                screen.set_cursor(row, column + usize::from(width));
            }
        }

        self.damage.mark(row);
    }

    fn apply_control(&mut self, kind: C0ControlKind) {
        match kind {
            C0ControlKind::Backspace => self.backspace(),
            C0ControlKind::HorizontalTab => self.horizontal_tab(),
            C0ControlKind::LineFeed | C0ControlKind::VerticalTab | C0ControlKind::FormFeed => {
                self.line_feed();
            }
            C0ControlKind::CarriageReturn => self.carriage_return(),
            _ => {}
        }
    }

    fn apply_csi(&mut self, sequence: &CsiSequence) -> Vec<TerminalAction> {
        if sequence.ignored() {
            return vec![TerminalAction::Unsupported(UnsupportedSequence::new(
                UnsupportedSequenceKind::ParserIgnored,
                format!("ignored CSI sequence with final byte {}", sequence.action()),
            ))];
        }

        if sequence.intermediates().is_empty() && sequence.action() == 'm' {
            self.apply_sgr(sequence);
            return Vec::new();
        }

        if sequence.intermediates() != b"?" || !matches!(sequence.action(), 'h' | 'l') {
            return Vec::new();
        }

        let enabled = sequence.action() == 'h';
        let mut generated = Vec::new();

        for param in sequence.params() {
            let Some(mode) = param.subparameters().first().copied() else {
                continue;
            };

            match mode {
                1049 => self.set_alternate_screen(enabled),
                47 | 1047 | 1048 => {
                    generated.push(TerminalAction::Unsupported(UnsupportedSequence::new(
                        UnsupportedSequenceKind::Other,
                        format!(
                            "unsupported alternate-screen private mode {mode}{}",
                            sequence.action()
                        ),
                    )));
                }
                _ => {}
            }
        }

        generated
    }

    fn backspace(&mut self) {
        let row = self.active_screen_ref().cursor.row();
        let column = self.active_screen_ref().cursor.column();
        let next_column = column.saturating_sub(1);

        {
            let screen = self.active_screen_mut();
            screen.pending_wrap = false;
            screen.set_cursor(row, next_column);
        }

        self.damage.mark(row);
    }

    fn apply_sgr(&mut self, sequence: &CsiSequence) {
        let params = sequence
            .params()
            .iter()
            .flat_map(|param| param.subparameters().iter().copied())
            .collect::<Vec<_>>();
        let params = if params.is_empty() { vec![0] } else { params };

        let mut index = 0;
        let mut foreground = self.style.foreground();
        let mut background = self.style.background();
        let mut bold = self.style.bold();
        let mut italic = self.style.italic();
        let mut underline = self.style.underline();
        let mut inverse = self.style.inverse();

        while index < params.len() {
            match params[index] {
                0 => {
                    foreground = None;
                    background = None;
                    bold = false;
                    italic = false;
                    underline = false;
                    inverse = false;
                    index += 1;
                }
                1 => {
                    bold = true;
                    index += 1;
                }
                3 => {
                    italic = true;
                    index += 1;
                }
                4 => {
                    underline = true;
                    index += 1;
                }
                7 => {
                    inverse = true;
                    index += 1;
                }
                22 => {
                    bold = false;
                    index += 1;
                }
                23 => {
                    italic = false;
                    index += 1;
                }
                24 => {
                    underline = false;
                    index += 1;
                }
                27 => {
                    inverse = false;
                    index += 1;
                }
                38 | 48 if params.get(index + 1) == Some(&2) && index + 4 < params.len() => {
                    let color = Color::Rgb {
                        red: sgr_u8(params[index + 2]),
                        green: sgr_u8(params[index + 3]),
                        blue: sgr_u8(params[index + 4]),
                    };
                    if params[index] == 38 {
                        foreground = Some(color);
                    } else {
                        background = Some(color);
                    }
                    index += 5;
                }
                39 => {
                    foreground = None;
                    index += 1;
                }
                49 => {
                    background = None;
                    index += 1;
                }
                _ => {
                    index += 1;
                }
            }
        }

        self.style = CellStyle::new(foreground, background, bold, italic, underline, inverse);
    }

    fn horizontal_tab(&mut self) {
        let columns = self.config.dimensions.columns();
        let row = self.active_screen_ref().cursor.row();
        let column = self.active_screen_ref().cursor.column();
        let next_stop = ((column / 8) + 1) * 8;
        let next_column = next_stop.min(columns - 1);

        {
            let screen = self.active_screen_mut();
            screen.pending_wrap = false;
            screen.set_cursor(row, next_column);
        }

        self.damage.mark(row);
    }

    fn carriage_return(&mut self) {
        let row = self.active_screen_ref().cursor.row();

        {
            let screen = self.active_screen_mut();
            screen.pending_wrap = false;
            screen.set_cursor(row, 0);
        }

        self.damage.mark(row);
    }

    fn line_feed(&mut self) {
        self.active_screen_mut().pending_wrap = false;
        self.move_down_or_scroll();
    }

    fn wrap_pending(&mut self) {
        let row = self.active_screen_ref().cursor.row();

        {
            let screen = self.active_screen_mut();
            screen.rows[row].wrapped = true;
            screen.pending_wrap = false;
        }

        self.damage.mark(row);
        self.move_down_or_scroll();

        let next_row = self.active_screen_ref().cursor.row();
        self.active_screen_mut().set_cursor(next_row, 0);
    }

    fn move_down_or_scroll(&mut self) {
        let rows = self.config.dimensions.rows();
        let current_row = self.active_screen_ref().cursor.row();
        let current_column = self.active_screen_ref().cursor.column();

        if current_row + 1 < rows {
            self.active_screen_mut()
                .set_cursor(current_row + 1, current_column);
            self.damage.mark(current_row);
            self.damage.mark(current_row + 1);
        } else {
            self.scroll_active_screen_up();
            self.active_screen_mut()
                .set_cursor(rows - 1, current_column);
        }
    }

    fn scroll_active_screen_up(&mut self) {
        let new_row = self.next_blank_row();
        let removed = match self.active_screen {
            ScreenIdentity::Primary => self.primary.scroll_up(new_row),
            ScreenIdentity::Alternate => self.alternate.scroll_up(new_row),
        };

        if self.active_screen == ScreenIdentity::Primary {
            self.scrollback.push(removed);
        }

        self.damage.mark_all(self.config.dimensions.rows());
    }

    fn set_alternate_screen(&mut self, enabled: bool) {
        if enabled {
            self.alternate
                .reset(self.config.dimensions, &mut self.next_row_id);
            self.active_screen = ScreenIdentity::Alternate;
        } else {
            self.active_screen = ScreenIdentity::Primary;
        }

        self.damage.mark_all(self.config.dimensions.rows());
    }

    fn resize_active_primary(&mut self, dimensions: Dimensions) {
        let scrollback_len = self.scrollback.len();
        let cursor_global_row = scrollback_len.saturating_add(self.primary.cursor.row());
        let cursor_column = self.primary.cursor.column();
        let cursor_pending_wrap = self.primary.pending_wrap;
        let source_rows = self
            .scrollback
            .iter()
            .cloned()
            .chain(self.primary.rows.iter().cloned())
            .collect::<Vec<_>>();

        let reflowed = reflow_rows(
            source_rows,
            cursor_global_row,
            cursor_column,
            cursor_pending_wrap,
            dimensions.columns(),
            &mut self.next_row_id,
        );
        let split_at =
            visible_start_for_cursor(reflowed.rows.len(), dimensions.rows(), reflowed.cursor_row);
        let cursor_row = reflowed
            .cursor_row
            .saturating_sub(split_at)
            .min(dimensions.rows() - 1);

        let mut visible_rows = reflowed
            .rows
            .iter()
            .skip(split_at)
            .take(dimensions.rows())
            .cloned()
            .collect::<VecDeque<_>>();
        while visible_rows.len() < dimensions.rows() {
            visible_rows.push_back(blank_row(dimensions.columns(), &mut self.next_row_id));
        }

        let scrollback_rows = reflowed
            .rows
            .into_iter()
            .take(split_at)
            .collect::<VecDeque<_>>();

        self.scrollback.replace_rows(scrollback_rows);
        self.primary = Screen::from_rows(
            visible_rows,
            CursorState::new(cursor_row, reflowed.cursor_column, true),
            reflowed.pending_wrap,
        );
    }

    fn resize_while_alternate_active(&mut self, dimensions: Dimensions) {
        self.primary
            .resize_visible(dimensions, &mut self.next_row_id);
        self.scrollback.resize_width(dimensions.columns());
        self.alternate
            .resize_visible(dimensions, &mut self.next_row_id);
    }

    fn next_blank_row(&mut self) -> Row {
        blank_row(self.config.dimensions.columns(), &mut self.next_row_id)
    }

    fn active_screen_ref(&self) -> &Screen {
        match self.active_screen {
            ScreenIdentity::Primary => &self.primary,
            ScreenIdentity::Alternate => &self.alternate,
        }
    }

    fn active_screen_mut(&mut self) -> &mut Screen {
        match self.active_screen {
            ScreenIdentity::Primary => &mut self.primary,
            ScreenIdentity::Alternate => &mut self.alternate,
        }
    }
}

struct Screen {
    rows: VecDeque<Row>,
    cursor: CursorState,
    pending_wrap: bool,
}

impl Screen {
    fn new(dimensions: Dimensions, next_row_id: &mut u64) -> Self {
        let rows = (0..dimensions.rows())
            .map(|_| {
                let handle = RowHandle::new(*next_row_id, 0);
                *next_row_id = next_row_id.saturating_add(1);
                Row::blank(dimensions.columns(), handle)
            })
            .collect();

        Self {
            rows,
            cursor: CursorState::new(0, 0, true),
            pending_wrap: false,
        }
    }

    fn reset(&mut self, dimensions: Dimensions, next_row_id: &mut u64) {
        *self = Self::new(dimensions, next_row_id);
    }

    fn from_rows(rows: VecDeque<Row>, cursor: CursorState, pending_wrap: bool) -> Self {
        Self {
            rows,
            cursor,
            pending_wrap,
        }
    }

    fn resize_visible(&mut self, dimensions: Dimensions, next_row_id: &mut u64) {
        for row in &mut self.rows {
            row.resize_width(dimensions.columns());
        }

        while self.rows.len() > dimensions.rows() {
            self.rows.pop_back();
        }

        while self.rows.len() < dimensions.rows() {
            self.rows
                .push_back(blank_row(dimensions.columns(), next_row_id));
        }

        self.cursor = CursorState::new(
            self.cursor.row().min(dimensions.rows() - 1),
            self.cursor.column().min(dimensions.columns() - 1),
            self.cursor.visible(),
        );
        self.pending_wrap = false;
    }

    fn set_cursor(&mut self, row: usize, column: usize) {
        self.cursor = CursorState::new(row, column, self.cursor.visible());
    }

    fn scroll_up(&mut self, new_bottom_row: Row) -> Row {
        let removed = match self.rows.pop_front() {
            Some(row) => row,
            None => new_bottom_row.clone(),
        };
        self.rows.push_back(new_bottom_row);
        removed
    }

    fn viewport_rows(&self) -> Vec<ViewportRow> {
        self.rows.iter().map(Row::to_viewport_row).collect()
    }
}

#[derive(Clone)]
struct Row {
    handle: RowHandle,
    cells: Vec<RenderCell>,
    wrapped: bool,
}

impl Row {
    fn blank(columns: usize, handle: RowHandle) -> Self {
        Self {
            handle,
            cells: vec![RenderCell::empty(); columns],
            wrapped: false,
        }
    }

    fn to_viewport_row(&self) -> ViewportRow {
        ViewportRow::new(self.handle, self.cells.clone(), self.wrapped)
    }

    fn to_scrollback_row(&self) -> ScrollbackRow {
        ScrollbackRow::new(self.handle, self.cells.clone(), self.wrapped)
    }

    fn resize_width(&mut self, columns: usize) {
        self.handle = next_generation(self.handle);
        self.cells.resize(columns, RenderCell::empty());
        self.wrapped = false;
    }

    fn estimated_bytes(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(
                self.cells
                    .capacity()
                    .saturating_mul(mem::size_of::<RenderCell>()),
            )
            .saturating_add(
                self.cells
                    .iter()
                    .map(RenderCell::estimated_extra_bytes)
                    .sum::<usize>(),
            )
    }
}

struct Scrollback {
    rows: VecDeque<Row>,
    max_lines: usize,
    max_bytes: usize,
    byte_len: usize,
}

impl Scrollback {
    fn new(config: ScrollbackConfig) -> Self {
        Self {
            rows: VecDeque::new(),
            max_lines: config.max_lines(),
            max_bytes: config.max_bytes(),
            byte_len: 0,
        }
    }

    fn push(&mut self, row: Row) {
        if self.max_lines == 0 || self.max_bytes == 0 {
            return;
        }

        self.byte_len = self.byte_len.saturating_add(row.estimated_bytes());
        self.rows.push_back(row);
        self.trim();
    }

    fn replace_rows(&mut self, rows: VecDeque<Row>) {
        self.rows = rows;
        self.recalculate_byte_len();
        self.trim();
    }

    fn resize_width(&mut self, columns: usize) {
        for row in &mut self.rows {
            row.resize_width(columns);
        }
        self.recalculate_byte_len();
        self.trim();
    }

    fn iter(&self) -> impl Iterator<Item = &Row> {
        self.rows.iter()
    }

    fn trim(&mut self) {
        while self.rows.len() > self.max_lines || self.byte_len > self.max_bytes {
            let Some(row) = self.rows.pop_front() else {
                self.byte_len = 0;
                break;
            };
            self.byte_len = self.byte_len.saturating_sub(row.estimated_bytes());
        }
    }

    fn recalculate_byte_len(&mut self) {
        self.byte_len = self.rows.iter().map(Row::estimated_bytes).sum();
    }

    fn rows(&self) -> Vec<ScrollbackRow> {
        self.rows.iter().map(Row::to_scrollback_row).collect()
    }

    fn row(&self, handle: RowHandle) -> Option<ScrollbackRow> {
        self.rows
            .iter()
            .find(|row| row.handle == handle)
            .map(Row::to_scrollback_row)
    }

    fn len(&self) -> usize {
        self.rows.len()
    }

    fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    fn byte_len(&self) -> usize {
        self.byte_len
    }
}

struct DamageTracker {
    rows: BTreeSet<usize>,
}

impl DamageTracker {
    fn dirty_all(rows: usize) -> Self {
        let mut tracker = Self {
            rows: BTreeSet::new(),
        };
        tracker.mark_all(rows);
        tracker
    }

    fn mark(&mut self, row: usize) {
        self.rows.insert(row);
    }

    fn mark_all(&mut self, rows: usize) {
        self.rows.extend(0..rows);
    }

    fn take(&mut self, columns: usize) -> Vec<DamageRegion> {
        let damage = self
            .rows
            .iter()
            .copied()
            .map(|row| DamageRegion::new(row, 0, columns))
            .collect::<Vec<_>>();
        self.rows.clear();
        damage
    }
}

fn printable_width(ch: char) -> u8 {
    if ch == '\0' { 0 } else { 1 }
}

fn sgr_u8(value: u16) -> u8 {
    value.min(u16::from(u8::MAX)) as u8
}

struct ReflowResult {
    rows: Vec<Row>,
    cursor_row: usize,
    cursor_column: usize,
    pending_wrap: bool,
}

struct LogicalLine {
    cells: Vec<RenderCell>,
    handles: Vec<RowHandle>,
    cursor_offset: Option<usize>,
}

fn reflow_rows(
    rows: Vec<Row>,
    cursor_global_row: usize,
    cursor_column: usize,
    cursor_pending_wrap: bool,
    columns: usize,
    next_row_id: &mut u64,
) -> ReflowResult {
    let logical_lines = logical_lines(rows, cursor_global_row, cursor_column, cursor_pending_wrap);
    let mut output = Vec::new();
    let mut cursor_row = 0;
    let mut cursor_column = 0;
    let mut pending_wrap = false;

    for line in logical_lines {
        let line_start = output.len();
        let line_len = line.cells.len();
        let cursor_offset = line.cursor_offset;
        let rows = visual_rows(line, columns, next_row_id);

        if let Some(cursor_offset) = cursor_offset {
            let placement = cursor_placement(cursor_offset, line_len, columns);
            cursor_row = line_start + placement.row;
            cursor_column = placement.column;
            pending_wrap = placement.pending_wrap;
        }

        output.extend(rows);
    }

    if output.is_empty() {
        output.push(blank_row(columns, next_row_id));
    }

    ReflowResult {
        rows: output,
        cursor_row,
        cursor_column,
        pending_wrap,
    }
}

fn logical_lines(
    rows: Vec<Row>,
    cursor_global_row: usize,
    cursor_column: usize,
    cursor_pending_wrap: bool,
) -> Vec<LogicalLine> {
    let mut lines = Vec::new();
    let mut current = LogicalLine {
        cells: Vec::new(),
        handles: Vec::new(),
        cursor_offset: None,
    };

    for (row_index, row) in rows.into_iter().enumerate() {
        let cursor_extent = if row_index == cursor_global_row {
            if cursor_pending_wrap {
                row.cells.len()
            } else {
                cursor_column.min(row.cells.len())
            }
        } else {
            0
        };
        let segment_len = if row.wrapped {
            row.cells.len()
        } else {
            occupied_cell_count(&row.cells).max(cursor_extent)
        };
        let segment_len = segment_len.min(row.cells.len());

        if row_index == cursor_global_row {
            let offset = if cursor_pending_wrap {
                current.cells.len().saturating_add(row.cells.len())
            } else {
                current
                    .cells
                    .len()
                    .saturating_add(cursor_column.min(row.cells.len()))
            };
            current.cursor_offset = Some(offset);
        }

        current.handles.push(row.handle);
        current
            .cells
            .extend(row.cells.into_iter().take(segment_len));

        if !row.wrapped {
            lines.push(current);
            current = LogicalLine {
                cells: Vec::new(),
                handles: Vec::new(),
                cursor_offset: None,
            };
        }
    }

    if !current.handles.is_empty() || !current.cells.is_empty() {
        lines.push(current);
    }

    lines
}

fn visual_rows(line: LogicalLine, columns: usize, next_row_id: &mut u64) -> Vec<Row> {
    let visual_count = line.cells.len().max(1).div_ceil(columns);
    let mut rows = Vec::with_capacity(visual_count);

    for index in 0..visual_count {
        let start = index * columns;
        let end = (start + columns).min(line.cells.len());
        let mut cells = line.cells[start..end].to_vec();
        cells.resize(columns, RenderCell::empty());

        let handle = line
            .handles
            .get(index)
            .copied()
            .map(next_generation)
            .unwrap_or_else(|| {
                let handle = RowHandle::new(*next_row_id, 0);
                *next_row_id = next_row_id.saturating_add(1);
                handle
            });

        rows.push(Row {
            handle,
            cells,
            wrapped: end < line.cells.len(),
        });
    }

    rows
}

struct CursorPlacement {
    row: usize,
    column: usize,
    pending_wrap: bool,
}

// Keep the modulo form until Hera's MSRV includes `usize::is_multiple_of`.
#[allow(clippy::manual_is_multiple_of)]
fn cursor_placement(offset: usize, line_len: usize, columns: usize) -> CursorPlacement {
    if line_len > 0 && offset >= line_len && line_len % columns == 0 {
        return CursorPlacement {
            row: (line_len / columns).saturating_sub(1),
            column: columns - 1,
            pending_wrap: true,
        };
    }

    CursorPlacement {
        row: offset / columns,
        column: (offset % columns).min(columns - 1),
        pending_wrap: false,
    }
}

fn visible_start_for_cursor(total_rows: usize, viewport_rows: usize, cursor_row: usize) -> usize {
    if total_rows <= viewport_rows {
        return 0;
    }

    let max_start = total_rows - viewport_rows;
    cursor_row
        .saturating_add(1)
        .saturating_sub(viewport_rows)
        .min(max_start)
}

fn occupied_cell_count(cells: &[RenderCell]) -> usize {
    cells
        .iter()
        .rposition(|cell| *cell != RenderCell::empty())
        .map_or(0, |index| index + 1)
}

fn blank_row(columns: usize, next_row_id: &mut u64) -> Row {
    let handle = RowHandle::new(*next_row_id, 0);
    *next_row_id = next_row_id.saturating_add(1);
    Row::blank(columns, handle)
}

fn next_generation(handle: RowHandle) -> RowHandle {
    RowHandle::new(handle.id(), handle.generation().saturating_add(1))
}
