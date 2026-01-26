use ratatui::style::Color;

use crate::constants::{
    DEFAULT_COL_WIDTH, DEFAULT_ROW_HEIGHT, MAX_COL_WIDTH, MAX_ROW_HEIGHT,
    MIN_COL_WIDTH, MIN_ROW_HEIGHT,
};
use crate::spreadsheet::Spreadsheet;
use crate::types::{CellStyle, TextAlignment, VerticalAlignment};

impl Spreadsheet {
    pub fn get_col_width(&self, col: usize) -> u16 {
        *self.col_widths.get(&col).unwrap_or(&DEFAULT_COL_WIDTH)
    }

    pub fn get_row_height(&self, row: usize) -> u16 {
        *self.row_heights.get(&row).unwrap_or(&DEFAULT_ROW_HEIGHT)
    }

    pub fn set_col_width(&mut self, col: usize, width: u16) {
        let width = width.clamp(MIN_COL_WIDTH, MAX_COL_WIDTH);
        if width == DEFAULT_COL_WIDTH {
            self.col_widths.remove(&col);
        } else {
            self.col_widths.insert(col, width);
        }
    }

    pub fn set_row_height(&mut self, row: usize, height: u16) {
        let height = height.clamp(MIN_ROW_HEIGHT, MAX_ROW_HEIGHT);
        if height == DEFAULT_ROW_HEIGHT {
            self.row_heights.remove(&row);
        } else {
            self.row_heights.insert(row, height);
        }
    }

    pub fn get_cell_style(&self, row: usize, col: usize) -> CellStyle {
        self.cell_styles.get(&(row, col)).copied().unwrap_or_default()
    }

    pub fn set_cell_fg(&mut self, row: usize, col: usize, color: Option<Color>) {
        let mut style = self.get_cell_style(row, col);
        style.fg = color;
        if style.fg.is_none() && style.bg.is_none() && !style.bold && style.alignment.is_none() && style.vertical_alignment.is_none() {
            self.cell_styles.remove(&(row, col));
        } else {
            self.cell_styles.insert((row, col), style);
        }
    }

    pub fn set_cell_bg(&mut self, row: usize, col: usize, color: Option<Color>) {
        let mut style = self.get_cell_style(row, col);
        style.bg = color;
        if style.fg.is_none() && style.bg.is_none() && !style.bold && style.alignment.is_none() && style.vertical_alignment.is_none() {
            self.cell_styles.remove(&(row, col));
        } else {
            self.cell_styles.insert((row, col), style);
        }
    }

    pub fn set_cell_bold(&mut self, row: usize, col: usize, bold: bool) {
        let mut style = self.get_cell_style(row, col);
        style.bold = bold;
        if style.fg.is_none() && style.bg.is_none() && !style.bold && style.alignment.is_none() && style.vertical_alignment.is_none() {
            self.cell_styles.remove(&(row, col));
        } else {
            self.cell_styles.insert((row, col), style);
        }
    }

    pub fn set_cell_alignment(&mut self, row: usize, col: usize, alignment: Option<TextAlignment>) {
        let mut style = self.get_cell_style(row, col);
        style.alignment = alignment;
        if style.fg.is_none() && style.bg.is_none() && !style.bold && style.alignment.is_none() && style.vertical_alignment.is_none() {
            self.cell_styles.remove(&(row, col));
        } else {
            self.cell_styles.insert((row, col), style);
        }
    }

    pub fn set_cell_vertical_alignment(&mut self, row: usize, col: usize, vertical_alignment: Option<VerticalAlignment>) {
        let mut style = self.get_cell_style(row, col);
        style.vertical_alignment = vertical_alignment;
        if style.fg.is_none() && style.bg.is_none() && !style.bold && style.alignment.is_none() && style.vertical_alignment.is_none() {
            self.cell_styles.remove(&(row, col));
        } else {
            self.cell_styles.insert((row, col), style);
        }
    }

    pub fn format_as_table(&mut self) {
        // Auto-detect table bounds starting from cursor position
        let start_row = self.cursor_row;
        let start_col = self.cursor_col;
        
        // Check if starting cell has data
        if self.get_cell(start_row, start_col).is_empty() {
            return;
        }
        
        // Find the leftmost column with data in the header row (scan left from cursor)
        let mut min_col = start_col;
        for col in (0..=start_col).rev() {
            if !self.get_cell(start_row, col).is_empty() {
                min_col = col;
            } else {
                break;
            }
        }
        
        // Find the rightmost column with data in the header row
        let mut max_col = start_col;
        for col in start_col..self.num_cols {
            if !self.get_cell(start_row, col).is_empty() {
                max_col = col;
            } else {
                break;
            }
        }
        
        // Find the topmost row with data in any column of the table (scan up from cursor)
        let mut min_row = start_row;
        for row in (0..=start_row).rev() {
            let mut has_data = false;
            for col in min_col..=max_col {
                if !self.get_cell(row, col).is_empty() {
                    has_data = true;
                    break;
                }
            }
            if has_data {
                min_row = row;
            } else {
                break;
            }
        }
        
        // Find the bottommost row with data in any column of the table
        let mut max_row = start_row;
        for row in start_row..self.num_rows {
            let mut has_data = false;
            for col in min_col..=max_col {
                if !self.get_cell(row, col).is_empty() {
                    has_data = true;
                    break;
                }
            }
            if has_data {
                max_row = row;
            } else {
                break;
            }
        }
        
        // Auto-select the detected table range
        self.selection_anchor = Some((min_row, min_col));
        self.cursor_row = max_row;
        self.cursor_col = max_col;
        
        // First row is headers - make them bold and solid black
        let header_fg = Color::Rgb(0, 0, 0); // Pure black (RGB 0,0,0)
        for col in min_col..=max_col {
            self.set_cell_bold(min_row, col, true);
            self.set_cell_fg(min_row, col, Some(header_fg));
        }
        
        // Apply alternating row colors (zebra striping)
        // Light gray for even rows, white for odd rows (starting from row after header)
        let header_bg = Color::Rgb(240, 240, 240); // Light gray for header
        let even_row_bg = Color::Rgb(250, 250, 250); // Very light gray for even rows
        let odd_row_bg = Color::White; // White for odd rows
        
        // Set header background
        for col in min_col..=max_col {
            self.set_cell_bg(min_row, col, Some(header_bg));
        }
        
        // Apply zebra striping to data rows
        for row in (min_row + 1)..=max_row {
            let is_even = (row - min_row - 1) % 2 == 0;
            let bg_color = if is_even { even_row_bg } else { odd_row_bg };
            for col in min_col..=max_col {
                self.set_cell_bg(row, col, Some(bg_color));
            }
        }
    }

    pub fn apply_style_to_selection(&mut self, set_fg: Option<Color>, set_bg: Option<Color>) {
        if let Some(((min_row, min_col), (max_row, max_col))) = self.get_selection_range() {
            for row in min_row..=max_row {
                for col in min_col..=max_col {
                    if let Some(color) = set_fg {
                        self.set_cell_fg(row, col, Some(color));
                    }
                    if let Some(color) = set_bg {
                        self.set_cell_bg(row, col, Some(color));
                    }
                }
            }
        } else {
            if let Some(color) = set_fg {
                self.set_cell_fg(self.cursor_row, self.cursor_col, Some(color));
            }
            if let Some(color) = set_bg {
                self.set_cell_bg(self.cursor_row, self.cursor_col, Some(color));
            }
        }
    }

    pub fn apply_alignment_to_selection(&mut self, alignment: Option<TextAlignment>) {
        if let Some(((min_row, min_col), (max_row, max_col))) = self.get_selection_range() {
            for row in min_row..=max_row {
                for col in min_col..=max_col {
                    self.set_cell_alignment(row, col, alignment);
                }
            }
        } else {
            self.set_cell_alignment(self.cursor_row, self.cursor_col, alignment);
        }
    }

    pub fn apply_vertical_alignment_to_selection(&mut self, vertical_alignment: Option<VerticalAlignment>) {
        if let Some(((min_row, min_col), (max_row, max_col))) = self.get_selection_range() {
            for row in min_row..=max_row {
                for col in min_col..=max_col {
                    self.set_cell_vertical_alignment(row, col, vertical_alignment);
                }
            }
        } else {
            self.set_cell_vertical_alignment(self.cursor_row, self.cursor_col, vertical_alignment);
        }
    }

    pub fn clear_formatting_from_selection(&mut self) {
        if let Some(((min_row, min_col), (max_row, max_col))) = self.get_selection_range() {
            for row in min_row..=max_row {
                for col in min_col..=max_col {
                    self.cell_styles.remove(&(row, col));
                }
            }
        } else {
            self.cell_styles.remove(&(self.cursor_row, self.cursor_col));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_col_width() {
        let mut sheet = Spreadsheet::new();
        assert_eq!(sheet.get_col_width(0), DEFAULT_COL_WIDTH);

        sheet.set_col_width(0, 15);
        assert_eq!(sheet.get_col_width(0), 15);

        sheet.set_col_width(0, 2); // below min
        assert_eq!(sheet.get_col_width(0), MIN_COL_WIDTH);

        sheet.set_col_width(0, 100); // above max
        assert_eq!(sheet.get_col_width(0), MAX_COL_WIDTH);
    }

    #[test]
    fn test_cell_style() {
        let mut sheet = Spreadsheet::new();
        let style = sheet.get_cell_style(0, 0);
        assert!(style.fg.is_none());
        assert!(style.bg.is_none());

        sheet.set_cell_fg(0, 0, Some(Color::Red));
        let style = sheet.get_cell_style(0, 0);
        assert_eq!(style.fg, Some(Color::Red));
    }
}
