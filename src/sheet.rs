use std::collections::HashMap;

use crate::constants::{DEFAULT_COLS, DEFAULT_ROWS};
use crate::types::CellStyle;

/// One worksheet: its contents, its column and row sizing, and where the user
/// was looking when they last left it.
///
/// A workbook holds several of these. The sheet the user is editing lives on
/// `Spreadsheet` itself as a working copy; this type is where the others wait,
/// and where the active one is parked when the user switches away. Keeping the
/// cursor and scroll offsets here is what lets a sheet remember its position.
#[derive(Clone)]
pub struct Sheet {
    pub name: String,
    pub cells: HashMap<(usize, usize), String>,
    pub cell_styles: HashMap<(usize, usize), CellStyle>,
    pub col_widths: HashMap<usize, u16>,
    pub row_heights: HashMap<usize, u16>,
    pub num_rows: usize,
    pub num_cols: usize,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub scroll_row: usize,
    pub scroll_col: usize,
}

impl Sheet {
    /// An empty sheet at the default size, with the cursor at A1.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            cells: HashMap::new(),
            cell_styles: HashMap::new(),
            col_widths: HashMap::new(),
            row_heights: HashMap::new(),
            num_rows: DEFAULT_ROWS,
            num_cols: DEFAULT_COLS,
            cursor_row: 0,
            cursor_col: 0,
            scroll_row: 0,
            scroll_col: 0,
        }
    }
}

impl Default for Sheet {
    fn default() -> Self {
        Self::new(DEFAULT_SHEET_NAME)
    }
}

/// The name given to the single sheet of a workbook that has no names of its
/// own — a new file, or one loaded from CSV.
pub const DEFAULT_SHEET_NAME: &str = "Sheet1";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_sheet_is_empty_and_starts_at_a1() {
        let sheet = Sheet::new("Alpha");

        assert_eq!(sheet.name, "Alpha");
        assert!(sheet.cells.is_empty());
        assert_eq!((sheet.cursor_row, sheet.cursor_col), (0, 0));
        assert_eq!((sheet.scroll_row, sheet.scroll_col), (0, 0));
        assert_eq!(sheet.num_rows, DEFAULT_ROWS);
        assert_eq!(sheet.num_cols, DEFAULT_COLS);
    }

    #[test]
    fn the_default_sheet_is_named_sheet1() {
        assert_eq!(Sheet::default().name, DEFAULT_SHEET_NAME);
    }
}
