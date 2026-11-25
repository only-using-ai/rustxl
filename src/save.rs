use std::fs::{self, File, OpenOptions, Permissions};
use std::io;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::spreadsheet::Spreadsheet;
use crate::types::SaveFormat;

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

fn writable_destination_permissions(path: &Path) -> io::Result<Option<Permissions>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => {
            // Rename checks directory permissions. Confirm write access to the file as well.
            let mut options = OpenOptions::new();
            options.write(true);
            #[cfg(unix)]
            options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_NOCTTY);
            let opened = options.open(path)?.metadata()?;
            if !opened.is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Save destination must be a regular file",
                ));
            }
            Ok(Some(opened.permissions()))
        }
        Ok(metadata) if metadata.is_symlink() => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Cannot save over a symbolic link",
        )),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Save destination must be a regular file",
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn create_staging_file(destination: &Path) -> io::Result<(PathBuf, File)> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);

    loop {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = destination.with_file_name(format!(".xl-{}-{id}.tmp", std::process::id()));
        match options.open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
}

fn save_atomically(
    destination: &Path,
    write: impl FnOnce(&mut File) -> io::Result<()>,
) -> io::Result<()> {
    writable_destination_permissions(destination)?;
    let (staging_path, mut staged) = create_staging_file(destination)?;

    let result = (|| {
        write(&mut staged)?;
        if let Some(permissions) = writable_destination_permissions(destination)? {
            #[cfg(windows)]
            if permissions.readonly() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "Save destination is read-only",
                ));
            }
            staged.set_permissions(permissions)?;
        }
        staged.sync_all()
    })();
    drop(staged);

    let result = result.and_then(|()| fs::rename(&staging_path, destination));
    if result.is_err() {
        let _ = fs::remove_file(staging_path);
    }
    result
}

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

        let (extension, delimiter) = match self.save_format {
            SaveFormat::Csv => ("csv", b','),
            SaveFormat::Tsv => ("tsv", b'\t'),
        };

        let filename = format!("{}.{}", self.save_filename, extension);
        save_atomically(Path::new(&filename), |file| {
            let mut writer = csv::WriterBuilder::new()
                .delimiter(delimiter)
                .quote_style(csv::QuoteStyle::Always)
                .from_writer(file);
            for row in 0..=max_row {
                writer.write_record((0..=max_col).map(|col| self.get_cell(row, col)))?;
            }
            writer.flush()
        })?;

        self.save_message = Some(format!("Saved to {}", filename));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{DEFAULT_COLS, DEFAULT_ROWS};
    use crate::spreadsheet::ClipboardData;
    use crate::types::{RowColumnSelectMode, StdinFormat};
    use crate::update::UpdateInfo;
    use std::io::Write;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            loop {
                let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
                let path =
                    std::env::temp_dir().join(format!("xl-test-{}-{id}", std::process::id()));
                match fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("Could not create test directory: {error}"),
                }
            }
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn test_data_bounds() {
        let mut sheet = Spreadsheet::new();
        assert_eq!(sheet.get_data_bounds(), (0, 0));

        sheet.set_cell(5, 3, "test".to_string());
        assert_eq!(sheet.get_data_bounds(), (5, 3));

        sheet.set_cell(2, 10, "test2".to_string());
        assert_eq!(sheet.get_data_bounds(), (5, 10));
    }

    #[test]
    fn failed_open_keeps_current_document() {
        let directory = TestDirectory::new();
        let bad_csv = directory.0.join("bad.csv");
        let bad_excel = directory.0.join("bad.xlsx");
        fs::write(&bad_csv, b"first,valid\nmissing-field\n").unwrap();
        fs::write(&bad_excel, b"invalid workbook").unwrap();

        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "old".into());
        sheet.set_cell_bold(0, 0, true);
        sheet.cursor_row = 5;
        sheet.selection_anchor = Some((4, 0));
        sheet.enter_open_mode();
        sheet.open_filename = "current input".into();

        for path in [bad_csv, bad_excel, directory.0.join("missing.csv")] {
            assert!(sheet.load_from_file(path.to_str().unwrap()).is_err());
            assert_eq!(sheet.cells.len(), 1);
            assert_eq!(sheet.get_cell(0, 0), "old");
            assert!(sheet.get_cell_style(0, 0).bold);
            assert_eq!(sheet.cursor_row, 5);
            assert_eq!(sheet.selection_anchor, Some((4, 0)));
            assert!(sheet.open_mode);
            assert_eq!(sheet.open_filename, "current input");
        }
    }

    #[test]
    fn successful_open_resets_document_state() {
        let directory = TestDirectory::new();
        let path = directory.0.join("data.csv");
        fs::write(&path, b"new,sheet\n").unwrap();

        let mut sheet = Spreadsheet::new();
        sheet.set_cell(DEFAULT_ROWS, DEFAULT_COLS, "old".into());
        sheet.num_rows += 20;
        sheet.num_cols += 20;
        sheet.set_cell_bold(0, 0, true);
        sheet.col_widths.insert(0, 42);
        sheet.row_heights.insert(0, 3);
        sheet.cursor_row = DEFAULT_ROWS;
        sheet.cursor_col = DEFAULT_COLS;
        sheet.scroll_row = 30;
        sheet.scroll_col = 15;
        sheet.selection_anchor = Some((8, 8));
        sheet.visual_mode = true;
        sheet.row_column_select_mode = RowColumnSelectMode::RowSelect;
        sheet.selected_rows = Some((1, 8));
        sheet.editing = true;
        sheet.edit_buffer = "old edit".into();
        sheet.formula_mode = true;
        sheet.selecting_ref = true;
        sheet.ref_anchor = Some((2, 2));
        sheet.formula_autocomplete_active = true;
        sheet.formula_suggestions.push("SUM".into());
        sheet.find_mode = true;
        sheet.find_query = "old".into();
        sheet.find_matches.push((DEFAULT_ROWS, DEFAULT_COLS));
        sheet.clipboard_data = Some(ClipboardData {
            cells: vec![],
            is_cut: true,
            cut_origin: Some((0, 0)),
        });
        sheet.enter_open_mode();
        sheet.open_filename = path.to_str().unwrap().into();
        sheet.save_filename = "old save".into();
        sheet.dark_mode = true;
        sheet.hide_update_prompt = true;
        sheet.update_prompt_shown = true;
        sheet.update_in_progress = true;
        sheet.update_message = Some("updating".into());
        sheet.update_available = Some(UpdateInfo {
            current_version: "1".into(),
            latest_version: "2".into(),
            download_url: String::new(),
            asset_name: String::new(),
        });

        sheet.load_from_file(path.to_str().unwrap()).unwrap();

        assert_eq!(sheet.cells.len(), 2);
        assert_eq!(sheet.get_cell(0, 0), "new");
        assert_eq!(
            (sheet.num_rows, sheet.num_cols),
            (DEFAULT_ROWS, DEFAULT_COLS)
        );
        assert_eq!((sheet.cursor_row, sheet.cursor_col), (0, 0));
        assert_eq!((sheet.scroll_row, sheet.scroll_col), (0, 0));
        assert!(sheet.cell_styles.is_empty());
        assert!(sheet.col_widths.is_empty());
        assert!(sheet.row_heights.is_empty());
        assert!(!sheet.visual_mode);
        assert!(sheet.row_column_select_mode == RowColumnSelectMode::None);
        assert!(sheet.selection_anchor.is_none());
        assert!(sheet.selected_rows.is_none());
        assert!(!sheet.editing && sheet.edit_buffer.is_empty());
        assert!(!sheet.formula_mode && !sheet.selecting_ref && sheet.ref_anchor.is_none());
        assert!(!sheet.formula_autocomplete_active && sheet.formula_suggestions.is_empty());
        assert!(!sheet.find_mode && sheet.find_query.is_empty() && sheet.find_matches.is_empty());
        assert!(sheet.clipboard_data.is_none());
        assert!(!sheet.open_mode && sheet.open_filename.is_empty());
        assert_eq!(sheet.save_filename, "spreadsheet");
        assert!(sheet.dark_mode && sheet.hide_update_prompt);
        assert!(sheet.update_prompt_shown && sheet.update_in_progress);
        assert_eq!(sheet.update_available.unwrap().latest_version, "2");
        assert_eq!(sheet.update_message.as_deref(), Some("updating"));
    }

    #[test]
    fn csv_and_tsv_round_trip_raw_cells() {
        let directory = TestDirectory::new();
        for (format, extension, stdin_format) in [
            (SaveFormat::Csv, "csv", StdinFormat::Csv),
            (SaveFormat::Tsv, "tsv", StdinFormat::Tsv),
        ] {
            let mut original = Spreadsheet::new();
            let base = directory.0.join("data");
            let path = base.with_extension(extension);
            original.save_filename = base.to_str().unwrap().into();
            original.save_format = format;
            original.set_cell(0, 0, "café, \"quoted\"\ttext".into());
            original.set_cell(0, 2, "one\rtwo\nthree\r\nfour".into());
            original.set_cell(2, 0, "=SUM(A1,A2)".into());
            fs::write(&path, b"previous content").unwrap();

            original.save_to_file().unwrap();
            assert!(original.save_message.is_some());

            let mut loaded = Spreadsheet::new();
            loaded.load_from_file(path.to_str().unwrap()).unwrap();
            assert_eq!(loaded.cells, original.cells);

            let data = fs::read(&path).unwrap();
            for mode in [stdin_format, StdinFormat::Auto] {
                let mut piped = Spreadsheet::new();
                piped.load_from_buffer(&data, mode).unwrap();
                assert_eq!(piped.cells, loaded.cells);
                assert_eq!(
                    (piped.num_rows, piped.num_cols),
                    (loaded.num_rows, loaded.num_cols)
                );
            }
        }
    }

    #[test]
    fn delimited_input_errors_match_for_files_and_pipes() {
        let directory = TestDirectory::new();
        for (extension, format, data) in [
            ("csv", StdinFormat::Csv, b"a,b\nx\n".as_slice()),
            ("tsv", StdinFormat::Tsv, b"a\tb\nx\t\xff\n".as_slice()),
        ] {
            let path = directory.0.join("bad").with_extension(extension);
            fs::write(&path, data).unwrap();

            let mut sheet = Spreadsheet::new();
            sheet.set_cell(0, 0, "old value".into());
            let file_error = sheet.load_from_file(path.to_str().unwrap()).unwrap_err();
            let pipe_error = sheet.load_from_buffer(data, format).unwrap_err();
            assert_eq!(pipe_error.kind(), file_error.kind());
            assert_eq!(pipe_error.to_string(), file_error.to_string());
            assert_eq!(sheet.get_cell(0, 0), "old value");
        }
    }

    #[test]
    fn one_column_round_trip_keeps_blank_rows() {
        let directory = TestDirectory::new();
        let base = directory.0.join("single-column");
        let mut original = Spreadsheet::new();
        original.save_filename = base.to_str().unwrap().into();
        original.set_cell(2, 0, "third row".into());
        original.save_to_file().unwrap();

        let mut loaded = Spreadsheet::new();
        loaded
            .load_from_file(base.with_extension("csv").to_str().unwrap())
            .unwrap();
        assert_eq!(loaded.cells, original.cells);
    }

    #[test]
    fn failed_staging_keeps_destination_and_cleans_up() {
        let directory = TestDirectory::new();
        let path = directory.0.join("data.csv");
        fs::write(&path, b"original").unwrap();

        let result = save_atomically(&path, |file| {
            file.write_all(b"partial replacement")?;
            Err(io::Error::other("simulated write failure"))
        });

        assert!(result.is_err());
        assert_eq!(fs::read(path).unwrap(), b"original");
        assert_eq!(fs::read_dir(directory.0.as_path()).unwrap().count(), 1);
    }

    #[test]
    fn destination_that_becomes_a_directory_blocks_replacement() {
        let directory = TestDirectory::new();
        let path = directory.0.join("data.csv");

        let result = save_atomically(&path, |file| {
            file.write_all(b"replacement")?;
            fs::create_dir(&path)
        });

        assert!(result.is_err());
        assert!(path.is_dir());
        assert_eq!(fs::read_dir(directory.0.as_path()).unwrap().count(), 1);
    }

    #[test]
    #[cfg(unix)]
    fn private_file_permissions_are_preserved() {
        use std::os::unix::fs::PermissionsExt;

        let directory = TestDirectory::new();
        let path = directory.0.join("data.csv");
        save_atomically(&path, |file| file.write_all(b"new file")).unwrap();
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o177, 0);

        fs::set_permissions(&path, Permissions::from_mode(0o600)).unwrap();
        save_atomically(&path, |file| file.write_all(b"replacement")).unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(fs::read(&path).unwrap(), b"replacement");
    }

    #[test]
    #[cfg(unix)]
    fn write_denial_keeps_destination_and_cleans_up() {
        use std::os::unix::fs::PermissionsExt;

        let directory = TestDirectory::new();
        let path = directory.0.join("data.csv");
        fs::write(&path, b"original").unwrap();
        fs::set_permissions(&path, Permissions::from_mode(0o444)).unwrap();

        match OpenOptions::new().write(true).open(&path) {
            Ok(_) => return, // A privileged test runner can bypass the mode check.
            Err(error) => assert_eq!(error.kind(), io::ErrorKind::PermissionDenied),
        }
        let error =
            save_atomically(&path, |_| panic!("saved without write permission")).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(fs::read(&path).unwrap(), b"original");
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);

        fs::set_permissions(&path, Permissions::from_mode(0o600)).unwrap();
        let error = save_atomically(&path, |file| {
            file.write_all(b"replacement")?;
            fs::set_permissions(&path, Permissions::from_mode(0o444))
        })
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(fs::read(&path).unwrap(), b"original");
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
    }

    #[test]
    #[cfg(unix)]
    fn save_rejects_existing_and_dangling_symlinks() {
        use std::os::unix::fs::symlink;

        let directory = TestDirectory::new();
        let path = directory.0.join("link.csv");
        let target = directory.0.join("target.csv");
        fs::write(&target, b"original").unwrap();
        symlink(&target, &path).unwrap();
        let mut sheet = Spreadsheet::new();
        sheet.save_filename = directory.0.join("link").to_str().unwrap().into();

        let error = sheet.save_to_file().unwrap_err();
        assert!(error.to_string().contains("symbolic link"));
        assert_eq!(fs::read_link(&path).unwrap(), target);
        assert_eq!(fs::read(&target).unwrap(), b"original");

        fs::remove_file(&target).unwrap();
        assert!(sheet.save_to_file().is_err());
        assert_eq!(fs::read_link(&path).unwrap(), target);
        assert!(!target.exists());
        assert!(sheet.save_message.is_none());
    }
}
