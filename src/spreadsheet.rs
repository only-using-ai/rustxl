use std::collections::HashMap;

use ratatui::layout::Rect;

use crate::constants::{DEFAULT_COLS, DEFAULT_ROWS};
use crate::types::{CellStyle, SaveFormat, VisualSubMode};

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
        }
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

    pub fn handle_char_input(&mut self, c: char) {
        self.edit_buffer.push(c);

        if c == '=' && self.edit_buffer == "=" {
            self.formula_mode = true;
        }

        if self.formula_mode && c == '(' {
            self.enter_ref_selection_mode();
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
}
