use std::fs::File;
use std::io::{self, Write};

use crate::spreadsheet::Spreadsheet;
use crate::types::SaveFormat;

impl Spreadsheet {
    pub fn enter_save_mode(&mut self) {
        self.save_mode = true;
        self.save_format = SaveFormat::Csv;
        self.save_message = None;
    }

    pub fn exit_save_mode(&mut self) {
        self.save_mode = false;
        self.save_message = None;
    }

    pub fn get_data_bounds(&self) -> (usize, usize) {
        let mut max_row = 0;
        let mut max_col = 0;
        for &(row, col) in self.cells.keys() {
            max_row = max_row.max(row);
            max_col = max_col.max(col);
        }
        (max_row, max_col)
    }

    pub fn save_to_file(&mut self) -> io::Result<()> {
        let (max_row, max_col) = self.get_data_bounds();

        let extension = match self.save_format {
            SaveFormat::Csv => "csv",
            SaveFormat::Tsv => "tsv",
        };
        let separator = match self.save_format {
            SaveFormat::Csv => ',',
            SaveFormat::Tsv => '\t',
        };

        let filename = format!("{}.{}", self.save_filename, extension);
        let mut file = File::create(&filename)?;

        for row in 0..=max_row {
            let mut row_data = Vec::new();
            for col in 0..=max_col {
                let content = self.get_cell(row, col);
                let escaped = if self.save_format == SaveFormat::Csv
                    && (content.contains(',') || content.contains('"') || content.contains('\n'))
                {
                    format!("\"{}\"", content.replace('"', "\"\""))
                } else {
                    content.to_string()
                };
                row_data.push(escaped);
            }
            writeln!(file, "{}", row_data.join(&separator.to_string()))?;
        }

        self.save_message = Some(format!("Saved to {}", filename));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_bounds() {
        let mut sheet = Spreadsheet::new();
        assert_eq!(sheet.get_data_bounds(), (0, 0));

        sheet.set_cell(5, 3, "test".to_string());
        assert_eq!(sheet.get_data_bounds(), (5, 3));

        sheet.set_cell(2, 10, "test2".to_string());
        assert_eq!(sheet.get_data_bounds(), (5, 10));
    }
}
