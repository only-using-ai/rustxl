mod clipboard;
mod constants;
mod formula;
mod formula_ref;
mod input;
mod save;
mod settings;
mod shell;
mod spreadsheet;
mod style;
mod types;
mod ui;
mod update;

use std::io::{self, Read};

use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use spreadsheet::Spreadsheet;
use types::StdinFormat;

#[derive(Parser)]
#[command(name = "xl")]
#[command(about = "A terminal-based spreadsheet application")]
struct Args {
    /// File to open (supports CSV, TSV, and Excel files)
    #[arg(short, long)]
    file: Option<String>,
    /// Format of piped input
    #[arg(long, value_enum, default_value = "auto", conflicts_with = "file")]
    stdin_format: StdinFormat,
    /// Print version information and exit
    #[arg(short = 'V', long = "version")]
    version: bool,
}

impl Args {
    fn load_input(
        &self,
        sheet: &mut Spreadsheet,
        read_stdin: impl FnOnce() -> io::Result<Option<Vec<u8>>>,
    ) -> io::Result<()> {
        if let Some(path) = &self.file {
            return sheet.load_from_file(path).map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!("Error loading file '{path}': {error}"),
                )
            });
        }
        if let Some(data) = read_stdin()? {
            sheet
                .load_from_buffer(&data, self.stdin_format)
                .map_err(|error| {
                    io::Error::new(error.kind(), format!("Error loading stdin: {error}"))
                })?;
        }
        Ok(())
    }
}

/// Reads all data from stdin into a buffer when stdin is piped.
/// Returns the buffer contents, or None if stdin is a TTY.
fn read_piped_stdin() -> io::Result<Option<Vec<u8>>> {
    if atty::is(atty::Stream::Stdin) {
        return Ok(None);
    }

    let mut buffer = Vec::new();
    io::stdin().read_to_end(&mut buffer)?;
    Ok(Some(buffer))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_file_skips_stdin() {
        let args = Args::try_parse_from(["xl", "--file", "spreadsheet.csv"]).unwrap();
        let mut sheet = Spreadsheet::new();
        args.load_input(&mut sheet, || panic!("file input should skip stdin"))
            .unwrap();
        assert_eq!(sheet.get_cell(0, 0), "First");
        assert_eq!(sheet.get_cell(0, 1), "Last");
    }

    #[test]
    fn startup_uses_explicit_stdin_format() {
        let args = Args::try_parse_from(["xl", "--stdin-format", "csv"]).unwrap();
        let mut sheet = Spreadsheet::new();
        args.load_input(&mut sheet, || Ok(Some(b"two words\n".to_vec())))
            .unwrap();
        assert_eq!(sheet.get_cell(0, 0), "two words");

        let error = args
            .load_input(&mut sheet, || Ok(Some(b"first,last\nJane\n".to_vec())))
            .unwrap_err();
        assert!(error.to_string().contains("Error loading stdin"));
        assert_eq!(sheet.get_cell(0, 0), "two words");
    }

    #[test]
    fn default_startup_detects_sample_csv() {
        let args = Args::try_parse_from(["xl"]).unwrap();
        let mut sheet = Spreadsheet::new();
        let data = include_bytes!("../spreadsheet.csv");
        args.load_input(&mut sheet, || Ok(Some(data.to_vec())))
            .unwrap();
        assert_eq!(sheet.get_cell(0, 0), "First");
        assert_eq!(sheet.get_cell(0, 1), "Last");
        assert_eq!(sheet.get_cell(2, 0), "Jane");
        assert_eq!(sheet.get_cell(2, 1), "Smith");
    }

    #[test]
    fn stdin_format_requires_a_valid_value_and_excludes_file() {
        use clap::error::ErrorKind;

        let kind = |args: &[&str]| Args::try_parse_from(args).err().unwrap().kind();
        assert_eq!(
            kind(&["xl", "--stdin-format", "xlsx"]),
            ErrorKind::InvalidValue
        );
        assert_eq!(
            kind(&["xl", "-f", "data.csv", "--stdin-format", "auto"]),
            ErrorKind::ArgumentConflict
        );
    }
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    // Handle version flag
    if args.version {
        println!("{}", constants::VERSION);
        return Ok(());
    }

    // Load input before setting up the interactive terminal.
    let mut spreadsheet = Spreadsheet::new();
    if let Err(error) = args.load_input(&mut spreadsheet, read_piped_stdin) {
        eprintln!("{error}");
        std::process::exit(1);
    }

    // Check if stdout is a TTY (required for terminal UI)
    if !atty::is(atty::Stream::Stdout) {
        eprintln!("Error: stdout must be a terminal to run xl interactively");
        eprintln!("When piping data, xl loads the data but requires a terminal for display");
        std::process::exit(1);
    }

    // Load settings
    let settings = settings::Settings::load();

    spreadsheet.dark_mode = settings.dark_mode;
    spreadsheet.hide_update_prompt = settings.hide_update_prompt;

    // Spawn update checker in background
    let update_rx = update::spawn_update_checker();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run app with pre-loaded spreadsheet and update receiver
    let res = input::run_app(&mut terminal, spreadsheet, update_rx);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {err}");
    }

    Ok(())
}
