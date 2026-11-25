use std::collections::HashMap;
use std::io::{self, Read};

use ratatui::layout::Rect;

use crate::clipboard::{
    ClipboardBackend, ClipboardRead, ClipboardText, Generation, Publication, export_token,
};
use crate::constants::{DEFAULT_COLS, DEFAULT_ROWS};
use crate::formula::FormulaEvaluator;
use crate::formula_ref::{self, Axis, Edit};
use crate::shell::ShellPrompt;
use crate::types::{CellStyle, RowColumnSelectMode, SaveFormat, StdinFormat, VisualSubMode};
use crate::update::UpdateInfo;

/// Represents copied/cut cell data with relative positions
pub struct ClipboardData {
    /// Cell data: ((relative_row, relative_col), value, style)
    cells: Vec<((usize, usize), String, Option<CellStyle>)>,
    origin: (usize, usize),
    height: usize,
    width: usize,
    action: ClipboardAction,
    export: ClipboardExport,
}

#[derive(PartialEq)]
enum ClipboardAction {
    Copy,
    Cut,
    StaleCut,
}

enum ClipboardExport {
    Local,
    Failed,
    Published {
        token: String,
        generation: Option<Generation>,
    },
}

impl ClipboardData {
    fn plain_text(&self) -> String {
        let mut text = String::new();
        let mut cells = self.cells.iter().peekable();
        for row in 0..self.height {
            if row > 0 {
                text.push('\n');
            }
            for col in 0..self.width {
                if col > 0 {
                    text.push('\t');
                }
                if let Some((_, value, _)) = cells.next_if(|cell| cell.0 == (row, col)) {
                    text.push_str(value);
                }
            }
        }
        text
    }

    fn matches_export(&self, entry: &ClipboardText, observed: Option<Generation>) -> bool {
        match &self.export {
            ClipboardExport::Published { token, generation } => {
                (generation.is_none() || observed.is_none() || *generation == observed)
                    && entry.token.as_ref() == Some(token)
                    && self.plain_text() == entry.text
            }
            _ => false,
        }
    }

    fn owns_publication(&self, observed: Option<Generation>) -> bool {
        matches!(self.export, ClipboardExport::Published { generation: Some(generation), .. }
            if Some(generation) == observed)
    }
}

pub struct Spreadsheet {
    pub cells: HashMap<(usize, usize), String>,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub scroll_row: usize,
    pub scroll_col: usize,
    pub editing: bool,
    pub edit_buffer: String,
    pub num_rows: usize,
    pub num_cols: usize,
    // Formula mode fields
    pub formula_mode: bool,
    pub selecting_ref: bool,
    pub ref_cursor_row: usize,
    pub ref_cursor_col: usize,
    pub ref_anchor: Option<(usize, usize)>,
    pub ref_insert_pos: usize,
    pub ref_current_len: usize,
    // Normal mode selection
    pub selection_anchor: Option<(usize, usize)>,
    // Visual mode
    pub visual_mode: bool,
    pub visual_sub_mode: VisualSubMode,
    // Cell styling
    pub cell_styles: HashMap<(usize, usize), CellStyle>,
    pub col_widths: HashMap<usize, u16>,
    pub row_heights: HashMap<usize, u16>,
    // Save mode
    pub save_mode: bool,
    pub save_format: SaveFormat,
    pub save_filename: String,
    pub save_message: Option<String>,
    // Open mode
    pub open_mode: bool,
    pub open_filename: String,
    pub open_message: Option<String>,
    // Row/Column select mode
    pub row_column_select_mode: RowColumnSelectMode,
    pub selected_rows: Option<(usize, usize)>, // (min_row, max_row)
    pub selected_cols: Option<(usize, usize)>, // (min_col, max_col)
    // Dark mode
    pub dark_mode: bool,
    // Find mode
    pub find_mode: bool,
    pub find_query: String,
    pub find_matches: Vec<(usize, usize)>, // List of (row, col) matching the query
    // Clipboard (internal for cut tracking)
    pub clipboard_data: Option<ClipboardData>,
    pub clipboard_message: Option<&'static str>,
    // Command mode (vim-style :command)
    pub command_mode: bool,
    pub command_buffer: String,
    pub command_message: Option<String>,
    pub shell_prompt: Option<ShellPrompt>,
    // Update mode
    pub update_available: Option<UpdateInfo>,
    pub update_prompt_shown: bool,
    pub update_in_progress: bool,
    pub update_message: Option<String>,
    /// If true, user chose "don't show again" (persisted in ~/.xlrc)
    pub hide_update_prompt: bool,
    // Formula autocomplete
    pub formula_autocomplete_active: bool,
    pub formula_suggestions: Vec<String>,
    pub formula_suggestion_index: usize,
    pub formula_prefix: String,
}

impl Spreadsheet {
    pub fn new() -> Self {
        Self {
            cells: HashMap::new(),
            cursor_row: 0,
            cursor_col: 0,
            scroll_row: 0,
            scroll_col: 0,
            editing: false,
            edit_buffer: String::new(),
            num_rows: DEFAULT_ROWS,
            num_cols: DEFAULT_COLS,
            formula_mode: false,
            selecting_ref: false,
            ref_cursor_row: 0,
            ref_cursor_col: 0,
            ref_anchor: None,
            ref_insert_pos: 0,
            ref_current_len: 0,
            selection_anchor: None,
            visual_mode: false,
            visual_sub_mode: VisualSubMode::Main,
            cell_styles: HashMap::new(),
            col_widths: HashMap::new(),
            row_heights: HashMap::new(),
            save_mode: false,
            save_format: SaveFormat::Csv,
            save_filename: String::from("spreadsheet"),
            save_message: None,
            open_mode: false,
            open_filename: String::new(),
            open_message: None,
            row_column_select_mode: RowColumnSelectMode::None,
            selected_rows: None,
            selected_cols: None,
            dark_mode: false,
            find_mode: false,
            find_query: String::new(),
            find_matches: Vec::new(),
            clipboard_data: None,
            clipboard_message: None,
            command_mode: false,
            command_buffer: String::new(),
            command_message: None,
            shell_prompt: None,
            update_available: None,
            update_prompt_shown: false,
            update_in_progress: false,
            update_message: None,
            hide_update_prompt: false,
            formula_autocomplete_active: false,
            formula_suggestions: Vec::new(),
            formula_suggestion_index: 0,
            formula_prefix: String::new(),
        }
    }

    pub fn enter_command_mode(&mut self) {
        self.command_mode = true;
        self.command_buffer.clear();
        self.command_message = None;
    }

    pub fn exit_command_mode(&mut self) {
        self.command_mode = false;
        self.command_buffer.clear();
        self.command_message = None;
    }

    /// Parse a cell reference like "A1", "B23", "AA5" and return (row, col)
    pub fn parse_cell_reference(input: &str) -> Option<(usize, usize)> {
        let input = input.trim().to_uppercase();
        if input.is_empty() {
            return None;
        }

        // Find where letters end and numbers begin
        let mut col_str = String::new();
        let mut row_str = String::new();

        for c in input.chars() {
            if c.is_ascii_alphabetic() {
                if !row_str.is_empty() {
                    // Letters after numbers is invalid
                    return None;
                }
                col_str.push(c);
            } else if c.is_ascii_digit() {
                row_str.push(c);
            } else {
                return None; // Invalid character
            }
        }

        if col_str.is_empty() || row_str.is_empty() {
            return None;
        }

        // Convert column letters to index (A=0, B=1, ..., Z=25, AA=26, etc.)
        let mut col: usize = 0;
        for c in col_str.chars() {
            col = col * 26 + (c as usize - 'A' as usize + 1);
        }
        col -= 1; // Convert to 0-based index

        // Convert row number to index (1-based to 0-based)
        let row: usize = row_str.parse().ok()?;
        if row == 0 {
            return None; // Row numbers start at 1
        }
        let row = row - 1;

        Some((row, col))
    }

    pub fn execute_command(&mut self) -> bool {
        let cmd = self.command_buffer.trim().to_uppercase();
        
        // Check if it's a quit command
        if cmd == "Q" || cmd == "QUIT" {
            return true; // Signal to quit
        }

        if cmd == "RUN" {
            match self.begin_shell_run() {
                Ok(()) => self.exit_command_mode(),
                Err(message) => self.command_message = Some(message.to_string()),
            }
            return false;
        }

        // Try to parse as cell reference
        if let Some((row, col)) = Self::parse_cell_reference(&cmd) {
            if row < self.num_rows && col < self.num_cols {
                self.cursor_row = row;
                self.cursor_col = col;
                self.selection_anchor = None;
                self.command_message = None;
                self.exit_command_mode();
            } else {
                self.command_message = Some(format!("Cell {} is out of range", cmd));
            }
        } else {
            self.command_message = Some(format!("Unknown command: {}", cmd));
        }

        false // Don't quit
    }

    pub fn enter_find_mode(&mut self) {
        self.find_mode = true;
        self.find_query.clear();
        self.find_matches.clear();
    }

    pub fn exit_find_mode(&mut self) {
        self.find_mode = false;
        self.find_query.clear();
        self.find_matches.clear();
    }

    pub fn update_find_matches(&mut self) {
        self.find_matches.clear();
        
        if self.find_query.is_empty() {
            return;
        }

        let query_lower = self.find_query.to_lowercase();
        
        // Search through all cells
        for row in 0..self.num_rows {
            for col in 0..self.num_cols {
                let content = self.get_cell(row, col);
                if !content.is_empty() && content.to_lowercase().contains(&query_lower) {
                    self.find_matches.push((row, col));
                }
            }
        }

        // Move cursor to first match if any
        if let Some(&(row, col)) = self.find_matches.first() {
            self.cursor_row = row;
            self.cursor_col = col;
        }
    }

    pub fn is_find_match(&self, row: usize, col: usize) -> bool {
        self.find_mode && self.find_matches.contains(&(row, col))
    }

    /// Copy the current selection (or single cell) to clipboard
    pub fn copy_selection(&mut self, clipboard: &mut impl ClipboardBackend) {
        self.copy_or_cut_selection(false, clipboard);
    }

    /// Cut the current selection (or single cell) - copies and marks for deletion on paste
    pub fn cut_selection(&mut self, clipboard: &mut impl ClipboardBackend) {
        self.copy_or_cut_selection(true, clipboard);
    }

    fn copy_or_cut_selection(&mut self, is_cut: bool, clipboard: &mut impl ClipboardBackend) {
        let mut snapshot = self.capture_clipboard(is_cut);
        let token = export_token();
        snapshot.export = match clipboard.publish(&snapshot.plain_text(), &token) {
            Publication::Offline => ClipboardExport::Local,
            Publication::Failed => ClipboardExport::Failed,
            Publication::Published(generation) => ClipboardExport::Published { token, generation },
        };
        self.clipboard_data = Some(snapshot);
    }

    pub(crate) fn capture_clipboard(&self, is_cut: bool) -> ClipboardData {
        let (origin, end) = if let Some(range) = self.get_selection_range() {
            range
        } else if let Some((min_row, max_row)) = self.selected_rows {
            ((min_row, 0), (max_row, self.num_cols - 1))
        } else if let Some((min_col, max_col)) = self.selected_cols {
            ((0, min_col), (self.num_rows - 1, max_col))
        } else {
            let position = (self.cursor_row, self.cursor_col);
            (position, position)
        };

        let height = end.0 - origin.0 + 1;
        let width = end.1 - origin.1 + 1;
        ClipboardData {
            cells: self
                .clipboard_contents(origin, height, width)
                .map(|(offset, value, style)| (offset, value.to_owned(), style))
                .collect(),
            origin,
            height,
            width,
            export: ClipboardExport::Local,
            action: if is_cut {
                ClipboardAction::Cut
            } else {
                ClipboardAction::Copy
            },
        }
    }

    fn clipboard_contents(
        &self,
        (start_row, start_col): (usize, usize),
        height: usize,
        width: usize,
    ) -> impl Iterator<Item = ((usize, usize), &str, Option<CellStyle>)> {
        (0..height).flat_map(move |row| {
            (0..width).filter_map(move |col| {
                let position = (start_row + row, start_col + col);
                let value = self.get_cell(position.0, position.1);
                let style = self.cell_styles.get(&position).copied();
                (!value.is_empty() || style.is_some()).then_some(((row, col), value, style))
            })
        })
    }

    fn invalidate_cut(&mut self) {
        if let Some(snapshot) = &mut self.clipboard_data {
            if snapshot.action == ClipboardAction::Cut {
                snapshot.action = ClipboardAction::StaleCut;
            }
        }
    }

    /// Paste from clipboard at current cursor position
    pub fn paste(&mut self, clipboard: &mut impl ClipboardBackend) {
        self.clipboard_message = None;
        let snapshot = self.clipboard_data.take();
        let entry = clipboard.read_stable();
        match entry {
            ClipboardRead::Text(entry, generation) => {
                if let Some(snapshot) =
                    snapshot.filter(|data| data.matches_export(&entry, generation))
                {
                    let can_move = snapshot.owns_publication(generation);
                    self.paste_snapshot(snapshot, can_move);
                } else {
                    self.paste_text(&entry.text);
                }
            }
            ClipboardRead::NoText => {}
            ClipboardRead::Changed => {
                self.clipboard_data = snapshot.map(|mut snapshot| {
                    snapshot.action = ClipboardAction::Copy;
                    snapshot
                });
                self.clipboard_message = Some("  Clipboard changed. Paste again.");
            }
            ClipboardRead::Offline | ClipboardRead::Unavailable => {
                if let Some(snapshot) = snapshot {
                    let can_move = matches!(entry, ClipboardRead::Offline)
                        && matches!(snapshot.export, ClipboardExport::Local);
                    self.paste_snapshot(snapshot, can_move);
                }
            }
        }
    }

    fn paste_snapshot(&mut self, mut snapshot: ClipboardData, can_move: bool) {
        if snapshot.action == ClipboardAction::Cut && !can_move {
            snapshot.action = ClipboardAction::StaleCut;
        }
        self.paste_internal(snapshot);
    }

    fn paste_internal(&mut self, mut snapshot: ClipboardData) {
        let dest_row = self.cursor_row;
        let dest_col = self.cursor_col;

        let captured = snapshot
            .cells
            .iter()
            .map(|(offset, value, style)| (*offset, value.as_str(), *style));
        let source_changed = snapshot.action == ClipboardAction::Cut
            && !self
                .clipboard_contents(snapshot.origin, snapshot.height, snapshot.width)
                .eq(captured);
        if snapshot.action == ClipboardAction::StaleCut || source_changed {
            snapshot.action = ClipboardAction::Copy;
            self.clipboard_data = Some(snapshot);
            self.clipboard_message = Some("  Cut canceled. Paste again to copy.");
            return;
        }

        if snapshot.action == ClipboardAction::Cut {
            let (source_row, source_col) = snapshot.origin;
            for &((row, col), _, _) in &snapshot.cells {
                let position = (source_row + row, source_col + col);
                self.cells.remove(&position);
                self.cell_styles.remove(&position);
            }
        }

        let end_row = dest_row + snapshot.height;
        let end_col = dest_col + snapshot.width;
        self.num_rows = self.num_rows.max(end_row);
        self.num_cols = self.num_cols.max(end_col);
        for row in dest_row..end_row {
            for col in dest_col..end_col {
                self.cells.remove(&(row, col));
                self.cell_styles.remove(&(row, col));
            }
        }

        for ((rel_row, rel_col), value, style) in &snapshot.cells {
            let position = (dest_row + rel_row, dest_col + rel_col);
            if !value.is_empty() {
                self.cells.insert(position, value.clone());
            }
            if let Some(s) = style {
                self.cell_styles.insert(position, *s);
            }
        }

        snapshot.action = ClipboardAction::Copy;
        self.clipboard_data = Some(snapshot);
        self.clear_selection();
    }

    fn paste_text(&mut self, text: &str) {
        let dest_row = self.cursor_row;
        let dest_col = self.cursor_col;

        for (row_offset, line) in text.lines().enumerate() {
            let new_row = dest_row + row_offset;
            self.num_rows = self.num_rows.max(new_row + 1);
            for (col_offset, value) in line.split('\t').enumerate() {
                let new_col = dest_col + col_offset;
                self.num_cols = self.num_cols.max(new_col + 1);
                self.set_cell(new_row, new_col, value.to_owned());
            }
        }

        self.clear_selection();
    }

    pub fn toggle_dark_mode(&mut self) {
        self.dark_mode = !self.dark_mode;
        // Save the setting to the config file
        let mut settings = crate::settings::Settings::load();
        settings.set_dark_mode(self.dark_mode);
    }

    pub fn col_name(col: usize) -> String {
        let mut name = String::new();
        let mut c = col;
        loop {
            name.insert(0, (b'A' + (c % 26) as u8) as char);
            if c < 26 {
                break;
            }
            c = c / 26 - 1;
        }
        name
    }

    pub fn cell_ref(&self) -> String {
        format!("{}{}", Self::col_name(self.cursor_col), self.cursor_row + 1)
    }

    pub fn get_cell(&self, row: usize, col: usize) -> &str {
        self.cells
            .get(&(row, col))
            .map(|s| s.as_str())
            .unwrap_or("")
    }

    pub fn set_cell(&mut self, row: usize, col: usize, value: String) {
        if value.is_empty() {
            self.cells.remove(&(row, col));
        } else {
            self.cells.insert((row, col), value);
        }
    }

    pub fn move_cursor(&mut self, dr: isize, dc: isize, extend_selection: bool) {
        if extend_selection {
            if self.selection_anchor.is_none() {
                self.selection_anchor = Some((self.cursor_row, self.cursor_col));
            }
        } else {
            self.selection_anchor = None;
        }

        let new_row = (self.cursor_row as isize + dr).max(0) as usize;
        let new_col = (self.cursor_col as isize + dc).max(0) as usize;

        self.cursor_row = new_row.min(self.num_rows - 1);
        self.cursor_col = new_col.min(self.num_cols - 1);
    }

    /// Find the rightmost column with data in the given row
    pub fn find_last_col_in_row(&self, row: usize) -> Option<usize> {
        let mut max_col = None;
        for &(r, c) in self.cells.keys() {
            if r == row && !self.get_cell(r, c).is_empty() {
                max_col = Some(max_col.map_or(c, |m: usize| m.max(c)));
            }
        }
        max_col
    }

    /// Find the leftmost column with data in the given row
    pub fn find_first_col_in_row(&self, row: usize) -> Option<usize> {
        let mut min_col = None;
        for &(r, c) in self.cells.keys() {
            if r == row && !self.get_cell(r, c).is_empty() {
                min_col = Some(min_col.map_or(c, |m: usize| m.min(c)));
            }
        }
        min_col
    }

    /// Find the bottommost row with data in the given column
    pub fn find_last_row_in_col(&self, col: usize) -> Option<usize> {
        let mut max_row = None;
        for &(r, c) in self.cells.keys() {
            if c == col && !self.get_cell(r, c).is_empty() {
                max_row = Some(max_row.map_or(r, |m: usize| m.max(r)));
            }
        }
        max_row
    }

    /// Find the topmost row with data in the given column
    pub fn find_first_row_in_col(&self, col: usize) -> Option<usize> {
        let mut min_row = None;
        for &(r, c) in self.cells.keys() {
            if c == col && !self.get_cell(r, c).is_empty() {
                min_row = Some(min_row.map_or(r, |m: usize| m.min(r)));
            }
        }
        min_row
    }

    /// Jump to the last data column in the current row (right)
    pub fn jump_to_last_col(&mut self) {
        if let Some(col) = self.find_last_col_in_row(self.cursor_row) {
            self.cursor_col = col;
            self.selection_anchor = None;
        }
    }

    /// Jump to the first data column in the current row (left)
    pub fn jump_to_first_col(&mut self) {
        if let Some(col) = self.find_first_col_in_row(self.cursor_row) {
            self.cursor_col = col;
            self.selection_anchor = None;
        }
    }

    /// Jump to the last data row in the current column (down)
    pub fn jump_to_last_row(&mut self) {
        if let Some(row) = self.find_last_row_in_col(self.cursor_col) {
            self.cursor_row = row;
            self.selection_anchor = None;
        }
    }

    /// Jump to the first data row in the current column (up)
    pub fn jump_to_first_row(&mut self) {
        if let Some(row) = self.find_first_row_in_col(self.cursor_col) {
            self.cursor_row = row;
            self.selection_anchor = None;
        }
    }

    pub fn get_selection_range(&self) -> Option<((usize, usize), (usize, usize))> {
        if let Some((anchor_row, anchor_col)) = self.selection_anchor {
            let min_row = anchor_row.min(self.cursor_row);
            let max_row = anchor_row.max(self.cursor_row);
            let min_col = anchor_col.min(self.cursor_col);
            let max_col = anchor_col.max(self.cursor_col);
            Some(((min_row, min_col), (max_row, max_col)))
        } else {
            None
        }
    }

    /// Returns selection stats: (row_count, cell_count, numeric_count, sum) for the current selection
    /// Returns None if no multi-cell selection exists
    pub fn get_selection_stats(&self) -> Option<(usize, usize, usize, f64)> {
        // Get the effective selection range - could be from selection_anchor, selected_rows, or selected_cols
        let range = if let Some(((min_row, min_col), (max_row, max_col))) = self.get_selection_range() {
            // Regular cell selection
            if min_row == max_row && min_col == max_col {
                return None; // Single cell, no stats
            }
            Some((min_row, min_col, max_row, max_col))
        } else if let Some((min_row, max_row)) = self.selected_rows {
            // Row selection - use all columns with data
            Some((min_row, 0, max_row, self.num_cols - 1))
        } else if let Some((min_col, max_col)) = self.selected_cols {
            // Column selection - use all rows with data
            Some((0, min_col, self.num_rows - 1, max_col))
        } else {
            None
        };

        let (min_row, min_col, max_row, max_col) = range?;

        let mut cell_count = 0usize;
        let mut numeric_count = 0usize;
        let mut sum = 0.0f64;
        let mut rows_with_data = std::collections::HashSet::new();
        let mut formulas = FormulaEvaluator::new(self);

        for row in min_row..=max_row {
            for col in min_col..=max_col {
                let content = self.get_cell(row, col);
                if !content.is_empty() {
                    cell_count += 1;
                    rows_with_data.insert(row);
                    // Evaluate the cell to get the actual value (handles formulas)
                    let evaluated = formulas.evaluate(row, col);
                    if let Ok(val) = evaluated.parse::<f64>() {
                        numeric_count += 1;
                        sum += val;
                    }
                }
            }
        }

        let row_count = rows_with_data.len();

        if cell_count > 0 {
            Some((row_count, cell_count, numeric_count, sum))
        } else {
            None
        }
    }

    pub fn clear_selection(&mut self) {
        self.selection_anchor = None;
    }

    pub fn selection_ref(&self) -> String {
        if let Some((anchor_row, anchor_col)) = self.selection_anchor {
            let min_row = anchor_row.min(self.cursor_row);
            let max_row = anchor_row.max(self.cursor_row);
            let min_col = anchor_col.min(self.cursor_col);
            let max_col = anchor_col.max(self.cursor_col);
            format!(
                "{}{}:{}{}",
                Self::col_name(min_col),
                min_row + 1,
                Self::col_name(max_col),
                max_row + 1
            )
        } else {
            self.cell_ref()
        }
    }

    pub fn start_editing(&mut self) {
        self.editing = true;
        self.edit_buffer = self.get_cell(self.cursor_row, self.cursor_col).to_string();
        self.formula_mode = self.edit_buffer.starts_with('=');
        self.selecting_ref = false;
        self.ref_anchor = None;
        self.ref_insert_pos = 0;
        self.ref_current_len = 0;
        // Initialize autocomplete state
        if self.formula_mode {
            let after_equals = &self.edit_buffer[1..];
            let prefix_end = after_equals
                .char_indices()
                .find(|(_, ch)| !ch.is_alphabetic())
                .map(|(i, _)| i)
                .unwrap_or(after_equals.len());
            self.formula_prefix = after_equals[..prefix_end].to_string();
            self.update_formula_suggestions();
        } else {
            self.formula_autocomplete_active = false;
            self.formula_suggestions.clear();
            self.formula_prefix.clear();
        }
    }

    pub fn finish_editing_with_move(&mut self, dr: isize, dc: isize) {
        if self.formula_mode {
            let open_parens = self.edit_buffer.chars().filter(|&c| c == '(').count();
            let close_parens = self.edit_buffer.chars().filter(|&c| c == ')').count();
            for _ in 0..(open_parens.saturating_sub(close_parens)) {
                self.edit_buffer.push(')');
            }
        }

        self.set_cell(self.cursor_row, self.cursor_col, self.edit_buffer.clone());
        self.reset_editing_state();
        self.move_cursor(dr, dc, false);
    }

    pub fn finish_editing(&mut self) {
        self.finish_editing_with_move(1, 0);
    }

    pub fn cancel_editing(&mut self) {
        self.reset_editing_state();
    }

    pub fn reset_editing_state(&mut self) {
        self.editing = false;
        self.edit_buffer.clear();
        self.formula_mode = false;
        self.selecting_ref = false;
        self.ref_anchor = None;
        self.ref_insert_pos = 0;
        self.ref_current_len = 0;
        self.formula_autocomplete_active = false;
        self.formula_suggestions.clear();
        self.formula_prefix.clear();
        self.formula_suggestion_index = 0;
    }

    pub fn delete_cell(&mut self) {
        if let Some(((min_row, min_col), (max_row, max_col))) = self.get_selection_range() {
            for row in min_row..=max_row {
                for col in min_col..=max_col {
                    self.cells.remove(&(row, col));
                }
            }
            self.clear_selection();
        } else {
            self.cells.remove(&(self.cursor_row, self.cursor_col));
        }
    }

    pub fn get_available_formulas() -> Vec<String> {
        vec![
            "ABS".to_string(),
            "AND".to_string(),
            "AVG".to_string(),
            "AVERAGEIF".to_string(),
            "CONCAT".to_string(),
            "CONCATENATE".to_string(),
            "CORREL".to_string(),
            "COUNT".to_string(),
            "COUNTA".to_string(),
            "COUNTIF".to_string(),
            "IF".to_string(),
            "IFERROR".to_string(),
            "INT".to_string(),
            "LEN".to_string(),
            "LEFT".to_string(),
            "LOWER".to_string(),
            "MAX".to_string(),
            "MEDIAN".to_string(),
            "MID".to_string(),
            "MIN".to_string(),
            "MOD".to_string(),
            "NOT".to_string(),
            "OR".to_string(),
            "POWER".to_string(),
            "PRODUCT".to_string(),
            "PROPER".to_string(),
            "RIGHT".to_string(),
            "ROUND".to_string(),
            "SHELL".to_string(),
            "SQRT".to_string(),
            "SUM".to_string(),
            "SUMIF".to_string(),
            "TRIM".to_string(),
            "UPPER".to_string(),
            "VLOOKUP".to_string(),
        ]
    }

    pub fn update_formula_suggestions(&mut self) {
        if !self.formula_mode || self.formula_prefix.is_empty() {
            self.formula_autocomplete_active = false;
            self.formula_suggestions.clear();
            return;
        }

        let prefix_upper = self.formula_prefix.to_uppercase();
        let all_formulas = Self::get_available_formulas();
        
        let mut suggestions: Vec<String> = all_formulas
            .into_iter()
            .filter(|f| f.starts_with(&prefix_upper))
            .collect();
        
        // Sort alphabetically for better UX
        suggestions.sort();
        
        self.formula_suggestions = suggestions;
        self.formula_autocomplete_active = !self.formula_suggestions.is_empty();
        if self.formula_suggestion_index >= self.formula_suggestions.len() {
            self.formula_suggestion_index = 0;
        }
    }

    pub fn handle_char_input(&mut self, c: char) {
        self.edit_buffer.push(c);

        if c == '=' && self.edit_buffer == "=" {
            self.formula_mode = true;
            self.formula_prefix.clear();
            self.formula_autocomplete_active = false;
            self.formula_suggestions.clear();
        } else if self.formula_mode {
            // Check if we're typing a formula name (letters after =)
            if c.is_alphabetic() {
                // Extract the formula prefix (everything after = that's letters)
                let after_equals = &self.edit_buffer[1..];
                // Find where the letters end (either at '(' or end of string)
                let prefix_end = after_equals
                    .char_indices()
                    .find(|(_, ch)| !ch.is_alphabetic())
                    .map(|(i, _)| i)
                    .unwrap_or(after_equals.len());
                self.formula_prefix = after_equals[..prefix_end].to_string();
                self.update_formula_suggestions();
            } else if c == '(' {
                // User typed '(', so they're done with the formula name
                self.formula_autocomplete_active = false;
                self.formula_suggestions.clear();
                self.enter_ref_selection_mode();
            } else {
                // Non-letter character, disable autocomplete
                self.formula_autocomplete_active = false;
                self.formula_suggestions.clear();
            }
        }
    }

    pub fn enter_ref_selection_mode(&mut self) {
        self.selecting_ref = true;
        self.ref_cursor_row = self.cursor_row;
        self.ref_cursor_col = self.cursor_col;
        self.ref_anchor = None;
        self.ref_insert_pos = self.edit_buffer.len();
        self.ref_current_len = 0;
    }

    pub fn exit_ref_selection_mode(&mut self) {
        self.selecting_ref = false;
        self.ref_anchor = None;
    }

    pub fn move_ref_cursor(&mut self, dr: isize, dc: isize, extend_range: bool) {
        let new_row = (self.ref_cursor_row as isize + dr).max(0) as usize;
        let new_col = (self.ref_cursor_col as isize + dc).max(0) as usize;
        self.ref_cursor_row = new_row.min(self.num_rows - 1);
        self.ref_cursor_col = new_col.min(self.num_cols - 1);

        if extend_range {
            if self.ref_anchor.is_none() {
                let prev_row = (self.ref_cursor_row as isize - dr).max(0) as usize;
                let prev_col = (self.ref_cursor_col as isize - dc).max(0) as usize;
                self.ref_anchor = Some((prev_row, prev_col));
            }
        } else {
            self.ref_anchor = None;
        }

        self.update_ref_in_buffer();
    }

    pub fn update_ref_in_buffer(&mut self) {
        let ref_text = if let Some((anchor_row, anchor_col)) = self.ref_anchor {
            let min_row = anchor_row.min(self.ref_cursor_row);
            let max_row = anchor_row.max(self.ref_cursor_row);
            let min_col = anchor_col.min(self.ref_cursor_col);
            let max_col = anchor_col.max(self.ref_cursor_col);
            format!(
                "{}{}:{}{}",
                Self::col_name(min_col),
                min_row + 1,
                Self::col_name(max_col),
                max_row + 1
            )
        } else {
            format!(
                "{}{}",
                Self::col_name(self.ref_cursor_col),
                self.ref_cursor_row + 1
            )
        };

        let end_pos = self.ref_insert_pos + self.ref_current_len;
        if end_pos <= self.edit_buffer.len() {
            self.edit_buffer.replace_range(self.ref_insert_pos..end_pos, &ref_text);
        } else {
            self.edit_buffer.push_str(&ref_text);
        }
        self.ref_current_len = ref_text.len();
    }

    pub fn get_ref_range(&self) -> Option<((usize, usize), (usize, usize))> {
        if !self.selecting_ref {
            return None;
        }
        if let Some((anchor_row, anchor_col)) = self.ref_anchor {
            let min_row = anchor_row.min(self.ref_cursor_row);
            let max_row = anchor_row.max(self.ref_cursor_row);
            let min_col = anchor_col.min(self.ref_cursor_col);
            let max_col = anchor_col.max(self.ref_cursor_col);
            Some(((min_row, min_col), (max_row, max_col)))
        } else {
            Some((
                (self.ref_cursor_row, self.ref_cursor_col),
                (self.ref_cursor_row, self.ref_cursor_col),
            ))
        }
    }

    pub fn enter_visual_mode(&mut self) {
        self.visual_mode = true;
        self.visual_sub_mode = VisualSubMode::Main;
    }

    pub fn exit_visual_mode(&mut self) {
        self.visual_mode = false;
        self.visual_sub_mode = VisualSubMode::Main;
    }

    pub fn enter_row_select_mode(&mut self) {
        self.row_column_select_mode = RowColumnSelectMode::RowSelect;
        self.selected_rows = Some((self.cursor_row, self.cursor_row));
        self.selected_cols = None;
        self.selection_anchor = None;
    }

    pub fn enter_column_select_mode(&mut self) {
        self.row_column_select_mode = RowColumnSelectMode::ColumnSelect;
        self.selected_cols = Some((self.cursor_col, self.cursor_col));
        self.selected_rows = None;
        self.selection_anchor = None;
    }

    pub fn exit_row_column_select_mode(&mut self) {
        self.row_column_select_mode = RowColumnSelectMode::None;
        self.selected_rows = None;
        self.selected_cols = None;
    }

    pub fn delete_selected_rows(&mut self) {
        if let Some((min_row, max_row)) = self.selected_rows {
            self.invalidate_cut();
            // Delete rows from bottom to top to avoid index shifting issues
            for row in (min_row..=max_row).rev() {
                self.delete_row(row);
            }
            self.rewrite_formulas(
                Axis::Row,
                Edit::Delete {
                    first: min_row,
                    last: max_row,
                },
            );
            // Adjust cursor position
            if self.cursor_row >= min_row {
                if self.cursor_row <= max_row {
                    self.cursor_row = min_row.min(self.num_rows - 1);
                } else {
                    self.cursor_row -= max_row - min_row + 1;
                }
            }
            self.exit_row_column_select_mode();
        }
    }

    pub fn delete_selected_columns(&mut self) {
        if let Some((min_col, max_col)) = self.selected_cols {
            self.invalidate_cut();
            // Delete columns from right to left to avoid index shifting issues
            for col in (min_col..=max_col).rev() {
                self.delete_column(col);
            }
            self.rewrite_formulas(
                Axis::Column,
                Edit::Delete {
                    first: min_col,
                    last: max_col,
                },
            );
            // Adjust cursor position
            if self.cursor_col >= min_col {
                if self.cursor_col <= max_col {
                    self.cursor_col = min_col.min(self.num_cols - 1);
                } else {
                    self.cursor_col -= max_col - min_col + 1;
                }
            }
            self.exit_row_column_select_mode();
        }
    }

    pub fn insert_rows_after_selected(&mut self) {
        if let Some((min_row, max_row)) = self.selected_rows {
            self.invalidate_cut();
            let count = max_row - min_row + 1;
            // Insert after the last selected row
            let insert_after = max_row;
            for _ in 0..count {
                self.insert_row_after(insert_after);
            }
            self.rewrite_formulas(
                Axis::Row,
                Edit::Insert {
                    at: insert_after + 1,
                    count,
                },
            );
            // Move cursor to the first newly inserted row
            self.cursor_row = insert_after + 1;
            self.selected_rows = Some((insert_after + 1, insert_after + count));
        }
    }

    pub fn insert_columns_after_selected(&mut self) {
        if let Some((min_col, max_col)) = self.selected_cols {
            self.invalidate_cut();
            let count = max_col - min_col + 1;
            // Insert after the last selected column
            let insert_after = max_col;
            for _ in 0..count {
                self.insert_column_after(insert_after);
            }
            self.rewrite_formulas(
                Axis::Column,
                Edit::Insert {
                    at: insert_after + 1,
                    count,
                },
            );
            // Move cursor to the first newly inserted column
            self.cursor_col = insert_after + 1;
            self.selected_cols = Some((insert_after + 1, insert_after + count));
        }
    }

    fn rewrite_formulas(&mut self, axis: Axis, edit: Edit) {
        for value in self.cells.values_mut() {
            if let Some(updated) = formula_ref::rewrite(value, axis, edit) {
                *value = updated;
            }
        }
    }

    fn insert_row_after(&mut self, row: usize) {
        // Increase num_rows
        self.num_rows += 1;
        // Shift all cells below (and including) row+1 down by 1
        // We need to iterate from the bottom to avoid overwriting
        for r in (row + 1..self.num_rows - 1).rev() {
            for col in 0..self.num_cols {
                if let Some(value) = self.cells.remove(&(r, col)) {
                    self.cells.insert((r + 1, col), value);
                }
                if let Some(style) = self.cell_styles.remove(&(r, col)) {
                    self.cell_styles.insert((r + 1, col), style);
                }
            }
            // Shift row heights
            if let Some(height) = self.row_heights.remove(&r) {
                self.row_heights.insert(r + 1, height);
            }
        }
    }

    fn insert_column_after(&mut self, col: usize) {
        // Increase num_cols
        self.num_cols += 1;
        // Shift all cells to the right of col down by 1
        // We need to iterate from the right to avoid overwriting
        for c in (col + 1..self.num_cols - 1).rev() {
            for row in 0..self.num_rows {
                if let Some(value) = self.cells.remove(&(row, c)) {
                    self.cells.insert((row, c + 1), value);
                }
                if let Some(style) = self.cell_styles.remove(&(row, c)) {
                    self.cell_styles.insert((row, c + 1), style);
                }
            }
            // Shift column widths
            if let Some(width) = self.col_widths.remove(&c) {
                self.col_widths.insert(c + 1, width);
            }
        }
    }

    fn delete_row(&mut self, row: usize) {
        // Remove all cells in this row
        for col in 0..self.num_cols {
            self.cells.remove(&(row, col));
            self.cell_styles.remove(&(row, col));
        }
        // Remove row height if set
        self.row_heights.remove(&row);
        // Shift all cells below this row up
        for r in (row + 1)..self.num_rows {
            for col in 0..self.num_cols {
                if let Some(value) = self.cells.remove(&(r, col)) {
                    self.cells.insert((r - 1, col), value);
                }
                if let Some(style) = self.cell_styles.remove(&(r, col)) {
                    self.cell_styles.insert((r - 1, col), style);
                }
            }
            // Shift row heights
            if let Some(height) = self.row_heights.remove(&r) {
                self.row_heights.insert(r - 1, height);
            }
        }
        // Decrease num_rows if this was the last row
        if row < self.num_rows {
            self.num_rows -= 1;
            if self.num_rows == 0 {
                self.num_rows = 1; // Keep at least one row
            }
        }
    }

    fn delete_column(&mut self, col: usize) {
        // Remove all cells in this column
        for row in 0..self.num_rows {
            self.cells.remove(&(row, col));
            self.cell_styles.remove(&(row, col));
        }
        // Remove column width if set
        self.col_widths.remove(&col);
        // Shift all cells to the right of this column left
        for c in (col + 1)..self.num_cols {
            for row in 0..self.num_rows {
                if let Some(value) = self.cells.remove(&(row, c)) {
                    self.cells.insert((row, c - 1), value);
                }
                if let Some(style) = self.cell_styles.remove(&(row, c)) {
                    self.cell_styles.insert((row, c - 1), style);
                }
            }
            // Shift column widths
            if let Some(width) = self.col_widths.remove(&c) {
                self.col_widths.insert(c - 1, width);
            }
        }
        // Decrease num_cols if this was the last column
        if col < self.num_cols {
            self.num_cols -= 1;
            if self.num_cols == 0 {
                self.num_cols = 1; // Keep at least one column
            }
        }
    }

    pub fn visible_cols(&self, width: u16) -> usize {
        let row_num_width = 5;
        let available = width.saturating_sub(row_num_width) as i32;
        let mut used = 0i32;
        let mut count = 0;
        for col in self.scroll_col..self.num_cols {
            let col_w = self.get_col_width(col) as i32;
            if used + col_w > available {
                break;
            }
            used += col_w;
            count += 1;
        }
        count.max(1)
    }

    pub fn visible_rows(&self, height: u16) -> usize {
        height.saturating_sub(7).max(1) as usize
    }

    pub fn is_numeric(s: &str) -> bool {
        s.parse::<f64>().is_ok()
    }

    pub fn adjust_scroll(&mut self, area: Rect) {
        let visible_cols = self.visible_cols(area.width);
        let visible_rows = self.visible_rows(area.height);

        if self.cursor_col < self.scroll_col {
            self.scroll_col = self.cursor_col;
        } else if self.cursor_col >= self.scroll_col + visible_cols {
            self.scroll_col = self.cursor_col - visible_cols + 1;
        }

        if self.cursor_row < self.scroll_row {
            self.scroll_row = self.cursor_row;
        } else if self.cursor_row >= self.scroll_row + visible_rows {
            self.scroll_row = self.cursor_row - visible_rows + 1;
        }
    }

    pub fn load_from_file(&mut self, filepath: &str) -> std::io::Result<()> {
        let path = std::path::Path::new(filepath);
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();

        let mut loaded = Self::new();
        match extension.as_str() {
            "csv" => loaded.load_delimited(std::fs::File::open(path)?, b',')?,
            "tsv" => loaded.load_delimited(std::fs::File::open(path)?, b'\t')?,
            "xlsx" | "xls" => loaded.load_excel(filepath)?,
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("Unsupported file format: {}", extension),
                ));
            }
        }
        self.replace_document(loaded);
        Ok(())
    }

    fn replace_document(&mut self, mut loaded: Self) {
        let (max_row, max_col) = loaded.get_data_bounds();
        loaded.num_rows = (max_row + 1).max(DEFAULT_ROWS);
        loaded.num_cols = (max_col + 1).max(DEFAULT_COLS);

        loaded.dark_mode = self.dark_mode;
        loaded.hide_update_prompt = self.hide_update_prompt;
        loaded.update_available = self.update_available.take();
        loaded.update_prompt_shown = self.update_prompt_shown;
        loaded.update_in_progress = self.update_in_progress;
        loaded.update_message = self.update_message.take();
        *self = loaded;
    }

    fn delimited_reader<R: Read>(source: R, delimiter: u8) -> csv::Reader<R> {
        csv::ReaderBuilder::new()
            .delimiter(delimiter)
            .has_headers(false)
            .from_reader(source)
    }

    fn load_delimited(&mut self, source: impl Read, delimiter: u8) -> io::Result<()> {
        let mut reader = Self::delimited_reader(source, delimiter);
        for (row_idx, result) in reader.records().enumerate() {
            let record = result?;
            for (col_idx, field) in record.iter().enumerate() {
                if !field.is_empty() {
                    self.set_cell(row_idx, col_idx, field.to_string());
                }
            }
        }

        Ok(())
    }

    fn load_excel(&mut self, filepath: &str) -> std::io::Result<()> {
        use calamine::{open_workbook_auto, Reader, Data};

        let path = std::path::Path::new(filepath);
        
        // Open workbook - calamine can auto-detect the format
        let mut workbook = open_workbook_auto(path)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

        // Get the first sheet name
        let sheet_names = workbook.sheet_names().to_owned();
        if sheet_names.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "No worksheets found in Excel file",
            ));
        }

        // Read the first worksheet
        let range = workbook
            .worksheet_range(&sheet_names[0])
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
        let Some((start_row, start_col)) = range.start() else {
            return Ok(());
        };

        for (row_idx, row) in range.rows().enumerate() {
            for (col_idx, cell) in row.iter().enumerate() {
                let value = match cell {
                    Data::Empty => continue,
                    Data::String(s) => s.clone(),
                    Data::Float(f) => {
                        // Format floats without unnecessary decimals
                        if f.fract() == 0.0 {
                            format!("{:.0}", f)
                        } else {
                            f.to_string()
                        }
                    }
                    Data::Int(i) => i.to_string(),
                    Data::Bool(b) => b.to_string(),
                    Data::Error(e) => format!("#ERROR: {:?}", e),
                    Data::DateTime(dt) => {
                        // Format datetime as string
                        format!("{}", dt)
                    }
                    Data::DateTimeIso(s) => s.clone(),
                    Data::DurationIso(s) => s.clone(),
                };

                if !value.is_empty() {
                    self.set_cell(
                        start_row as usize + row_idx,
                        start_col as usize + col_idx,
                        value,
                    );
                }
            }
        }

        Ok(())
    }

    pub fn load_from_buffer(&mut self, buffer: &[u8], format: StdinFormat) -> io::Result<()> {
        let delimiter = match format {
            // Prefer TSV when both delimiters yield consistent records.
            StdinFormat::Auto => [b'\t', b',']
                .into_iter()
                .find(|&delimiter| Self::looks_delimited(buffer, delimiter)),
            StdinFormat::Csv => Some(b','),
            StdinFormat::Tsv => Some(b'\t'),
            StdinFormat::Whitespace => None,
        };

        let mut loaded = Self::new();
        if let Some(delimiter) = delimiter {
            loaded.load_delimited(buffer, delimiter)?;
        } else {
            let text = String::from_utf8_lossy(buffer);
            for (row, line) in text
                .lines()
                .filter(|line| !line.trim().is_empty())
                .enumerate()
            {
                for (col, value) in line.split_whitespace().enumerate() {
                    loaded.set_cell(row, col, value.to_owned());
                }
            }
        }
        self.replace_document(loaded);
        Ok(())
    }

    fn looks_delimited(buffer: &[u8], delimiter: u8) -> bool {
        let mut reader = Self::delimited_reader(buffer, delimiter);
        let mut record = csv::ByteRecord::new();
        if !matches!(reader.read_byte_record(&mut record), Ok(true)) || record.len() < 2 {
            return false;
        }

        // The reader rejects records whose field counts differ from the first.
        loop {
            match reader.read_byte_record(&mut record) {
                Ok(true) => continue,
                Ok(false) => return true,
                Err(_) => return false,
            }
        }
    }
}

impl Default for Spreadsheet {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clipboard::OfflineClipboard;

    #[test]
    fn test_col_name() {
        assert_eq!(Spreadsheet::col_name(0), "A");
        assert_eq!(Spreadsheet::col_name(1), "B");
        assert_eq!(Spreadsheet::col_name(25), "Z");
        assert_eq!(Spreadsheet::col_name(26), "AA");
    }

    #[test]
    fn test_cell_operations() {
        let mut sheet = Spreadsheet::new();
        assert_eq!(sheet.get_cell(0, 0), "");

        sheet.set_cell(0, 0, "Hello".to_string());
        assert_eq!(sheet.get_cell(0, 0), "Hello");

        sheet.set_cell(0, 0, "".to_string());
        assert_eq!(sheet.get_cell(0, 0), "");
    }

    #[test]
    fn test_cursor_movement() {
        let mut sheet = Spreadsheet::new();
        assert_eq!(sheet.cursor_row, 0);
        assert_eq!(sheet.cursor_col, 0);

        sheet.move_cursor(1, 0, false);
        assert_eq!(sheet.cursor_row, 1);

        sheet.move_cursor(0, 1, false);
        assert_eq!(sheet.cursor_col, 1);

        sheet.move_cursor(-1, -1, false);
        assert_eq!(sheet.cursor_row, 0);
        assert_eq!(sheet.cursor_col, 0);
    }

    #[test]
    fn test_selection() {
        let mut sheet = Spreadsheet::new();
        assert!(sheet.get_selection_range().is_none());

        sheet.move_cursor(1, 1, true);
        let range = sheet.get_selection_range().unwrap();
        assert_eq!(range, ((0, 0), (1, 1)));

        sheet.clear_selection();
        assert!(sheet.get_selection_range().is_none());
    }

    #[test]
    fn stale_cut_cancels_overlapping_paste_and_remains_available_as_copy() {
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "first".into());
        sheet.set_cell(0, 1, "second".into());
        sheet.set_cell(0, 2, "destination".into());
        sheet.selection_anchor = Some((0, 0));
        sheet.cursor_col = 1;
        sheet.clipboard_data = Some(sheet.capture_clipboard(true));
        sheet.set_cell(0, 1, "edited".into());

        sheet.paste(&mut OfflineClipboard);

        assert_eq!(sheet.get_cell(0, 0), "first");
        assert_eq!(sheet.get_cell(0, 1), "edited");
        assert_eq!(sheet.get_cell(0, 2), "destination");
        assert!(sheet.clipboard_message.is_some());

        sheet.cursor_col = 3;
        sheet.paste(&mut OfflineClipboard);
        assert_eq!(sheet.get_cell(0, 1), "edited");
        assert_eq!(sheet.get_cell(0, 3), "first");
        assert_eq!(sheet.get_cell(0, 4), "second");
        assert!(sheet.clipboard_message.is_none());
    }

    #[test]
    fn cut_detects_new_content_in_a_previously_blank_field() {
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "first".into());
        sheet.selection_anchor = Some((0, 0));
        sheet.cursor_col = 1;
        sheet.clipboard_data = Some(sheet.capture_clipboard(true));
        sheet.set_cell(0, 1, "new".into());
        sheet.cursor_col = 2;

        sheet.paste(&mut OfflineClipboard);

        assert_eq!(sheet.get_cell(0, 0), "first");
        assert_eq!(sheet.get_cell(0, 1), "new");
        assert_eq!(sheet.get_cell(0, 2), "");
        assert!(sheet.clipboard_message.is_some());
    }

    #[test]
    fn cut_detects_a_style_change() {
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "same value".into());
        sheet.clipboard_data = Some(sheet.capture_clipboard(true));
        sheet.set_cell_bold(0, 0, true);
        sheet.cursor_col = 1;

        sheet.paste(&mut OfflineClipboard);

        assert_eq!(sheet.get_cell(0, 0), "same value");
        assert!(sheet.get_cell_style(0, 0).bold);
        assert_eq!(sheet.get_cell(0, 1), "");
        assert!(sheet.clipboard_message.is_some());
    }

    #[test]
    fn row_deletion_invalidates_cut_even_if_old_coordinates_still_match() {
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(1, 0, "same value".into());
        sheet.set_cell(2, 0, "same value".into());
        sheet.cursor_row = 1;
        sheet.clipboard_data = Some(sheet.capture_clipboard(true));

        sheet.selected_rows = Some((0, 0));
        sheet.delete_selected_rows();
        sheet.cursor_col = 1;
        sheet.paste(&mut OfflineClipboard);

        assert_eq!(sheet.get_cell(0, 0), "same value");
        assert_eq!(sheet.get_cell(1, 0), "same value");
        assert_eq!(sheet.get_cell(0, 1), "");
        assert!(sheet.clipboard_message.is_some());
    }

    #[test]
    fn column_insertion_invalidates_cut_even_if_old_coordinates_still_match() {
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 1, "same value".into());
        sheet.set_cell(0, 2, "same value".into());
        sheet.cursor_col = 2;
        sheet.clipboard_data = Some(sheet.capture_clipboard(true));

        sheet.selected_cols = Some((0, 0));
        sheet.insert_columns_after_selected();
        sheet.cursor_col = 4;
        sheet.paste(&mut OfflineClipboard);

        assert_eq!(sheet.get_cell(0, 2), "same value");
        assert_eq!(sheet.get_cell(0, 3), "same value");
        assert_eq!(sheet.get_cell(0, 4), "");
        assert!(sheet.clipboard_message.is_some());
    }

    #[test]
    fn valid_overlapping_cut_moves_values_and_styles() {
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "first".into());
        sheet.set_cell_bold(0, 0, true);
        sheet.set_cell(0, 1, "second".into());
        sheet.selection_anchor = Some((0, 0));
        sheet.cursor_col = 1;
        sheet.clipboard_data = Some(sheet.capture_clipboard(true));

        sheet.paste(&mut OfflineClipboard);

        assert_eq!(sheet.get_cell(0, 0), "");
        assert!(!sheet.get_cell_style(0, 0).bold);
        assert_eq!(sheet.get_cell(0, 1), "first");
        assert!(sheet.get_cell_style(0, 1).bold);
        assert_eq!(sheet.get_cell(0, 2), "second");
        assert!(!sheet.get_cell_style(0, 2).bold);
        assert!(sheet.clipboard_message.is_none());
    }

    #[test]
    fn internal_paste_replaces_full_rectangle_and_keeps_source_styles() {
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "copied".into());
        sheet.set_cell_bold(0, 1, true);
        sheet.selection_anchor = Some((0, 0));
        sheet.cursor_row = 1;
        sheet.cursor_col = 2;
        sheet.clipboard_data = Some(sheet.capture_clipboard(false));

        for row in 3..=5 {
            for col in 4..=7 {
                sheet.set_cell(row, col, "previous".into());
                sheet.set_cell_bold(row, col, true);
            }
        }
        sheet.cursor_row = 3;
        sheet.cursor_col = 4;
        sheet.paste(&mut OfflineClipboard);

        assert_eq!(sheet.get_cell(3, 4), "copied");
        assert_eq!(sheet.get_cell(3, 5), "");
        assert!(sheet.get_cell_style(3, 5).bold);
        for (row, col) in [(3, 6), (4, 4), (4, 5), (4, 6)] {
            assert_eq!(sheet.get_cell(row, col), "");
            assert!(!sheet.get_cell_style(row, col).bold);
        }
        assert!(!sheet.get_cell_style(3, 4).bold);
        assert_eq!(sheet.get_cell(5, 4), "previous");
        assert_eq!(sheet.get_cell(3, 7), "previous");

        sheet.cursor_row = DEFAULT_ROWS - 1;
        sheet.cursor_col = DEFAULT_COLS - 1;
        sheet.paste(&mut OfflineClipboard);
        assert_eq!(sheet.get_cell(DEFAULT_ROWS - 1, DEFAULT_COLS - 1), "copied");
        assert_eq!(sheet.num_rows, DEFAULT_ROWS + 1);
        assert_eq!(sheet.num_cols, DEFAULT_COLS + 2);
    }

    #[test]
    fn external_paste_clears_explicit_empty_fields_and_preserves_formatting() {
        let mut sheet = Spreadsheet::new();
        for row in 0..3 {
            for col in 0..5 {
                sheet.set_cell(row, col, "previous".into());
                sheet.set_cell_bold(row, col, true);
            }
        }

        sheet.paste_text("left\t\tright\t\r\nnext\r\n\r\n");

        assert_eq!(sheet.get_cell(0, 0), "left");
        assert_eq!(sheet.get_cell(0, 2), "right");
        for (row, col) in [(0, 1), (0, 3), (2, 0)] {
            assert_eq!(sheet.get_cell(row, col), "");
            assert!(sheet.get_cell_style(row, col).bold);
        }
        assert_eq!(sheet.get_cell(0, 4), "previous");
        assert_eq!(sheet.get_cell(1, 0), "next");
        assert_eq!(sheet.get_cell(1, 1), "previous");
    }

    #[test]
    fn test_load_from_buffer_simple() {
        let mut sheet = Spreadsheet::new();
        let data = b"hello world\nfoo bar baz";
        
        sheet.load_from_buffer(data, StdinFormat::Auto).unwrap();
        
        assert_eq!(sheet.get_cell(0, 0), "hello");
        assert_eq!(sheet.get_cell(0, 1), "world");
        assert_eq!(sheet.get_cell(1, 0), "foo");
        assert_eq!(sheet.get_cell(1, 1), "bar");
        assert_eq!(sheet.get_cell(1, 2), "baz");
    }

    #[test]
    fn test_load_from_buffer_with_extra_whitespace() {
        let mut sheet = Spreadsheet::new();
        // Multiple spaces and tabs between fields
        let data = b"col1    col2\tcol3\n  value1   value2  ";
        
        sheet.load_from_buffer(data, StdinFormat::Auto).unwrap();
        
        assert_eq!(sheet.get_cell(0, 0), "col1");
        assert_eq!(sheet.get_cell(0, 1), "col2");
        assert_eq!(sheet.get_cell(0, 2), "col3");
        assert_eq!(sheet.get_cell(1, 0), "value1");
        assert_eq!(sheet.get_cell(1, 1), "value2");
    }

    #[test]
    fn test_load_from_buffer_skips_empty_lines() {
        let mut sheet = Spreadsheet::new();
        let data = b"line1\n\n\nline2\n   \nline3";
        
        sheet.load_from_buffer(data, StdinFormat::Auto).unwrap();
        
        assert_eq!(sheet.get_cell(0, 0), "line1");
        assert_eq!(sheet.get_cell(1, 0), "line2");
        assert_eq!(sheet.get_cell(2, 0), "line3");
    }

    #[test]
    fn test_load_from_buffer_ls_output() {
        let mut sheet = Spreadsheet::new();
        // Simulated ls -Al output (simplified)
        let data = b"total 120
drwxr-xr-x  3 user group  96 Jan 23 14:20 .git
-rw-r--r--  1 user group 500 Jan 23 14:20 Cargo.toml";
        
        sheet.load_from_buffer(data, StdinFormat::Auto).unwrap();
        
        // First line: "total 120"
        assert_eq!(sheet.get_cell(0, 0), "total");
        assert_eq!(sheet.get_cell(0, 1), "120");
        
        // Second line: directory entry
        assert_eq!(sheet.get_cell(1, 0), "drwxr-xr-x");
        assert_eq!(sheet.get_cell(1, 1), "3");
        assert_eq!(sheet.get_cell(1, 2), "user");
        
        // Third line: file entry  
        assert_eq!(sheet.get_cell(2, 0), "-rw-r--r--");
    }

    #[test]
    fn test_load_from_buffer_empty() {
        let mut sheet = Spreadsheet::new();
        let data = b"";
        
        sheet.load_from_buffer(data, StdinFormat::Auto).unwrap();
        
        // Should have default dimensions but no data
        assert!(sheet.num_rows >= DEFAULT_ROWS);
        assert!(sheet.num_cols >= DEFAULT_COLS);
        assert_eq!(sheet.get_cell(0, 0), "");
    }

    #[test]
    fn test_load_from_buffer_unicode() {
        let mut sheet = Spreadsheet::new();
        let data = "héllo wörld\n日本語 テスト".as_bytes();
        
        sheet.load_from_buffer(data, StdinFormat::Auto).unwrap();
        
        assert_eq!(sheet.get_cell(0, 0), "héllo");
        assert_eq!(sheet.get_cell(0, 1), "wörld");
        assert_eq!(sheet.get_cell(1, 0), "日本語");
        assert_eq!(sheet.get_cell(1, 1), "テスト");
    }

    #[test]
    fn auto_prefers_tsv_when_both_delimiters_fit() {
        let mut sheet = Spreadsheet::new();
        sheet
            .load_from_buffer(b"a,b\tc,d\nx,y\tz,w\n", StdinFormat::Auto)
            .unwrap();
        assert_eq!(sheet.get_cell(0, 0), "a,b");
        assert_eq!(sheet.get_cell(0, 1), "c,d");
        assert_eq!(sheet.get_cell(1, 0), "x,y");
        assert_eq!(sheet.get_cell(1, 1), "z,w");
    }

    #[test]
    fn whitespace_can_be_selected_when_delimiters_look_consistent() {
        let mut sheet = Spreadsheet::new();
        sheet
            .load_from_buffer(b"first,last extra\n", StdinFormat::Whitespace)
            .unwrap();
        assert_eq!(sheet.get_cell(0, 0), "first,last");
        assert_eq!(sheet.get_cell(0, 1), "extra");
    }

    #[test]
    fn excel_open_keeps_lone_value_at_c5() {
        let mut sheet = Spreadsheet::new();
        sheet
            .load_from_file(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/only-c5.xlsx"
            ))
            .unwrap();

        assert_eq!(sheet.get_cell(4, 2), "42");
        assert_eq!(sheet.get_cell(0, 0), "");
        assert_eq!(sheet.cells.len(), 1);
    }

    #[test]
    fn excel_open_expands_grid_for_offset_value() {
        let mut sheet = Spreadsheet::new();
        sheet
            .load_from_file(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/only-ab105.xlsx"
            ))
            .unwrap();

        assert_eq!(sheet.get_cell(104, 27), "7");
        assert_eq!((sheet.num_rows, sheet.num_cols), (105, 28));
    }

    #[test]
    fn excel_open_replaces_current_sheet_with_empty_worksheet() {
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "before Open".into());
        sheet
            .load_from_file(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/empty-sheet.xlsx"
            ))
            .unwrap();

        assert!(sheet.cells.is_empty());
        assert_eq!((sheet.num_rows, sheet.num_cols), (DEFAULT_ROWS, DEFAULT_COLS));
    }
}
