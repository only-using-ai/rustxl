use std::collections::HashMap;

use ratatui::layout::Rect;

use crate::constants::{DEFAULT_COLS, DEFAULT_ROWS};
use crate::hit_test::GridGeometry;
use crate::sheet::Sheet;
use crate::types::{CellStyle, RowColumnSelectMode, SaveFormat, VisualSubMode};
use crate::update::UpdateInfo;

/// Represents copied/cut cell data with relative positions
#[derive(Clone)]
pub struct ClipboardData {
    /// Cell data: ((relative_row, relative_col), value, style)
    pub cells: Vec<((usize, usize), String, Option<CellStyle>)>,
    /// Whether this was a cut operation (cells should be cleared on paste)
    pub is_cut: bool,
    /// Original position of the cut (for clearing on paste)
    pub cut_origin: Option<(usize, usize)>,
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
    // Command mode (vim-style :command)
    pub command_mode: bool,
    pub command_buffer: String,
    pub command_message: Option<String>,
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
    // Workbook
    /// Every sheet in the workbook, including the one being edited.
    ///
    /// The fields above are a working copy of `sheets[active_sheet]`: editing
    /// touches them directly, and the copy is written back when the user
    /// switches sheets. Read `sheets` only through the methods that park the
    /// working copy first, or the active entry will be stale.
    pub sheets: Vec<Sheet>,
    pub active_sheet: usize,
    /// What the grid looked like when it was last drawn.
    ///
    /// Mouse input arrives as a screen position, and only the renderer knows
    /// where each row and column ended up. Recording the geometry as it is drawn
    /// is what lets a click be resolved back to a cell.
    pub grid_geometry: GridGeometry,
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
            command_mode: false,
            command_buffer: String::new(),
            command_message: None,
            update_available: None,
            update_prompt_shown: false,
            update_in_progress: false,
            update_message: None,
            hide_update_prompt: false,
            formula_autocomplete_active: false,
            formula_suggestions: Vec::new(),
            formula_suggestion_index: 0,
            formula_prefix: String::new(),
            sheets: vec![Sheet::default()],
            active_sheet: 0,
            grid_geometry: GridGeometry::default(),
        }
    }

    /// How many sheets the workbook holds. Never zero.
    pub fn sheet_count(&self) -> usize {
        self.sheets.len()
    }

    /// The name of the sheet currently being edited.
    pub fn active_sheet_name(&self) -> &str {
        self.sheets
            .get(self.active_sheet)
            .map(|sheet| sheet.name.as_str())
            .unwrap_or(crate::sheet::DEFAULT_SHEET_NAME)
    }

    /// Writes the working copy back into `sheets[active_sheet]`.
    ///
    /// Copies rather than moves, so the collection is always complete: every
    /// entry holds a usable sheet, and only the active one can lag behind the
    /// edits made since it was checked out. Parking closes that gap.
    fn park_active_sheet(&mut self) {
        let (cells, cell_styles, col_widths, row_heights) = (
            self.cells.clone(),
            self.cell_styles.clone(),
            self.col_widths.clone(),
            self.row_heights.clone(),
        );
        let (num_rows, num_cols) = (self.num_rows, self.num_cols);
        let (cursor_row, cursor_col) = (self.cursor_row, self.cursor_col);
        let (scroll_row, scroll_col) = (self.scroll_row, self.scroll_col);

        let Some(sheet) = self.sheets.get_mut(self.active_sheet) else {
            return;
        };
        sheet.cells = cells;
        sheet.cell_styles = cell_styles;
        sheet.col_widths = col_widths;
        sheet.row_heights = row_heights;
        sheet.num_rows = num_rows;
        sheet.num_cols = num_cols;
        sheet.cursor_row = cursor_row;
        sheet.cursor_col = cursor_col;
        sheet.scroll_row = scroll_row;
        sheet.scroll_col = scroll_col;
    }

    /// Loads `sheets[index]` into the working copy.
    ///
    /// The caller is responsible for having parked the previous sheet first.
    fn checkout_sheet(&mut self, index: usize) {
        let Some(sheet) = self.sheets.get(index) else {
            return;
        };
        self.cells = sheet.cells.clone();
        self.cell_styles = sheet.cell_styles.clone();
        self.col_widths = sheet.col_widths.clone();
        self.row_heights = sheet.row_heights.clone();
        self.num_rows = sheet.num_rows;
        self.num_cols = sheet.num_cols;
        self.cursor_row = sheet.cursor_row;
        self.cursor_col = sheet.cursor_col;
        self.scroll_row = sheet.scroll_row;
        self.scroll_col = sheet.scroll_col;
        self.active_sheet = index;
    }

    /// Switches to another sheet, keeping the current one's edits and position.
    ///
    /// Returns `false` if the index does not name a sheet, or if it is already
    /// the active one, in which case nothing changes.
    pub fn activate_sheet(&mut self, index: usize) -> bool {
        if index >= self.sheets.len() || index == self.active_sheet {
            return false;
        }

        self.park_active_sheet();
        self.checkout_sheet(index);
        self.clear_selection();
        true
    }

    /// Puts the cursor on a cell and starts a fresh selection there.
    ///
    /// Out-of-range coordinates are ignored rather than clamped: a click that
    /// landed outside the sheet should do nothing, not quietly pick the nearest
    /// cell.
    pub fn select_cell(&mut self, row: usize, col: usize) -> bool {
        if row >= self.num_rows || col >= self.num_cols {
            return false;
        }
        self.cursor_row = row;
        self.cursor_col = col;
        self.selection_anchor = None;
        self.row_column_select_mode = RowColumnSelectMode::None;
        self.selected_rows = None;
        self.selected_cols = None;
        true
    }

    /// Grows the selection from its anchor to this cell.
    ///
    /// The anchor is set on the first call so that a drag which begins on the
    /// current cell selects the range it covers.
    pub fn extend_selection_to(&mut self, row: usize, col: usize) -> bool {
        if row >= self.num_rows || col >= self.num_cols {
            return false;
        }
        if self.selection_anchor.is_none() {
            self.selection_anchor = Some((self.cursor_row, self.cursor_col));
        }
        if (self.cursor_row, self.cursor_col) == (row, col) {
            return false;
        }
        self.cursor_row = row;
        self.cursor_col = col;
        true
    }

    /// Selects a whole column, as clicking its letter does in a spreadsheet.
    pub fn select_whole_column(&mut self, col: usize) -> bool {
        if col >= self.num_cols {
            return false;
        }
        self.cursor_col = col;
        self.selection_anchor = None;
        self.selected_rows = None;
        self.selected_cols = Some((col, col));
        self.row_column_select_mode = RowColumnSelectMode::ColumnSelect;
        true
    }

    /// Selects a whole row, as clicking its number does in a spreadsheet.
    pub fn select_whole_row(&mut self, row: usize) -> bool {
        if row >= self.num_rows {
            return false;
        }
        self.cursor_row = row;
        self.selection_anchor = None;
        self.selected_cols = None;
        self.selected_rows = Some((row, row));
        self.row_column_select_mode = RowColumnSelectMode::RowSelect;
        true
    }

    /// Scrolls the viewport by `delta` rows, carrying the cursor with it.
    ///
    /// The cursor has to move too. Every frame ends with `adjust_scroll`, which
    /// pulls the viewport back until the cursor is inside it — so a wheel event
    /// that moved the view and left the cursor behind would be undone before
    /// the user saw anything happen. Moving both keeps that adjustment a no-op.
    ///
    /// The offset is clamped to the sheet so the grid cannot be scrolled off
    /// its own end.
    pub fn scroll_grid_vertically(&mut self, delta: isize) -> bool {
        let last_row = self.num_rows.saturating_sub(1);
        let target = self.scroll_row.saturating_add_signed(delta).min(last_row);
        if target == self.scroll_row {
            return false;
        }

        let moved = target as isize - self.scroll_row as isize;
        self.scroll_row = target;
        self.cursor_row = self.cursor_row.saturating_add_signed(moved).min(last_row);
        true
    }

    /// Scrolls the viewport by `delta` columns, carrying the cursor with it.
    ///
    /// Same reason as the vertical case: the renderer keeps the cursor on
    /// screen, so the view cannot be moved away from it.
    pub fn scroll_grid_horizontally(&mut self, delta: isize) -> bool {
        let last_col = self.num_cols.saturating_sub(1);
        let target = self.scroll_col.saturating_add_signed(delta).min(last_col);
        if target == self.scroll_col {
            return false;
        }

        let moved = target as isize - self.scroll_col as isize;
        self.scroll_col = target;
        self.cursor_col = self.cursor_col.saturating_add_signed(moved).min(last_col);
        true
    }

    /// Moves to the next sheet, wrapping round to the first one at the end.
    ///
    /// Wrapping keeps the key useful on the last sheet: in a workbook of two or
    /// three sheets, cycling is what the user is doing anyway.
    pub fn next_sheet(&mut self) -> bool {
        if self.sheets.len() < 2 {
            return false;
        }
        let next = (self.active_sheet + 1) % self.sheets.len();
        self.activate_sheet(next)
    }

    /// Moves to the previous sheet, wrapping round to the last one at the start.
    pub fn previous_sheet(&mut self) -> bool {
        if self.sheets.len() < 2 {
            return false;
        }
        let previous = if self.active_sheet == 0 {
            self.sheets.len() - 1
        } else {
            self.active_sheet - 1
        };
        self.activate_sheet(previous)
    }

    /// How the active sheet should be labelled on screen.
    ///
    /// A workbook with a single sheet says just its name — the position would be
    /// noise. With more than one, the position is what the user needs to know.
    pub fn sheet_indicator(&self) -> String {
        let count = self.sheet_count();
        if count < 2 {
            return self.active_sheet_name().to_string();
        }
        format!(
            "{} ({}/{})",
            self.active_sheet_name(),
            self.active_sheet + 1,
            count
        )
    }

    /// Makes whatever is in the grid the workbook's only sheet.
    ///
    /// CSV, TSV and piped input have no notion of sheets, so they load straight
    /// into the grid. Without this the sheets of a workbook opened earlier would
    /// stay behind, and switching sheets would show a file that is no longer
    /// open.
    fn adopt_grid_as_only_sheet(&mut self, name: &str) {
        self.sheets = vec![Sheet::new(name)];
        self.active_sheet = 0;
        self.park_active_sheet();
    }

    /// Replaces the workbook with `sheets`, showing the first one.
    ///
    /// Used by the loaders. An empty list is ignored: a workbook always has at
    /// least one sheet.
    pub fn replace_sheets(&mut self, sheets: Vec<Sheet>) {
        if sheets.is_empty() {
            return;
        }

        self.sheets = sheets;
        self.active_sheet = 0;
        self.checkout_sheet(0);
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
    pub fn copy_selection(&mut self) {
        self.copy_or_cut_selection(false);
    }

    /// Cut the current selection (or single cell) - copies and marks for deletion on paste
    pub fn cut_selection(&mut self) {
        self.copy_or_cut_selection(true);
    }

    fn copy_or_cut_selection(&mut self, is_cut: bool) {
        // Determine the range to copy
        let (min_row, min_col, max_row, max_col) = if let Some(((r1, c1), (r2, c2))) = self.get_selection_range() {
            (r1, c1, r2, c2)
        } else if let Some((min_row, max_row)) = self.selected_rows {
            (min_row, 0, max_row, self.num_cols - 1)
        } else if let Some((min_col, max_col)) = self.selected_cols {
            (0, min_col, self.num_rows - 1, max_col)
        } else {
            // Single cell
            (self.cursor_row, self.cursor_col, self.cursor_row, self.cursor_col)
        };

        // Collect cell data with relative positions
        let mut cells_data = Vec::new();
        for row in min_row..=max_row {
            for col in min_col..=max_col {
                let rel_row = row - min_row;
                let rel_col = col - min_col;
                let value = self.get_cell(row, col).to_string();
                let style = self.cell_styles.get(&(row, col)).copied();
                
                // Only include non-empty cells or cells with styles
                if !value.is_empty() || style.is_some() {
                    cells_data.push(((rel_row, rel_col), value, style));
                }
            }
        }

        // Build tab-separated text for system clipboard
        let mut clipboard_text = String::new();
        for row in min_row..=max_row {
            let mut row_values = Vec::new();
            for col in min_col..=max_col {
                row_values.push(self.get_cell(row, col).to_string());
            }
            if row > min_row {
                clipboard_text.push('\n');
            }
            clipboard_text.push_str(&row_values.join("\t"));
        }

        // Try to set system clipboard
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text(&clipboard_text);
        }

        // Store internal clipboard data
        self.clipboard_data = Some(ClipboardData {
            cells: cells_data,
            is_cut,
            cut_origin: if is_cut { Some((min_row, min_col)) } else { None },
        });
    }

    /// Paste from clipboard at current cursor position
    pub fn paste(&mut self) {
        // First, try to use internal clipboard data if available
        if let Some(clipboard_data) = self.clipboard_data.take() {
            self.paste_internal(clipboard_data);
            return;
        }

        // Fall back to system clipboard
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            if let Ok(text) = clipboard.get_text() {
                self.paste_text(&text);
            }
        }
    }

    fn paste_internal(&mut self, clipboard_data: ClipboardData) {
        let dest_row = self.cursor_row;
        let dest_col = self.cursor_col;

        // If this was a cut operation, clear the original cells first
        if clipboard_data.is_cut {
            if let Some((orig_row, orig_col)) = clipboard_data.cut_origin {
                for ((rel_row, rel_col), _, _) in &clipboard_data.cells {
                    let src_row = orig_row + rel_row;
                    let src_col = orig_col + rel_col;
                    self.cells.remove(&(src_row, src_col));
                    self.cell_styles.remove(&(src_row, src_col));
                }
            }
        }

        // Paste cells at new position
        for ((rel_row, rel_col), value, style) in &clipboard_data.cells {
            let new_row = dest_row + rel_row;
            let new_col = dest_col + rel_col;

            // Expand grid if needed
            if new_row >= self.num_rows {
                self.num_rows = new_row + 1;
            }
            if new_col >= self.num_cols {
                self.num_cols = new_col + 1;
            }

            // Set cell value
            if !value.is_empty() {
                self.cells.insert((new_row, new_col), value.clone());
            }

            // Set cell style
            if let Some(s) = style {
                self.cell_styles.insert((new_row, new_col), *s);
            }
        }

        // Re-store clipboard data for multiple pastes (but mark as copy, not cut)
        self.clipboard_data = Some(ClipboardData {
            cells: clipboard_data.cells,
            is_cut: false,
            cut_origin: None,
        });

        self.clear_selection();
    }

    fn paste_text(&mut self, text: &str) {
        let dest_row = self.cursor_row;
        let dest_col = self.cursor_col;

        for (row_offset, line) in text.lines().enumerate() {
            let new_row = dest_row + row_offset;
            
            // Expand rows if needed
            if new_row >= self.num_rows {
                self.num_rows = new_row + 1;
            }

            // Split by tabs (Excel/spreadsheet format)
            let values: Vec<&str> = line.split('\t').collect();
            
            for (col_offset, value) in values.iter().enumerate() {
                let new_col = dest_col + col_offset;
                
                // Expand columns if needed
                if new_col >= self.num_cols {
                    self.num_cols = new_col + 1;
                }

                if !value.is_empty() {
                    self.cells.insert((new_row, new_col), value.to_string());
                }
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
    pub fn get_selection_stats(&mut self) -> Option<(usize, usize, usize, f64)> {
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

        for row in min_row..=max_row {
            for col in min_col..=max_col {
                let content = self.get_cell(row, col);
                if !content.is_empty() {
                    cell_count += 1;
                    rows_with_data.insert(row);
                    // Evaluate the cell to get the actual value (handles formulas)
                    let evaluated = self.evaluate_cell(row, col);
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
            // Delete rows from bottom to top to avoid index shifting issues
            for row in (min_row..=max_row).rev() {
                self.delete_row(row);
            }
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
            // Delete columns from right to left to avoid index shifting issues
            for col in (min_col..=max_col).rev() {
                self.delete_column(col);
            }
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
            let count = max_row - min_row + 1;
            // Insert after the last selected row
            let insert_after = max_row;
            for _ in 0..count {
                self.insert_row_after(insert_after);
            }
            // Move cursor to the first newly inserted row
            self.cursor_row = insert_after + 1;
            self.selected_rows = Some((insert_after + 1, insert_after + count));
        }
    }

    pub fn insert_columns_after_selected(&mut self) {
        if let Some((min_col, max_col)) = self.selected_cols {
            let count = max_col - min_col + 1;
            // Insert after the last selected column
            let insert_after = max_col;
            for _ in 0..count {
                self.insert_column_after(insert_after);
            }
            // Move cursor to the first newly inserted column
            self.cursor_col = insert_after + 1;
            self.selected_cols = Some((insert_after + 1, insert_after + count));
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

        match extension.as_str() {
            "csv" => self.load_csv(filepath),
            "tsv" => self.load_tsv(filepath),
            "xlsx" | "xls" => self.load_excel(filepath),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Unsupported file format: {}", extension),
            )),
        }
    }

    fn load_csv(&mut self, filepath: &str) -> std::io::Result<()> {
        self.load_delimited(filepath, b',')
    }

    fn load_tsv(&mut self, filepath: &str) -> std::io::Result<()> {
        self.load_delimited(filepath, b'\t')
    }

    fn load_delimited(&mut self, filepath: &str, delimiter: u8) -> std::io::Result<()> {
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(delimiter)
            .has_headers(false)
            .from_path(filepath)?;

        self.cells.clear();
        let mut row_idx = 0;

        for result in reader.records() {
            let record = result?;
            for (col_idx, field) in record.iter().enumerate() {
                if !field.is_empty() {
                    self.set_cell(row_idx, col_idx, field.to_string());
                }
            }
            row_idx += 1;
        }

        // Update dimensions based on loaded data
        let (max_row, max_col) = self.get_data_bounds();
        self.num_rows = (max_row + 1).max(DEFAULT_ROWS);
        self.num_cols = (max_col + 1).max(DEFAULT_COLS);
        self.adopt_grid_as_only_sheet(crate::sheet::DEFAULT_SHEET_NAME);

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

        // Read every worksheet. A workbook that only surrendered its first
        // sheet would lose the rest of the document the moment it was saved.
        let mut sheets = Vec::with_capacity(sheet_names.len());
        for name in &sheet_names {
            let range = workbook
                .worksheet_range(name)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

            let mut sheet = Sheet::new(name.clone());
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
                        sheet.cells.insert((row_idx, col_idx), value);
                    }
                }
            }

            // Size each sheet to its own contents.
            let (max_row, max_col) = sheet
                .cells
                .keys()
                .fold((0, 0), |(r, c), &(row, col)| (r.max(row), c.max(col)));
            sheet.num_rows = (max_row + 1).max(DEFAULT_ROWS);
            sheet.num_cols = (max_col + 1).max(DEFAULT_COLS);

            sheets.push(sheet);
        }

        self.replace_sheets(sheets);

        Ok(())
    }

    /// Load spreadsheet data from a byte buffer (e.g., from piped stdin)
    pub fn load_from_buffer(&mut self, buffer: &[u8]) -> std::io::Result<()> {
        // Convert to string for processing
        let buffer_str = String::from_utf8_lossy(buffer);
        
        self.cells.clear();
        let mut row_idx = 0;
        
        // Process the buffered data line by line
        for line in buffer_str.lines() {
            let trimmed = line.trim();
            
            // Skip empty lines
            if trimmed.is_empty() {
                continue;
            }
            
            // Split by whitespace (handles multiple spaces/tabs)
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            
            for (col_idx, part) in parts.iter().enumerate() {
                if !part.is_empty() {
                    self.set_cell(row_idx, col_idx, part.to_string());
                }
            }
            
            row_idx += 1;
        }
        
        // Update dimensions based on loaded data
        let (max_row, max_col) = self.get_data_bounds();
        self.num_rows = (max_row + 1).max(DEFAULT_ROWS);
        self.num_cols = (max_col + 1).max(DEFAULT_COLS);
        self.adopt_grid_as_only_sheet(crate::sheet::DEFAULT_SHEET_NAME);
        
        Ok(())
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
    fn test_load_from_buffer_simple() {
        let mut sheet = Spreadsheet::new();
        let data = b"hello world\nfoo bar baz";
        
        sheet.load_from_buffer(data).unwrap();
        
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
        
        sheet.load_from_buffer(data).unwrap();
        
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
        
        sheet.load_from_buffer(data).unwrap();
        
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
        
        sheet.load_from_buffer(data).unwrap();
        
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
        
        sheet.load_from_buffer(data).unwrap();
        
        // Should have default dimensions but no data
        assert!(sheet.num_rows >= DEFAULT_ROWS);
        assert!(sheet.num_cols >= DEFAULT_COLS);
        assert_eq!(sheet.get_cell(0, 0), "");
    }

    #[test]
    fn test_load_from_buffer_unicode() {
        let mut sheet = Spreadsheet::new();
        let data = "héllo wörld\n日本語 テスト".as_bytes();
        
        sheet.load_from_buffer(data).unwrap();
        
        assert_eq!(sheet.get_cell(0, 0), "héllo");
        assert_eq!(sheet.get_cell(0, 1), "wörld");
        assert_eq!(sheet.get_cell(1, 0), "日本語");
        assert_eq!(sheet.get_cell(1, 1), "テスト");
    }
}

/// Characterization tests for workbook loading.
///
/// These tests pin down what `load_from_file` does *today* so that later changes
/// to the loader are deliberate rather than accidental. They are deliberately
/// descriptive: where current behaviour loses data, the test records the loss
/// instead of asserting the behaviour we would prefer.
///
/// Fixture: `tests/fixtures/three_sheets.xlsx` (see `tests/fixtures/README.md`).
/// Three worksheets, formulas with cached results, a boolean, a date and a float.
#[cfg(test)]
mod workbook_loading_characterization {
    use super::*;

    fn fixture(name: &str) -> String {
        format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name)
    }

    /// Every worksheet is loaded, and the first one is the one on screen.
    ///
    /// This test used to record the opposite: `load_excel` read all the sheet
    /// names and then kept only `sheet_names[0]`, so a three-sheet workbook was
    /// silently reduced to one and saving it discarded two thirds of the
    /// document. The loader now keeps them all.
    #[test]
    fn loading_a_multi_sheet_workbook_keeps_every_sheet() {
        let mut sheet = Spreadsheet::new();
        sheet
            .load_from_file(&fixture("three_sheets.xlsx"))
            .expect("fixture loads");

        assert_eq!(sheet.sheet_count(), 3);
        assert_eq!(
            {
                sheet.park_active_sheet();
                sheet.sheets.iter()
            }
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Alpha", "Beta", "Gamma"],
            "sheets keep their workbook order and names"
        );

        // Sheet "Alpha" (the first) is the one being edited.
        assert_eq!(sheet.active_sheet_name(), "Alpha");
        assert_eq!(sheet.get_cell(0, 0), "Name");
        assert_eq!(sheet.get_cell(0, 1), "Qty");
        assert_eq!(sheet.get_cell(1, 0), "widget");
        assert_eq!(sheet.get_cell(2, 0), "gadget");

        // The other sheets are loaded but not on screen.
        let on_screen: Vec<&str> = sheet.cells.values().map(String::as_str).collect();
        assert!(
            !on_screen.contains(&"flag"),
            "only the active sheet belongs in the grid: {on_screen:?}"
        );
    }

    /// The sheets that are not on screen still carry their own contents.
    #[test]
    fn the_sheets_behind_the_active_one_hold_their_own_data() {
        let mut sheet = Spreadsheet::new();
        sheet
            .load_from_file(&fixture("three_sheets.xlsx"))
            .expect("fixture loads");

        assert!(sheet.activate_sheet(1), "Beta exists");
        assert_eq!(sheet.active_sheet_name(), "Beta");
        assert_eq!(sheet.get_cell(0, 0), "flag");
        assert_eq!(sheet.get_cell(0, 1), "when");

        assert!(sheet.activate_sheet(2), "Gamma exists");
        assert_eq!(sheet.active_sheet_name(), "Gamma");
        assert_eq!(sheet.get_cell(0, 0), "cross");
    }

    /// Each sheet is sized to its own contents, not to the largest one.
    #[test]
    fn every_sheet_is_sized_independently() {
        let mut sheet = Spreadsheet::new();
        sheet
            .load_from_file(&fixture("three_sheets.xlsx"))
            .expect("fixture loads");

        sheet.park_active_sheet();
        for loaded in &sheet.sheets {
            assert!(
                loaded.num_rows >= DEFAULT_ROWS && loaded.num_cols >= DEFAULT_COLS,
                "sheet {} fell below the default size",
                loaded.name
            );
        }
    }

    /// Numbers arrive as their cached display text, not as typed values.
    ///
    /// Integral floats lose the decimal point (`10.0` becomes `"10"`), which is
    /// what the grid stores and what a later CSV save writes out.
    #[test]
    fn integral_numbers_are_stored_without_a_decimal_point() {
        let mut sheet = Spreadsheet::new();
        sheet
            .load_from_file(&fixture("three_sheets.xlsx"))
            .expect("fixture loads");

        assert_eq!(sheet.get_cell(1, 1), "10");
        assert_eq!(sheet.get_cell(2, 1), "20");
    }

    /// Formula cells contribute their cached result, never the formula text.
    ///
    /// `Alpha!C2` holds `=SUM(B2:B3)` with a cached value of 30. The loader sees
    /// only the cached number, so the formula itself is not recoverable from the
    /// grid — editing and saving the file would replace it with a constant.
    #[test]
    fn formula_cells_load_as_their_cached_result() {
        let mut sheet = Spreadsheet::new();
        sheet
            .load_from_file(&fixture("three_sheets.xlsx"))
            .expect("fixture loads");

        assert_eq!(sheet.get_cell(1, 2), "30");
    }

    /// Dimensions grow to fit the data but never shrink below the defaults.
    #[test]
    fn dimensions_are_at_least_the_defaults_after_loading() {
        let mut sheet = Spreadsheet::new();
        sheet
            .load_from_file(&fixture("three_sheets.xlsx"))
            .expect("fixture loads");

        assert!(sheet.num_rows >= DEFAULT_ROWS);
        assert!(sheet.num_cols >= DEFAULT_COLS);
    }

    /// A sheetless format replaces the whole workbook, not just the grid.
    ///
    /// Opening a workbook and then a CSV used to leave the workbook's other
    /// sheets in place, so switching sheets showed data from a file that was no
    /// longer open.
    #[test]
    fn loading_a_csv_leaves_the_workbook_with_one_sheet() {
        let mut sheet = Spreadsheet::new();
        sheet
            .load_from_file(&fixture("three_sheets.xlsx"))
            .expect("fixture loads");
        assert_eq!(sheet.sheet_count(), 3, "workbook is open");

        let csv = std::env::temp_dir().join(format!(
            "xl-single-sheet-{}.csv",
            std::process::id()
        ));
        std::fs::write(&csv, "a,b\n1,2\n").expect("temp csv is writable");
        sheet
            .load_from_file(&csv.to_string_lossy())
            .expect("csv loads");
        let _ = std::fs::remove_file(&csv);

        assert_eq!(sheet.sheet_count(), 1);
        assert_eq!(sheet.active_sheet, 0);
        assert_eq!(sheet.get_cell(0, 0), "a");
    }

    /// Loading replaces the previous contents rather than merging into them.
    #[test]
    fn loading_clears_any_previously_held_cells() {
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(50, 20, "stale".to_string());

        sheet
            .load_from_file(&fixture("three_sheets.xlsx"))
            .expect("fixture loads");

        assert_eq!(sheet.get_cell(50, 20), "");
    }

    /// An unknown extension is rejected before any file access happens.
    #[test]
    fn unsupported_extensions_are_rejected() {
        let mut sheet = Spreadsheet::new();
        let err = sheet
            .load_from_file("/nonexistent/data.ods")
            .expect_err("ods is not supported");

        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        assert!(err.to_string().contains("Unsupported file format"));
    }
}

#[cfg(test)]
mod workbook_sheets {
    use super::*;

    fn named(name: &str) -> Sheet {
        Sheet::new(name)
    }

    #[test]
    fn a_new_workbook_holds_one_sheet() {
        let sheet = Spreadsheet::new();

        assert_eq!(sheet.sheet_count(), 1);
        assert_eq!(sheet.active_sheet, 0);
        assert_eq!(sheet.active_sheet_name(), crate::sheet::DEFAULT_SHEET_NAME);
    }

    #[test]
    fn switching_to_the_current_or_a_missing_sheet_changes_nothing() {
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "kept".to_string());

        assert!(!sheet.activate_sheet(0), "already on sheet 0");
        assert!(!sheet.activate_sheet(7), "sheet 7 does not exist");

        assert_eq!(sheet.get_cell(0, 0), "kept");
        assert_eq!(sheet.active_sheet, 0);
    }

    #[test]
    fn replacing_the_sheets_shows_the_first_one() {
        let mut sheet = Spreadsheet::new();
        let mut alpha = named("Alpha");
        alpha.cells.insert((0, 0), "first".to_string());
        let mut beta = named("Beta");
        beta.cells.insert((0, 0), "second".to_string());

        sheet.replace_sheets(vec![alpha, beta]);

        assert_eq!(sheet.sheet_count(), 2);
        assert_eq!(sheet.active_sheet_name(), "Alpha");
        assert_eq!(sheet.get_cell(0, 0), "first");
    }

    #[test]
    fn an_empty_replacement_is_ignored_because_a_workbook_always_has_a_sheet() {
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "kept".to_string());

        sheet.replace_sheets(Vec::new());

        assert_eq!(sheet.sheet_count(), 1);
        assert_eq!(sheet.get_cell(0, 0), "kept");
    }

    #[test]
    fn edits_survive_a_trip_to_another_sheet_and_back() {
        let mut sheet = Spreadsheet::new();
        sheet.replace_sheets(vec![named("Alpha"), named("Beta")]);

        sheet.set_cell(1, 1, "written on Alpha".to_string());
        assert!(sheet.activate_sheet(1));
        assert_eq!(sheet.get_cell(1, 1), "", "Beta starts empty");

        sheet.set_cell(2, 2, "written on Beta".to_string());
        assert!(sheet.activate_sheet(0));

        assert_eq!(sheet.get_cell(1, 1), "written on Alpha");
        assert_eq!(sheet.get_cell(2, 2), "", "Beta's edit stayed on Beta");
    }

    #[test]
    fn each_sheet_remembers_where_the_cursor_was() {
        let mut sheet = Spreadsheet::new();
        sheet.replace_sheets(vec![named("Alpha"), named("Beta")]);

        sheet.cursor_row = 4;
        sheet.cursor_col = 2;
        sheet.scroll_row = 3;

        assert!(sheet.activate_sheet(1));
        assert_eq!(
            (sheet.cursor_row, sheet.cursor_col, sheet.scroll_row),
            (0, 0, 0),
            "a freshly visited sheet starts at A1"
        );

        sheet.cursor_row = 9;
        assert!(sheet.activate_sheet(0));

        assert_eq!((sheet.cursor_row, sheet.cursor_col), (4, 2));
        assert_eq!(sheet.scroll_row, 3);

        assert!(sheet.activate_sheet(1));
        assert_eq!(sheet.cursor_row, 9, "Beta kept its own cursor");
    }

    #[test]
    fn the_sheet_collection_includes_edits_that_have_not_been_switched_away_from() {
        let mut sheet = Spreadsheet::new();
        sheet.replace_sheets(vec![named("Alpha"), named("Beta")]);
        sheet.set_cell(0, 0, "unsaved".to_string());

        sheet.park_active_sheet();
        let sheets = &sheet.sheets;

        assert_eq!(sheets.len(), 2);
        assert_eq!(
            sheets[0].cells.get(&(0, 0)).map(String::as_str),
            Some("unsaved"),
            "the active sheet must not be stale when the collection is read"
        );
    }

    #[test]
    fn reading_the_collection_leaves_the_grid_usable() {
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "before".to_string());

        sheet.park_active_sheet();

        assert_eq!(sheet.get_cell(0, 0), "before");
        sheet.set_cell(0, 1, "after".to_string());
        assert_eq!(sheet.get_cell(0, 1), "after");
    }

    #[test]
    fn switching_sheets_drops_a_selection_that_belonged_to_the_old_sheet() {
        let mut sheet = Spreadsheet::new();
        sheet.replace_sheets(vec![named("Alpha"), named("Beta")]);
        sheet.selection_anchor = Some((0, 0));

        assert!(sheet.activate_sheet(1));

        assert_eq!(sheet.selection_anchor, None);
    }
}

#[cfg(test)]
mod sheet_navigation {
    use super::*;
    use crate::sheet::Sheet;

    fn workbook(names: &[&str]) -> Spreadsheet {
        let mut sheet = Spreadsheet::new();
        sheet.replace_sheets(names.iter().map(|n| Sheet::new(*n)).collect());
        sheet
    }

    #[test]
    fn moving_forward_wraps_round_to_the_first_sheet() {
        let mut sheet = workbook(&["Alpha", "Beta", "Gamma"]);

        assert!(sheet.next_sheet());
        assert_eq!(sheet.active_sheet_name(), "Beta");
        assert!(sheet.next_sheet());
        assert_eq!(sheet.active_sheet_name(), "Gamma");
        assert!(sheet.next_sheet());
        assert_eq!(sheet.active_sheet_name(), "Alpha", "wrapped round");
    }

    #[test]
    fn moving_backward_wraps_round_to_the_last_sheet() {
        let mut sheet = workbook(&["Alpha", "Beta", "Gamma"]);

        assert!(sheet.previous_sheet());
        assert_eq!(sheet.active_sheet_name(), "Gamma", "wrapped round");
        assert!(sheet.previous_sheet());
        assert_eq!(sheet.active_sheet_name(), "Beta");
    }

    #[test]
    fn navigation_does_nothing_in_a_single_sheet_workbook() {
        let mut sheet = Spreadsheet::new();

        assert!(!sheet.next_sheet());
        assert!(!sheet.previous_sheet());
        assert_eq!(sheet.active_sheet, 0);
    }

    #[test]
    fn the_indicator_shows_the_position_only_when_there_is_more_than_one_sheet() {
        let single = Spreadsheet::new();
        assert_eq!(single.sheet_indicator(), crate::sheet::DEFAULT_SHEET_NAME);

        let mut many = workbook(&["Alpha", "Beta", "Gamma"]);
        assert_eq!(many.sheet_indicator(), "Alpha (1/3)");
        many.next_sheet();
        assert_eq!(many.sheet_indicator(), "Beta (2/3)");
    }
}
