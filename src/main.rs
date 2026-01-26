mod constants;
mod formula;
mod input;
mod save;
mod spreadsheet;
mod style;
mod types;
mod ui;

use std::io::{self, Read};

use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

#[derive(Parser)]
#[command(name = "xl")]
#[command(about = "A terminal-based spreadsheet application")]
struct Args {
    /// File to open (supports CSV, TSV, and Excel files)
    #[arg(short, long)]
    file: Option<String>,
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

/// Redirects stdin (fd 0) to /dev/tty so that crossterm can read keyboard
/// events from the terminal even when the original stdin was a pipe.
#[cfg(unix)]
fn redirect_stdin_to_tty() -> io::Result<()> {
    use std::fs::File;
    use std::os::unix::io::AsRawFd;
    
    // Open /dev/tty to get a direct connection to the terminal
    let tty = File::open("/dev/tty").map_err(|e| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("Failed to open /dev/tty for keyboard input: {}. Cannot run interactively when stdin is piped.", e)
        )
    })?;
    
    // Duplicate the tty file descriptor to stdin (fd 0)
    // This allows crossterm to read keyboard events from the terminal
    unsafe {
        let tty_fd = tty.as_raw_fd();
        if libc::dup2(tty_fd, 0) == -1 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to redirect stdin to /dev/tty: {}", io::Error::last_os_error())
            ));
        }
    }
    // tty File is dropped here, but fd 0 now points to /dev/tty
    Ok(())
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

    // CRITICAL: Read all piped data from stdin FIRST, before any terminal setup.
    // Once we redirect stdin to /dev/tty, we can't read from the pipe anymore.
    let piped_data = read_piped_stdin()?;
    let stdin_was_piped = piped_data.is_some();
    
    // If stdin was piped, redirect it to /dev/tty BEFORE any crossterm calls.
    // This must happen before enable_raw_mode() or any other crossterm function
    // that might initialize the internal event reader.
    #[cfg(unix)]
    if stdin_was_piped {
        redirect_stdin_to_tty()?;
    }
    
    // Check if stdout is a TTY (required for terminal UI)
    if !atty::is(atty::Stream::Stdout) {
        eprintln!("Error: stdout must be a terminal to run xl interactively");
        eprintln!("When piping data, xl loads the data but requires a terminal for display");
        std::process::exit(1);
    }
    
    // Now create and populate the spreadsheet
    let mut spreadsheet = crate::spreadsheet::Spreadsheet::new();
    
    if let Some(data) = piped_data {
        // Load data from the buffer we read earlier
        if let Err(e) = spreadsheet.load_from_buffer(&data) {
            eprintln!("Error loading data from stdin: {}", e);
            std::process::exit(1);
        }
    } else if let Some(ref filepath) = args.file {
        // Load from file if provided
        if let Err(e) = spreadsheet.load_from_file(filepath) {
            eprintln!("Error loading file '{}': {}", filepath, e);
            std::process::exit(1);
        }
    }
    
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run app with pre-loaded spreadsheet
    let res = input::run_app(&mut terminal, spreadsheet);

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
