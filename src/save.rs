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

    pub fn enter_open_mode(&mut self) {
        self.open_mode = true;
        self.open_filename.clear();
        self.open_message = None;
    }

    pub fn exit_open_mode(&mut self) {
        self.open_mode = false;
        self.open_filename.clear();
        self.open_message = None;
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

/// Characterization tests for writing a spreadsheet back out.
///
/// `save_to_file` appends the format's extension to `save_filename` and writes
/// the rectangle from `(0, 0)` to the last populated cell. These tests record
/// the escaping rules and the shape of the output as they behave today.
#[cfg(test)]
mod save_characterization {
    use super::*;

    /// A per-test directory under the system temp dir.
    ///
    /// `save_to_file` builds its path from `save_filename`, so an absolute
    /// prefix keeps the test off the process-wide current directory — which
    /// matters because tests run in parallel.
    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default();
            let path = std::env::temp_dir().join(format!("xl-save-{label}-{unique}"));
            std::fs::create_dir_all(&path).expect("temp dir is creatable");
            Self(path)
        }

        /// Path prefix to hand to `save_filename` (no extension: it is appended).
        fn prefix(&self, stem: &str) -> String {
            self.0.join(stem).to_string_lossy().into_owned()
        }

        fn read(&self, name: &str) -> String {
            std::fs::read_to_string(self.0.join(name)).expect("saved file is readable")
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// CSV quoting kicks in for separators, quotes and newlines; quotes double up.
    #[test]
    fn csv_quotes_only_the_fields_that_need_it() {
        let dir = TempDir::new("csv-quoting");
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "plain".to_string());
        sheet.set_cell(0, 1, "has,comma".to_string());
        sheet.set_cell(0, 2, "has\"quote".to_string());
        sheet.set_cell(0, 3, "has\nnewline".to_string());

        sheet.save_filename = dir.prefix("out");
        sheet.save_format = SaveFormat::Csv;
        sheet.save_to_file().expect("save succeeds");

        assert_eq!(
            dir.read("out.csv"),
            "plain,\"has,comma\",\"has\"\"quote\",\"has\nnewline\"\n"
        );
    }

    /// TSV writes fields verbatim: the quoting branch is CSV-only.
    ///
    /// A value containing a tab would therefore break the column alignment of
    /// the output. That is the behaviour today, recorded rather than endorsed.
    #[test]
    fn tsv_writes_fields_verbatim_without_quoting() {
        let dir = TempDir::new("tsv-verbatim");
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "plain".to_string());
        sheet.set_cell(0, 1, "has,comma".to_string());
        sheet.set_cell(0, 2, "has\"quote".to_string());

        sheet.save_filename = dir.prefix("out");
        sheet.save_format = SaveFormat::Tsv;
        sheet.save_to_file().expect("save succeeds");

        assert_eq!(dir.read("out.tsv"), "plain\thas,comma\thas\"quote\n");
    }

    /// The output is the full rectangle to the last populated cell, gaps included.
    #[test]
    fn empty_cells_inside_the_used_range_become_empty_fields() {
        let dir = TempDir::new("csv-gaps");
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "a".to_string());
        sheet.set_cell(2, 2, "c".to_string());

        sheet.save_filename = dir.prefix("out");
        sheet.save_format = SaveFormat::Csv;
        sheet.save_to_file().expect("save succeeds");

        assert_eq!(dir.read("out.csv"), "a,,\n,,\n,,c\n");
    }

    /// Saving reports the written path back to the user through `save_message`.
    #[test]
    fn saving_records_the_written_path_in_the_status_message() {
        let dir = TempDir::new("csv-message");
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "a".to_string());

        sheet.save_filename = dir.prefix("out");
        sheet.save_format = SaveFormat::Csv;
        sheet.save_to_file().expect("save succeeds");

        let message = sheet.save_message.as_deref().unwrap_or_default();
        assert!(
            message.starts_with("Saved to ") && message.ends_with("out.csv"),
            "unexpected status message: {message:?}"
        );
    }

    /// Only CSV and TSV can be written; there is no workbook writer today.
    ///
    /// This is what makes a round trip lossy: a three-sheet `.xlsx` opened and
    /// saved comes back as a single-sheet text file.
    ///
    /// The match below is exhaustive on purpose. Adding a variant to
    /// `SaveFormat` stops this test from compiling, so a new output format
    /// cannot be introduced without someone revisiting this expectation.
    #[test]
    fn the_only_writable_formats_are_csv_and_tsv() {
        fn extension_for(format: SaveFormat) -> &'static str {
            match format {
                SaveFormat::Csv => "csv",
                SaveFormat::Tsv => "tsv",
            }
        }

        assert_eq!(extension_for(SaveFormat::Csv), "csv");
        assert_eq!(extension_for(SaveFormat::Tsv), "tsv");
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
