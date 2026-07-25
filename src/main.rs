mod constants;
mod formula;
mod input;
mod save;
mod settings;
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
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

#[derive(Parser, Debug)]
#[command(name = "xl")]
#[command(about = "A terminal-based spreadsheet application")]
struct Args {
    /// File to open (supports CSV, TSV, and Excel files)
    #[arg(value_name = "FILE")]
    path: Option<String>,
    /// File to open (supports CSV, TSV, and Excel files)
    #[arg(short, long, value_name = "FILE", conflicts_with = "path")]
    file: Option<String>,
    /// Print version information and exit
    #[arg(short = 'V', long = "version")]
    version: bool,
}

impl Args {
    /// The file to open, whether it was given positionally or through `--file`.
    ///
    /// The two are mutually exclusive at the parser level, so at most one of
    /// them can be set here.
    fn file_to_open(&self) -> Option<&str> {
        self.path.as_deref().or(self.file.as_deref())
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
mod argument_parsing {
    use super::*;

    fn parse(args: &[&str]) -> Result<Args, clap::Error> {
        Args::try_parse_from(args)
    }

    #[test]
    fn a_positional_path_selects_the_file_to_open() {
        let args = parse(&["xl", "data.xlsx"]).expect("a bare path is accepted");
        assert_eq!(args.file_to_open(), Some("data.xlsx"));
    }

    #[test]
    fn the_file_flag_still_works() {
        let short = parse(&["xl", "-f", "data.csv"]).expect("-f is accepted");
        assert_eq!(short.file_to_open(), Some("data.csv"));

        let long = parse(&["xl", "--file", "data.csv"]).expect("--file is accepted");
        assert_eq!(long.file_to_open(), Some("data.csv"));
    }

    #[test]
    fn giving_both_forms_is_rejected_with_a_conflict_error() {
        let error = parse(&["xl", "one.csv", "-f", "two.csv"])
            .expect_err("the two forms are mutually exclusive");

        assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn no_argument_means_no_file_to_open() {
        let args = parse(&["xl"]).expect("running without arguments is allowed");
        assert_eq!(args.file_to_open(), None);
    }

    #[test]
    fn paths_that_look_like_flags_are_still_rejected() {
        // A stray unknown flag must not be silently swallowed as a filename.
        let error = parse(&["xl", "--nope"]).expect_err("unknown flags are errors");

        assert_eq!(error.kind(), clap::error::ErrorKind::UnknownArgument);
    }
}

/// Tests for piped stdin handling
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_spreadsheet_load_from_buffer() {
        let mut spreadsheet = spreadsheet::Spreadsheet::new();
        let data = b"col1 col2 col3\nval1 val2 val3";
        
        spreadsheet.load_from_buffer(data).unwrap();
        
        assert_eq!(spreadsheet.get_cell(0, 0), "col1");
        assert_eq!(spreadsheet.get_cell(0, 1), "col2");
        assert_eq!(spreadsheet.get_cell(0, 2), "col3");
        assert_eq!(spreadsheet.get_cell(1, 0), "val1");
        assert_eq!(spreadsheet.get_cell(1, 1), "val2");
        assert_eq!(spreadsheet.get_cell(1, 2), "val3");
    }
    
    #[test]
    #[cfg(unix)]
    fn test_dev_tty_exists() {
        // Verify /dev/tty exists on Unix systems (required for piped stdin support)
        use std::fs::metadata;
        assert!(metadata("/dev/tty").is_ok(), "/dev/tty should exist on Unix systems");
    }
    
    #[test]
    #[cfg(unix)]
    fn test_dev_tty_is_openable() {
        // Verify we can open /dev/tty (may fail in CI environments without a TTY)
        use std::fs::File;
        // This test documents the behavior - it may fail in headless CI
        // In real usage, the error is handled gracefully
        let result = File::open("/dev/tty");
        // We don't assert success because CI might not have a TTY,
        // but we verify the operation doesn't panic
        let _ = result;
    }
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    // Handle version flag
    if args.version {
        println!("{}", constants::VERSION);
        return Ok(());
    }

    // CRITICAL: Read all piped data from stdin FIRST, before any terminal setup.
    // The use-dev-tty feature makes crossterm read from /dev/tty directly,
    // but we still need to consume the piped data before it's lost.
    let piped_data = read_piped_stdin()?;
    
    // Check if stdout is a TTY (required for terminal UI)
    if !atty::is(atty::Stream::Stdout) {
        eprintln!("Error: stdout must be a terminal to run xl interactively");
        eprintln!("When piping data, xl loads the data but requires a terminal for display");
        std::process::exit(1);
    }
    
    // Load settings
    let settings = settings::Settings::load();

    // Now create and populate the spreadsheet
    let mut spreadsheet = crate::spreadsheet::Spreadsheet::new();
    spreadsheet.dark_mode = settings.dark_mode;
    spreadsheet.hide_update_prompt = settings.hide_update_prompt;
    
    if let Some(data) = piped_data {
        // Load data from the buffer we read earlier
        if let Err(e) = spreadsheet.load_from_buffer(&data) {
            eprintln!("Error loading data from stdin: {}", e);
            std::process::exit(1);
        }
    } else if let Some(filepath) = args.file_to_open() {
        // Load from file if provided
        if let Err(e) = spreadsheet.load_from_file(filepath) {
            eprintln!("Error loading file '{}': {}", filepath, e);
            std::process::exit(1);
        }
    }
    
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
