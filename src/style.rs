use ratatui::style::Color;

use crate::constants::{
    DEFAULT_COL_WIDTH, DEFAULT_ROW_HEIGHT, MAX_COL_WIDTH, MAX_ROW_HEIGHT,
    MIN_COL_WIDTH, MIN_ROW_HEIGHT,
};
use crate::spreadsheet::Spreadsheet;
use crate::types::CellStyle;

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
        if style.fg.is_none() && style.bg.is_none() {
            self.cell_styles.remove(&(row, col));
        } else {
            self.cell_styles.insert((row, col), style);
        }
    }

    pub fn set_cell_bg(&mut self, row: usize, col: usize, color: Option<Color>) {
        let mut style = self.get_cell_style(row, col);
        style.bg = color;
        if style.fg.is_none() && style.bg.is_none() {
            self.cell_styles.remove(&(row, col));
        } else {
            self.cell_styles.insert((row, col), style);
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
