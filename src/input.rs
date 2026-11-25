use std::io;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::clipboard::{ClipboardBackend, SystemClipboard};
use crate::constants::COLOR_PALETTE;
use crate::settings;
use crate::shell;
use crate::spreadsheet::Spreadsheet;
use crate::types::{DataType, RowColumnSelectMode, SaveFormat, TextAlignment, VerticalAlignment, VisualSubMode};
use crate::ui;
use crate::update::{self, InstallResult, UpdateInfo, UpdateMessage};

#[derive(Default)]
struct ActiveInstall {
    receiver: Option<Receiver<InstallResult>>,
    quit_when_done: bool,
}

impl ActiveInstall {
    fn defer_or_allow_quit(&mut self, spreadsheet: &mut Spreadsheet) -> bool {
        if self.receiver.is_none() {
            return true;
        }

        self.quit_when_done = true;
        if spreadsheet.command_mode {
            spreadsheet.exit_command_mode();
        }
        spreadsheet.update_message =
            Some("Downloading update... xl will quit when it finishes.".into());
        false
    }

    fn ready_for_deferred_quit(&self) -> bool {
        self.quit_when_done && self.receiver.is_none()
    }
}

pub fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut spreadsheet: Spreadsheet,
    update_rx: Receiver<UpdateMessage>,
) -> io::Result<Option<String>> {
    let mut clipboard = SystemClipboard::default();
    let mut active_install = ActiveInstall::default();

    loop {
        poll_update_install(&mut spreadsheet, &mut active_install);
        if active_install.ready_for_deferred_quit() {
            return Ok(spreadsheet.update_message.take());
        }
        // Check for update messages (non-blocking)
        if let Ok(msg) = update_rx.try_recv() {
            match msg {
                UpdateMessage::Available(info) => {
                    spreadsheet.update_available = Some(info);
                    if !spreadsheet.hide_update_prompt && active_install.receiver.is_none() {
                        spreadsheet.update_prompt_shown = true;
                    }
                }
                UpdateMessage::NotAvailable => {}
                UpdateMessage::Error(_) => {
                    // Silently ignore update check errors (network issues, etc.)
                    // The update check will be retried on next launch
                }
            }
        }

        let drawn_size = terminal
            .draw(|f| ui::render(f, &mut spreadsheet))?
            .area
            .as_size();

        // Use poll with timeout to allow checking update messages periodically
        if event::poll(Duration::from_millis(100))? {
            match event::read() {
                Ok(Event::Key(key)) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    spreadsheet.clipboard_message = None;

                    if spreadsheet.shell_prompt.is_some() {
                        let confirming = matches!(key.code, KeyCode::Char('r' | 'a'));
                        if key.modifiers.is_empty()
                            && (!confirming || terminal.size()? == drawn_size)
                        {
                            handle_shell_prompt(&mut spreadsheet, key.code, shell::run_command);
                        }
                        continue;
                    }

                    if handle_sheet_key(
                        &mut spreadsheet,
                        key,
                        &mut clipboard,
                        &mut active_install,
                        update::download_and_install,
                    ) {
                        return Ok(None);
                    }
                }
                Ok(_) => {} // Ignore non-key events
                Err(e) => {
                    // If we can't read events, it might be a terminal issue
                    // Try to restore terminal and exit gracefully
                    let _ = crossterm::terminal::disable_raw_mode();
                    let _ = crossterm::execute!(
                        terminal.backend_mut(),
                        crossterm::terminal::LeaveAlternateScreen,
                        crossterm::event::DisableMouseCapture
                    );
                    let _ = terminal.show_cursor();
                    return Err(e);
                }
            }
        }
    }
}

fn handle_sheet_key(
    spreadsheet: &mut Spreadsheet,
    key: KeyEvent,
    clipboard: &mut impl ClipboardBackend,
    active_install: &mut ActiveInstall,
    install: impl FnOnce(&UpdateInfo) -> Result<(), String> + Send + 'static,
) -> bool {
    if spreadsheet.update_prompt_shown
        && handle_update_prompt(spreadsheet, key.code, active_install, install)
    {
        return false;
    }

    if spreadsheet.editing {
        handle_editing_mode(spreadsheet, key.code, key.modifiers);
    } else if spreadsheet.command_mode {
        return handle_command_mode(spreadsheet, key.code)
            && active_install.defer_or_allow_quit(spreadsheet);
    } else if spreadsheet.open_mode {
        handle_open_mode(spreadsheet, key.code);
    } else if spreadsheet.save_mode {
        handle_save_mode(spreadsheet, key.code);
    } else if spreadsheet.visual_mode {
        handle_visual_mode(spreadsheet, key.code);
    } else if spreadsheet.row_column_select_mode != RowColumnSelectMode::None {
        handle_row_column_select_mode(spreadsheet, key.code, key.modifiers);
    } else if spreadsheet.find_mode {
        handle_find_mode(spreadsheet, key.code, key.modifiers, clipboard);
    } else {
        return handle_ready_mode(spreadsheet, key.code, key.modifiers, clipboard)
            && active_install.defer_or_allow_quit(spreadsheet);
    }
    false
}

fn poll_update_install(
    spreadsheet: &mut Spreadsheet,
    active_install: &mut ActiveInstall,
) -> bool {
    let Some(receiver) = active_install.receiver.as_ref() else {
        return false;
    };
    let result = match receiver.try_recv() {
        Ok(result) => result,
        Err(TryRecvError::Empty) => return false,
        Err(TryRecvError::Disconnected) => Err("Installer exited before reporting a result".into()),
    };
    active_install.receiver = None;
    finish_update_install(spreadsheet, result);
    true
}

fn finish_update_install(spreadsheet: &mut Spreadsheet, result: InstallResult) {
    spreadsheet.update_in_progress = false;
    spreadsheet.update_prompt_shown = false;
    spreadsheet.update_message = Some(match result {
        Ok(version) => format!("Updated to {version}! Please restart xl to use the new version."),
        Err(error) => format!("Update failed: {error}"),
    });
}

/// Handle update prompt (y/n)
/// Returns true if the key was handled by the update prompt
fn handle_update_prompt(
    spreadsheet: &mut Spreadsheet,
    code: KeyCode,
    active_install: &mut ActiveInstall,
    install: impl FnOnce(&UpdateInfo) -> Result<(), String> + Send + 'static,
) -> bool {
    if active_install.receiver.is_some() {
        return false;
    }
    match code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            if let Some(info) = spreadsheet.update_available.clone() {
                match update::spawn_update_install(info, install) {
                    Ok(receiver) => {
                        active_install.receiver = Some(receiver);
                        spreadsheet.update_in_progress = true;
                        spreadsheet.update_prompt_shown = false;
                        spreadsheet.update_message = Some("Downloading update...".into());
                    }
                    Err(error) => {
                        finish_update_install(
                            spreadsheet,
                            Err(format!("Could not start installer: {error}")),
                        );
                    }
                }
            }
            true
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            spreadsheet.update_prompt_shown = false;
            spreadsheet.update_message = None;
            true
        }
        KeyCode::Char('d') | KeyCode::Char('D') => {
            spreadsheet.hide_update_prompt = true;
            spreadsheet.update_prompt_shown = false;
            spreadsheet.update_available = None;
            spreadsheet.update_message = None;
            let mut s = settings::Settings::load();
            s.set_hide_update_prompt(true);
            true
        }
        _ => false,
    }
}

fn handle_editing_mode(spreadsheet: &mut Spreadsheet, code: KeyCode, modifiers: KeyModifiers) {
    let shift = modifiers.contains(KeyModifiers::SHIFT);

    if spreadsheet.selecting_ref {
        handle_ref_selection_mode(spreadsheet, code, shift);
    } else {
        handle_normal_editing(spreadsheet, code);
    }
}

fn handle_ref_selection_mode(spreadsheet: &mut Spreadsheet, code: KeyCode, shift: bool) {
    match code {
        KeyCode::Up => spreadsheet.move_ref_cursor(-1, 0, shift),
        KeyCode::Down => spreadsheet.move_ref_cursor(1, 0, shift),
        KeyCode::Left => spreadsheet.move_ref_cursor(0, -1, shift),
        KeyCode::Right => spreadsheet.move_ref_cursor(0, 1, shift),
        KeyCode::Enter => spreadsheet.finish_editing(),
        KeyCode::Esc => spreadsheet.cancel_editing(),
        KeyCode::Char(',') => {
            spreadsheet.edit_buffer.push(',');
            spreadsheet.enter_ref_selection_mode();
        }
        KeyCode::Char(c) => {
            spreadsheet.exit_ref_selection_mode();
            spreadsheet.handle_char_input(c);
        }
        KeyCode::Backspace => {
            spreadsheet.exit_ref_selection_mode();
            spreadsheet.edit_buffer.pop();
        }
        _ => {}
    }
}

fn handle_normal_editing(spreadsheet: &mut Spreadsheet, code: KeyCode) {
    match code {
        KeyCode::Enter => {
            if spreadsheet.formula_autocomplete_active && !spreadsheet.formula_suggestions.is_empty() {
                // Select the current suggestion
                let selected = &spreadsheet.formula_suggestions[spreadsheet.formula_suggestion_index];
                // Replace the prefix in edit_buffer with the full formula name
                let prefix_start = spreadsheet.edit_buffer.find('=').unwrap_or(0) + 1;
                let prefix_end = prefix_start + spreadsheet.formula_prefix.len();
                spreadsheet.edit_buffer.replace_range(prefix_start..prefix_end, selected);
                spreadsheet.edit_buffer.push('(');
                spreadsheet.formula_autocomplete_active = false;
                spreadsheet.formula_suggestions.clear();
                spreadsheet.formula_prefix.clear();
                spreadsheet.enter_ref_selection_mode();
            } else {
                spreadsheet.finish_editing();
            }
        }
        KeyCode::Up => {
            if spreadsheet.formula_autocomplete_active && !spreadsheet.formula_suggestions.is_empty() {
                // Navigate up in suggestions
                if spreadsheet.formula_suggestion_index > 0 {
                    spreadsheet.formula_suggestion_index -= 1;
                } else {
                    spreadsheet.formula_suggestion_index = spreadsheet.formula_suggestions.len() - 1;
                }
            } else {
                spreadsheet.finish_editing_with_move(-1, 0);
            }
        }
        KeyCode::Down => {
            if spreadsheet.formula_autocomplete_active && !spreadsheet.formula_suggestions.is_empty() {
                // Navigate down in suggestions
                spreadsheet.formula_suggestion_index = (spreadsheet.formula_suggestion_index + 1) % spreadsheet.formula_suggestions.len();
            } else {
                spreadsheet.finish_editing_with_move(1, 0);
            }
        }
        KeyCode::Left => spreadsheet.finish_editing_with_move(0, -1),
        KeyCode::Right => spreadsheet.finish_editing_with_move(0, 1),
        KeyCode::Esc => {
            spreadsheet.formula_autocomplete_active = false;
            spreadsheet.formula_suggestions.clear();
            spreadsheet.cancel_editing();
        }
        KeyCode::Backspace => {
            spreadsheet.edit_buffer.pop();
            spreadsheet.formula_mode = spreadsheet.edit_buffer.starts_with('=');
            if spreadsheet.formula_mode {
                // Update suggestions after backspace
                let after_equals = &spreadsheet.edit_buffer[1..];
                let prefix_end = after_equals
                    .char_indices()
                    .find(|(_, ch)| !ch.is_alphabetic())
                    .map(|(i, _)| i)
                    .unwrap_or(after_equals.len());
                spreadsheet.formula_prefix = after_equals[..prefix_end].to_string();
                spreadsheet.update_formula_suggestions();
            } else {
                spreadsheet.formula_autocomplete_active = false;
                spreadsheet.formula_suggestions.clear();
                spreadsheet.formula_prefix.clear();
            }
        }
        KeyCode::Char(c) => {
            spreadsheet.handle_char_input(c);
        }
        KeyCode::Tab => {
            if spreadsheet.formula_autocomplete_active && !spreadsheet.formula_suggestions.is_empty() {
                // Select the current suggestion (same as Enter)
                let selected = &spreadsheet.formula_suggestions[spreadsheet.formula_suggestion_index];
                let prefix_start = spreadsheet.edit_buffer.find('=').unwrap_or(0) + 1;
                let prefix_end = prefix_start + spreadsheet.formula_prefix.len();
                spreadsheet.edit_buffer.replace_range(prefix_start..prefix_end, selected);
                spreadsheet.edit_buffer.push('(');
                spreadsheet.formula_autocomplete_active = false;
                spreadsheet.formula_suggestions.clear();
                spreadsheet.formula_prefix.clear();
                spreadsheet.enter_ref_selection_mode();
            } else {
                spreadsheet.finish_editing_with_move(0, 1);
            }
        }
        _ => {}
    }
}

fn handle_open_mode(spreadsheet: &mut Spreadsheet, code: KeyCode) -> bool {
    match code {
        KeyCode::Char(c) if c.is_alphanumeric() || c == '_' || c == '-' || c == '/' || c == '.' || c == '~' || c == ' ' => {
            spreadsheet.open_filename.push(c);
            spreadsheet.open_message = None;
        }
        KeyCode::Backspace => {
            spreadsheet.open_filename.pop();
            spreadsheet.open_message = None;
        }
        KeyCode::Enter => {
            if spreadsheet.open_filename.is_empty() {
                spreadsheet.open_message = Some("Filename cannot be empty".to_string());
            } else {
                let filename = spreadsheet.open_filename.clone();
                if let Err(e) = spreadsheet.load_from_file(&filename) {
                    spreadsheet.open_message = Some(format!("Error: {}", e));
                }
            }
        }
        KeyCode::Esc => spreadsheet.exit_open_mode(),
        _ => {}
    }
    false
}

fn handle_save_mode(spreadsheet: &mut Spreadsheet, code: KeyCode) -> bool {
    match code {
        KeyCode::Char('1') => spreadsheet.save_format = SaveFormat::Csv,
        KeyCode::Char('2') => spreadsheet.save_format = SaveFormat::Tsv,
        KeyCode::Char(c) if c.is_alphanumeric() || c == '_' || c == '-' => {
            spreadsheet.save_filename.push(c);
            spreadsheet.save_message = None;
        }
        KeyCode::Backspace => {
            spreadsheet.save_filename.pop();
            spreadsheet.save_message = None;
        }
        KeyCode::Enter => {
            if spreadsheet.save_filename.is_empty() {
                spreadsheet.save_message = Some("Filename cannot be empty".to_string());
            } else if let Err(e) = spreadsheet.save_to_file() {
                spreadsheet.save_message = Some(format!("Error: {}", e));
            }
        }
        KeyCode::Esc => spreadsheet.exit_save_mode(),
        _ => {}
    }
    false
}

fn handle_visual_mode(spreadsheet: &mut Spreadsheet, code: KeyCode) {
    match spreadsheet.visual_sub_mode {
        VisualSubMode::Main => handle_visual_main(spreadsheet, code),
        VisualSubMode::TextColor => handle_visual_text_color(spreadsheet, code),
        VisualSubMode::BackgroundColor => handle_visual_bg_color(spreadsheet, code),
        VisualSubMode::ColumnWidth => handle_visual_column_width(spreadsheet, code),
        VisualSubMode::RowHeight => handle_visual_row_height(spreadsheet, code),
        VisualSubMode::TextAlignment => handle_visual_text_alignment(spreadsheet, code),
        VisualSubMode::VerticalAlignment => handle_visual_vertical_alignment(spreadsheet, code),
        VisualSubMode::FontSize => handle_visual_font_size(spreadsheet, code),
        VisualSubMode::DataType => handle_visual_data_type(spreadsheet, code),
    }
}

fn handle_visual_main(spreadsheet: &mut Spreadsheet, code: KeyCode) {
    match code {
        KeyCode::Char('f') | KeyCode::Char('F') => {
            spreadsheet.visual_sub_mode = VisualSubMode::TextColor;
        }
        KeyCode::Char('b') | KeyCode::Char('B') => {
            spreadsheet.visual_sub_mode = VisualSubMode::BackgroundColor;
        }
        KeyCode::Char('a') | KeyCode::Char('A') => {
            spreadsheet.visual_sub_mode = VisualSubMode::TextAlignment;
        }
        KeyCode::Char('v') | KeyCode::Char('V') => {
            spreadsheet.visual_sub_mode = VisualSubMode::VerticalAlignment;
        }
        KeyCode::Char('w') | KeyCode::Char('W') => {
            spreadsheet.visual_sub_mode = VisualSubMode::ColumnWidth;
        }
        KeyCode::Char('h') | KeyCode::Char('H') => {
            spreadsheet.visual_sub_mode = VisualSubMode::RowHeight;
        }
        KeyCode::Char('s') | KeyCode::Char('S') => {
            spreadsheet.visual_sub_mode = VisualSubMode::FontSize;
        }
        KeyCode::Char('t') | KeyCode::Char('T') => {
            spreadsheet.visual_sub_mode = VisualSubMode::DataType;
        }
        KeyCode::Char('c') | KeyCode::Char('C') => {
            spreadsheet.clear_formatting_from_selection();
        }
        KeyCode::Char('m') | KeyCode::Char('M') => {
            spreadsheet.toggle_dark_mode();
        }
        KeyCode::Esc | KeyCode::Tab => spreadsheet.exit_visual_mode(),
        _ => {}
    }
}

fn handle_visual_text_color(spreadsheet: &mut Spreadsheet, code: KeyCode) {
    match code {
        KeyCode::Char(c) if c.is_ascii_digit() => {
            let idx = c.to_digit(10).unwrap() as usize;
            if idx < COLOR_PALETTE.len() {
                let color = COLOR_PALETTE[idx].0;
                spreadsheet.apply_style_to_selection(Some(color), None);
                spreadsheet.visual_sub_mode = VisualSubMode::Main;
            }
        }
        KeyCode::Esc => spreadsheet.visual_sub_mode = VisualSubMode::Main,
        _ => {}
    }
}

fn handle_visual_bg_color(spreadsheet: &mut Spreadsheet, code: KeyCode) {
    match code {
        KeyCode::Char(c) if c.is_ascii_digit() => {
            let idx = c.to_digit(10).unwrap() as usize;
            if idx < COLOR_PALETTE.len() {
                let color = COLOR_PALETTE[idx].0;
                spreadsheet.apply_style_to_selection(None, Some(color));
                spreadsheet.visual_sub_mode = VisualSubMode::Main;
            }
        }
        KeyCode::Esc => spreadsheet.visual_sub_mode = VisualSubMode::Main,
        _ => {}
    }
}

fn handle_visual_column_width(spreadsheet: &mut Spreadsheet, code: KeyCode) {
    match code {
        KeyCode::Left => {
            let col = spreadsheet.cursor_col;
            let current = spreadsheet.get_col_width(col);
            spreadsheet.set_col_width(col, current.saturating_sub(1));
        }
        KeyCode::Right => {
            let col = spreadsheet.cursor_col;
            let current = spreadsheet.get_col_width(col);
            spreadsheet.set_col_width(col, current + 1);
        }
        KeyCode::Esc => spreadsheet.visual_sub_mode = VisualSubMode::Main,
        _ => {}
    }
}

fn handle_visual_row_height(spreadsheet: &mut Spreadsheet, code: KeyCode) {
    match code {
        KeyCode::Up => {
            let row = spreadsheet.cursor_row;
            let current = spreadsheet.get_row_height(row);
            spreadsheet.set_row_height(row, current.saturating_sub(1));
        }
        KeyCode::Down => {
            let row = spreadsheet.cursor_row;
            let current = spreadsheet.get_row_height(row);
            spreadsheet.set_row_height(row, current + 1);
        }
        KeyCode::Esc => spreadsheet.visual_sub_mode = VisualSubMode::Main,
        _ => {}
    }
}

fn handle_visual_text_alignment(spreadsheet: &mut Spreadsheet, code: KeyCode) {
    match code {
        KeyCode::Char('1') => {
            spreadsheet.apply_alignment_to_selection(Some(TextAlignment::Left));
            spreadsheet.visual_sub_mode = VisualSubMode::Main;
        }
        KeyCode::Char('2') => {
            spreadsheet.apply_alignment_to_selection(Some(TextAlignment::Center));
            spreadsheet.visual_sub_mode = VisualSubMode::Main;
        }
        KeyCode::Char('3') => {
            spreadsheet.apply_alignment_to_selection(Some(TextAlignment::Right));
            spreadsheet.visual_sub_mode = VisualSubMode::Main;
        }
        KeyCode::Char('0') => {
            spreadsheet.apply_alignment_to_selection(None);
            spreadsheet.visual_sub_mode = VisualSubMode::Main;
        }
        KeyCode::Esc => spreadsheet.visual_sub_mode = VisualSubMode::Main,
        _ => {}
    }
}

fn handle_visual_vertical_alignment(spreadsheet: &mut Spreadsheet, code: KeyCode) {
    match code {
        KeyCode::Char('1') => {
            spreadsheet.apply_vertical_alignment_to_selection(Some(VerticalAlignment::Top));
            spreadsheet.visual_sub_mode = VisualSubMode::Main;
        }
        KeyCode::Char('2') => {
            spreadsheet.apply_vertical_alignment_to_selection(Some(VerticalAlignment::Center));
            spreadsheet.visual_sub_mode = VisualSubMode::Main;
        }
        KeyCode::Char('3') => {
            spreadsheet.apply_vertical_alignment_to_selection(Some(VerticalAlignment::Bottom));
            spreadsheet.visual_sub_mode = VisualSubMode::Main;
        }
        KeyCode::Char('0') => {
            spreadsheet.apply_vertical_alignment_to_selection(None);
            spreadsheet.visual_sub_mode = VisualSubMode::Main;
        }
        KeyCode::Esc => spreadsheet.visual_sub_mode = VisualSubMode::Main,
        _ => {}
    }
}

fn handle_visual_font_size(spreadsheet: &mut Spreadsheet, code: KeyCode) {
    match code {
        KeyCode::Char('+') | KeyCode::Char('=') | KeyCode::Up => {
            // Increase font size = make bold
            spreadsheet.apply_bold_to_selection(true);
        }
        KeyCode::Char('-') | KeyCode::Down => {
            // Decrease font size = remove bold
            spreadsheet.apply_bold_to_selection(false);
        }
        KeyCode::Esc => spreadsheet.visual_sub_mode = VisualSubMode::Main,
        _ => {}
    }
}

fn handle_visual_data_type(spreadsheet: &mut Spreadsheet, code: KeyCode) {
    match code {
        KeyCode::Char('1') => {
            spreadsheet.apply_data_type_to_selection(Some(DataType::Text));
            spreadsheet.visual_sub_mode = VisualSubMode::Main;
        }
        KeyCode::Char('2') => {
            spreadsheet.apply_data_type_to_selection(Some(DataType::Number));
            spreadsheet.visual_sub_mode = VisualSubMode::Main;
        }
        KeyCode::Char('3') => {
            spreadsheet.apply_data_type_to_selection(Some(DataType::Currency));
            spreadsheet.visual_sub_mode = VisualSubMode::Main;
        }
        KeyCode::Char('4') => {
            spreadsheet.apply_data_type_to_selection(Some(DataType::Percentage));
            spreadsheet.visual_sub_mode = VisualSubMode::Main;
        }
        KeyCode::Char('5') => {
            spreadsheet.apply_data_type_to_selection(Some(DataType::Date));
            spreadsheet.visual_sub_mode = VisualSubMode::Main;
        }
        KeyCode::Char('6') => {
            spreadsheet.apply_data_type_to_selection(Some(DataType::Time));
            spreadsheet.visual_sub_mode = VisualSubMode::Main;
        }
        KeyCode::Char('0') => {
            spreadsheet.apply_data_type_to_selection(None);
            spreadsheet.visual_sub_mode = VisualSubMode::Main;
        }
        KeyCode::Esc => spreadsheet.visual_sub_mode = VisualSubMode::Main,
        _ => {}
    }
}

fn handle_clipboard_shortcut(
    spreadsheet: &mut Spreadsheet,
    code: KeyCode,
    modifiers: KeyModifiers,
    clipboard: &mut impl ClipboardBackend,
) -> bool {
    if !modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::SUPER) {
        return false;
    }
    match code {
        KeyCode::Char('c') => spreadsheet.copy_selection(clipboard),
        KeyCode::Char('x') => spreadsheet.cut_selection(clipboard),
        KeyCode::Char('v') => spreadsheet.paste(clipboard),
        _ => return false,
    }
    true
}

fn handle_ready_mode(
    spreadsheet: &mut Spreadsheet,
    code: KeyCode,
    modifiers: KeyModifiers,
    clipboard: &mut impl ClipboardBackend,
) -> bool {
    if handle_clipboard_shortcut(spreadsheet, code, modifiers, clipboard) {
        return false;
    }
    let shift = modifiers.contains(KeyModifiers::SHIFT);
    let ctrl_or_cmd = modifiers.contains(KeyModifiers::CONTROL) || modifiers.contains(KeyModifiers::SUPER);
    let cmd = modifiers.contains(KeyModifiers::SUPER);
    let alt = modifiers.contains(KeyModifiers::ALT);
    
    match code {
        // Cmd+Arrow or Alt+Arrow: Jump to last data column/row
        // On macOS, Cmd+Arrow might be intercepted by the system, so Alt+Arrow is more reliable
        KeyCode::Right if cmd || alt => {
            spreadsheet.jump_to_last_col();
            return false; // Stay in ready mode
        }
        KeyCode::Left if cmd || alt => {
            spreadsheet.jump_to_first_col();
            return false; // Stay in ready mode
        }
        KeyCode::Down if cmd || alt => {
            spreadsheet.jump_to_last_row();
            return false; // Stay in ready mode
        }
        KeyCode::Up if cmd || alt => {
            spreadsheet.jump_to_first_row();
            return false; // Stay in ready mode
        }
        // Workaround for macOS: Cmd+Arrow sends special characters via terminal
        // Cmd+Right sends End (0x05 = Ctrl+E in Emacs)
        // Cmd+Left sends Home (0x01 = Ctrl+A in Emacs)  
        // Cmd+Down sends End of buffer (Ctrl+N or similar)
        // Cmd+Up sends Beginning of buffer (Ctrl+P or similar)
        KeyCode::End => {
            spreadsheet.jump_to_last_col();
            return false;
        }
        KeyCode::Home => {
            spreadsheet.jump_to_first_col();
            return false;
        }
        // PageDown/PageUp for jumping to last/first row (alternative to Cmd+Down/Up)
        KeyCode::PageDown => {
            spreadsheet.jump_to_last_row();
            return false;
        }
        KeyCode::PageUp => {
            spreadsheet.jump_to_first_row();
            return false;
        }
        // Handle Ctrl+E / Ctrl+A (Emacs keybindings, also sent by some terminals for Cmd+Arrow)
        KeyCode::Char('e') if ctrl_or_cmd => {
            spreadsheet.jump_to_last_col();
            return false;
        }
        KeyCode::Char('a') if ctrl_or_cmd => {
            spreadsheet.jump_to_first_col();
            return false;
        }
        // Handle Ctrl+N / Ctrl+P (Emacs keybindings for up/down, sent by terminals for Cmd+Up/Down)
        KeyCode::Char('n') if ctrl_or_cmd => {
            spreadsheet.jump_to_last_row();
            return false;
        }
        KeyCode::Char('p') if ctrl_or_cmd => {
            spreadsheet.jump_to_first_row();
            return false;
        }
        // Also handle raw control characters that terminals may send
        KeyCode::Char('\x05') => { // Ctrl+E / End of line
            spreadsheet.jump_to_last_col();
            return false;
        }
        KeyCode::Char('\x01') => { // Ctrl+A / Beginning of line
            spreadsheet.jump_to_first_col();
            return false;
        }
        KeyCode::Char('\x0E') => { // Ctrl+N / Next line
            spreadsheet.jump_to_last_row();
            return false;
        }
        KeyCode::Char('\x10') => { // Ctrl+P / Previous line
            spreadsheet.jump_to_first_row();
            return false;
        }
        KeyCode::Char('q') | KeyCode::Char('Q') => return true,
        KeyCode::Char('o') | KeyCode::Char('O') => spreadsheet.enter_open_mode(),
        KeyCode::Char('s') | KeyCode::Char('S') => spreadsheet.enter_save_mode(),
        KeyCode::Char('t') | KeyCode::Char('T') => spreadsheet.format_as_table(),
        KeyCode::Char('f') | KeyCode::Char('F') => spreadsheet.enter_find_mode(),
        KeyCode::Char('r') | KeyCode::Char('R') if shift => {
            spreadsheet.enter_row_select_mode();
        }
        KeyCode::Char('c') | KeyCode::Char('C') if shift => {
            spreadsheet.enter_column_select_mode();
        }
        KeyCode::Up => spreadsheet.move_cursor(-1, 0, shift),
        KeyCode::Down => spreadsheet.move_cursor(1, 0, shift),
        KeyCode::Left => spreadsheet.move_cursor(0, -1, shift),
        KeyCode::Right => spreadsheet.move_cursor(0, 1, shift),
        KeyCode::Enter => {
            spreadsheet.clear_selection();
            spreadsheet.start_editing();
        }
        KeyCode::Delete | KeyCode::Backspace => spreadsheet.delete_cell(),
        KeyCode::Tab => spreadsheet.enter_visual_mode(),
        // Enter command mode with colon (vim-style)
        KeyCode::Char(':') => {
            spreadsheet.enter_command_mode();
        }
        KeyCode::Char(c) => {
            spreadsheet.clear_selection();
            spreadsheet.start_editing();
            spreadsheet.handle_char_input(c);
        }
        KeyCode::Esc => spreadsheet.clear_selection(),
        _ => {}
    }
    false
}

fn handle_row_column_select_mode(
    spreadsheet: &mut Spreadsheet,
    code: KeyCode,
    _modifiers: KeyModifiers,
) {
    match spreadsheet.row_column_select_mode {
        RowColumnSelectMode::RowSelect => {
            match code {
                KeyCode::Up => {
                    // If cursor is at max_row and selection has more than one row, deselect bottom row
                    // Otherwise extend selection up
                    if let Some((min_row, max_row)) = spreadsheet.selected_rows {
                        if spreadsheet.cursor_row == max_row && max_row > min_row {
                            // Deselect the bottom row
                            spreadsheet.cursor_row -= 1;
                            spreadsheet.selected_rows = Some((min_row, max_row - 1));
                        } else if spreadsheet.cursor_row > 0 {
                            // Extend selection up
                            spreadsheet.cursor_row -= 1;
                            spreadsheet.selected_rows = Some((
                                spreadsheet.cursor_row.min(min_row),
                                max_row,
                            ));
                        }
                    }
                }
                KeyCode::Down => {
                    // If cursor is at min_row and selection has more than one row, deselect top row
                    // Otherwise extend selection down
                    if let Some((min_row, max_row)) = spreadsheet.selected_rows {
                        if spreadsheet.cursor_row == min_row && max_row > min_row {
                            // Deselect the top row
                            spreadsheet.cursor_row += 1;
                            spreadsheet.selected_rows = Some((min_row + 1, max_row));
                        } else if spreadsheet.cursor_row < spreadsheet.num_rows - 1 {
                            // Extend selection down
                            spreadsheet.cursor_row += 1;
                            spreadsheet.selected_rows = Some((
                                min_row,
                                spreadsheet.cursor_row.max(max_row),
                            ));
                        }
                    }
                }
                KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Delete | KeyCode::Backspace => {
                    spreadsheet.delete_selected_rows();
                }
                KeyCode::Char('i') | KeyCode::Char('I') => {
                    spreadsheet.insert_rows_after_selected();
                }
                KeyCode::Esc => {
                    spreadsheet.exit_row_column_select_mode();
                }
                _ => {}
            }
        }
        RowColumnSelectMode::ColumnSelect => {
            match code {
                KeyCode::Left => {
                    // If cursor is at max_col and selection has more than one column, deselect rightmost column
                    // Otherwise extend selection left
                    if let Some((min_col, max_col)) = spreadsheet.selected_cols {
                        if spreadsheet.cursor_col == max_col && max_col > min_col {
                            // Deselect the rightmost column
                            spreadsheet.cursor_col -= 1;
                            spreadsheet.selected_cols = Some((min_col, max_col - 1));
                        } else if spreadsheet.cursor_col > 0 {
                            // Extend selection left
                            spreadsheet.cursor_col -= 1;
                            spreadsheet.selected_cols = Some((
                                spreadsheet.cursor_col.min(min_col),
                                max_col,
                            ));
                        }
                    }
                }
                KeyCode::Right => {
                    // If cursor is at min_col and selection has more than one column, deselect leftmost column
                    // Otherwise extend selection right
                    if let Some((min_col, max_col)) = spreadsheet.selected_cols {
                        if spreadsheet.cursor_col == min_col && max_col > min_col {
                            // Deselect the leftmost column
                            spreadsheet.cursor_col += 1;
                            spreadsheet.selected_cols = Some((min_col + 1, max_col));
                        } else if spreadsheet.cursor_col < spreadsheet.num_cols - 1 {
                            // Extend selection right
                            spreadsheet.cursor_col += 1;
                            spreadsheet.selected_cols = Some((
                                min_col,
                                spreadsheet.cursor_col.max(max_col),
                            ));
                        }
                    }
                }
                KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Delete | KeyCode::Backspace => {
                    spreadsheet.delete_selected_columns();
                }
                KeyCode::Char('i') | KeyCode::Char('I') => {
                    spreadsheet.insert_columns_after_selected();
                }
                KeyCode::Esc => {
                    spreadsheet.exit_row_column_select_mode();
                }
                _ => {}
            }
        }
        RowColumnSelectMode::None => {}
    }
}

fn handle_find_mode(
    spreadsheet: &mut Spreadsheet,
    code: KeyCode,
    modifiers: KeyModifiers,
    clipboard: &mut impl ClipboardBackend,
) {
    if handle_clipboard_shortcut(spreadsheet, code, modifiers, clipboard) {
        return;
    }
    match code {
        KeyCode::Char(c) => {
            spreadsheet.find_query.push(c);
            spreadsheet.update_find_matches();
        }
        KeyCode::Backspace => {
            spreadsheet.find_query.pop();
            spreadsheet.update_find_matches();
        }
        KeyCode::Enter => {
            // Keep matches highlighted but exit find mode for navigation
            spreadsheet.find_mode = false;
        }
        KeyCode::Esc => {
            spreadsheet.exit_find_mode();
        }
        _ => {}
    }
}

/// Handle command mode (vim-style :command)
/// Returns true if the app should quit
fn handle_command_mode(spreadsheet: &mut Spreadsheet, code: KeyCode) -> bool {
    match code {
        KeyCode::Char(c) => {
            spreadsheet.command_buffer.push(c);
            spreadsheet.command_message = None;
        }
        KeyCode::Backspace => {
            if spreadsheet.command_buffer.is_empty() {
                spreadsheet.exit_command_mode();
            } else {
                spreadsheet.command_buffer.pop();
                spreadsheet.command_message = None;
            }
        }
        KeyCode::Enter => {
            if spreadsheet.command_buffer.is_empty() {
                spreadsheet.exit_command_mode();
            } else {
                return spreadsheet.execute_command();
            }
        }
        KeyCode::Esc => {
            spreadsheet.exit_command_mode();
        }
        _ => {}
    }
    false
}

fn handle_shell_prompt(
    spreadsheet: &mut Spreadsheet,
    code: KeyCode,
    runner: impl FnOnce(&str) -> Result<String, String>,
) {
    let Some(prompt) = &mut spreadsheet.shell_prompt else {
        return;
    };
    match code {
        KeyCode::Esc => spreadsheet.shell_prompt = None,
        KeyCode::Up => prompt.scroll = prompt.scroll.saturating_sub(1),
        KeyCode::Down => prompt.scroll = prompt.scroll.saturating_add(1),
        KeyCode::Char('r') => spreadsheet.confirm_shell_run(runner),
        KeyCode::Char('a') => spreadsheet.apply_shell_output(),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clipboard::OfflineClipboard;
    use crate::shell::ShellStep;
    use ratatui::backend::TestBackend;

    fn open_shell_preview(sheet: &mut Spreadsheet) {
        sheet.enter_command_mode();
        for ch in "run".chars() {
            handle_command_mode(sheet, KeyCode::Char(ch));
        }
        assert!(!handle_command_mode(sheet, KeyCode::Enter));
        assert!(!sheet.command_mode);
        assert!(matches!(
            &sheet.shell_prompt.as_ref().unwrap().step,
            ShellStep::Run { .. }
        ));
    }

    fn press_shell_key(sheet: &mut Spreadsheet, code: KeyCode) {
        handle_shell_prompt(sheet, code, |_| panic!("command must not run for this key"));
    }

    fn render_sheet(terminal: &mut Terminal<TestBackend>, sheet: &mut Spreadsheet) -> String {
        terminal.draw(|frame| ui::render(frame, sheet)).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn delayed_install_keeps_input_responsive_and_defers_both_quit_keys() {
        use std::sync::mpsc::{Sender, channel};
        use std::time::Instant;

        struct Cleanup(Sender<()>);

        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = self.0.send(());
            }
        }

        for (outcome, expected, quit_keys) in [
            (Ok(()), "Updated to v2.0.0!", &[KeyCode::Char('q')][..]),
            (
                Err("fixture copy failed".to_string()),
                "Update failed: fixture copy failed",
                &[KeyCode::Char(':'), KeyCode::Char('q'), KeyCode::Enter][..],
            ),
        ] {
            let (started_tx, started_rx) = channel();
            let (release_tx, release_rx) = channel();
            let (cleaned_tx, cleaned_rx) = channel();
            let mut sheet = Spreadsheet::new();
            sheet.update_available = Some(UpdateInfo {
                current_version: "v1.0.0".into(),
                latest_version: "v2.0.0".into(),
                download_url: String::new(),
                asset_name: String::new(),
            });
            sheet.update_prompt_shown = true;
            let mut active_install = ActiveInstall::default();
            let mut clipboard = OfflineClipboard;
            let mut terminal = Terminal::new(TestBackend::new(120, 20)).unwrap();
            let key = |code| KeyEvent::new(code, KeyModifiers::NONE);

            assert!(!handle_sheet_key(
                &mut sheet,
                key(KeyCode::Char('y')),
                &mut clipboard,
                &mut active_install,
                move |info| {
                    let _cleanup = Cleanup(cleaned_tx);
                    assert_eq!(info.latest_version, "v2.0.0");
                    started_tx.send(()).unwrap();
                    release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                    outcome
                },
            ));
            started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            assert!(sheet.update_in_progress);
            assert!(!poll_update_install(&mut sheet, &mut active_install));
            assert!(render_sheet(&mut terminal, &mut sheet).contains("Downloading update..."));
            assert!(!handle_update_prompt(
                &mut sheet,
                KeyCode::Char('y'),
                &mut active_install,
                |_| { panic!("duplicate install started") }
            ));

            for code in [KeyCode::Enter, KeyCode::Char('7'), KeyCode::Enter] {
                assert!(!handle_sheet_key(
                    &mut sheet,
                    key(code),
                    &mut clipboard,
                    &mut active_install,
                    |_| panic!("an editing key started an install"),
                ));
            }
            assert_eq!(sheet.get_cell(0, 0), "7");

            sheet
                .load_from_file(concat!(env!("CARGO_MANIFEST_DIR"), "/spreadsheet.csv"))
                .unwrap();
            assert_eq!(sheet.get_cell(0, 0), "First");
            assert!(sheet.update_in_progress);
            assert!(render_sheet(&mut terminal, &mut sheet).contains("Downloading update..."));

            for &code in quit_keys {
                assert!(!handle_sheet_key(
                    &mut sheet,
                    key(code),
                    &mut clipboard,
                    &mut active_install,
                    |_| panic!("a quit key started an install"),
                ));
            }
            assert!(active_install.quit_when_done);
            assert!(!active_install.ready_for_deferred_quit());
            assert!(!sheet.command_mode);
            assert!(
                render_sheet(&mut terminal, &mut sheet).contains("xl will quit when it finishes.")
            );
            assert_eq!(cleaned_rx.try_recv(), Err(TryRecvError::Empty));

            assert!(!handle_sheet_key(
                &mut sheet,
                key(KeyCode::Right),
                &mut clipboard,
                &mut active_install,
                |_| panic!("a navigation key started an install"),
            ));
            assert_eq!(sheet.cursor_col, 1);

            release_tx.send(()).unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            while !poll_update_install(&mut sheet, &mut active_install) {
                assert!(Instant::now() < deadline, "installer never completed");
                std::thread::yield_now();
            }
            assert_eq!(cleaned_rx.try_recv(), Ok(()));
            assert!(active_install.ready_for_deferred_quit());
            assert!(!sheet.update_in_progress && !sheet.update_prompt_shown);
            assert!(render_sheet(&mut terminal, &mut sheet).contains(expected));
        }
    }

    #[test]
    fn disconnected_installer_reports_failure_and_clears_progress() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut active_install = ActiveInstall {
            receiver: Some(rx),
            quit_when_done: true,
        };
        let mut sheet = Spreadsheet::new();
        sheet.update_in_progress = true;
        drop(tx);

        assert!(poll_update_install(&mut sheet, &mut active_install));
        assert!(active_install.ready_for_deferred_quit());
        assert!(!sheet.update_in_progress);
        assert!(
            sheet
                .update_message
                .as_deref()
                .unwrap()
                .contains("Installer exited")
        );
    }

    #[test]
    fn paste_shortcut_shows_canceled_cut_feedback() {
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "original".into());
        sheet.clipboard_data = Some(sheet.capture_clipboard(true));
        sheet.set_cell(0, 0, "edited".into());
        sheet.cursor_col = 1;

        handle_ready_mode(
            &mut sheet,
            KeyCode::Char('v'),
            KeyModifiers::CONTROL,
            &mut OfflineClipboard,
        );

        let mut terminal = Terminal::new(TestBackend::new(80, 12)).unwrap();
        assert!(
            render_sheet(&mut terminal, &mut sheet).contains("Cut canceled. Paste again to copy.")
        );
        assert_eq!(sheet.get_cell(0, 0), "edited");
        assert_eq!(sheet.get_cell(0, 1), "");
    }

    #[test]
    fn shell_import_evaluation_and_render_keep_formula_inert() {
        let mut sheet = Spreadsheet::new();
        let path = std::env::temp_dir().join(format!("xl-shell-import-{}.csv", std::process::id()));
        std::fs::write(&path, "=SHELL(echo imported),keep\n=SUM(A1),\n").unwrap();
        let import = sheet.load_from_file(path.to_str().unwrap());
        std::fs::remove_file(path).unwrap();
        import.unwrap();
        assert_eq!(sheet.get_cell(0, 0), "=SHELL(echo imported)");
        assert_eq!(sheet.get_cell(0, 1), "keep");
        let cells = sheet.cells.clone();
        let dimensions = (sheet.num_rows, sheet.num_cols);

        assert_eq!(sheet.evaluate_cell(0, 0), "[SHELL]");
        assert_eq!(sheet.evaluate_cell(1, 0), "0");
        assert_eq!(
            sheet.evaluate_formula("=IF(1=1,SHELL(echo nested),0)", 0, 1),
            "[SHELL]"
        );
        let mut terminal = Terminal::new(TestBackend::new(100, 20)).unwrap();
        assert!(render_sheet(&mut terminal, &mut sheet).contains("[SHELL]"));
        assert_eq!(sheet.cells, cells);
        assert_eq!((sheet.num_rows, sheet.num_cols), dimensions);
        assert!(sheet.shell_prompt.is_none());
    }

    #[test]
    fn shell_command_preview_scrolls_through_long_and_multiline_input() {
        let mut sheet = Spreadsheet::new();
        let command = format!("echo {}\necho last-line", "x".repeat(180));
        sheet.set_cell(0, 0, format!("=sHeLl( \"{command}\" )"));
        open_shell_preview(&mut sheet);

        let mut terminal = Terminal::new(TestBackend::new(38, 5)).unwrap();
        let initial = render_sheet(&mut terminal, &mut sheet);
        assert!(initial.contains("echo x"));
        assert!(!initial.contains("echo last-line"));
        for _ in 0..10 {
            press_shell_key(&mut sheet, KeyCode::Down);
            render_sheet(&mut terminal, &mut sheet);
        }
        assert!(render_sheet(&mut terminal, &mut sheet).contains("echo last-line"));
        press_shell_key(&mut sheet, KeyCode::Up);
        press_shell_key(&mut sheet, KeyCode::Esc);
        assert!(sheet.shell_prompt.is_none());
        assert_eq!(sheet.get_cell(0, 0), format!("=sHeLl( \"{command}\" )"));
    }

    #[test]
    fn shell_stages_ragged_output_until_apply() {
        let mut sheet = Spreadsheet::new();
        sheet.num_rows = 2;
        sheet.num_cols = 3;
        sheet.cursor_col = 1;
        sheet.set_cell(0, 1, "=sHeLl( 'echo one\necho two' )".to_string());
        sheet.set_cell(0, 2, "will replace".to_string());
        sheet.set_cell(1, 2, "ragged gap".to_string());
        let cells = sheet.cells.clone();
        open_shell_preview(&mut sheet);
        press_shell_key(&mut sheet, KeyCode::Char('a'));
        press_shell_key(&mut sheet, KeyCode::Enter);
        let mut terminal = Terminal::new(TestBackend::new(45, 8)).unwrap();
        render_sheet(&mut terminal, &mut sheet);

        let mut calls = 0;
        let runner = |command: &str| {
            calls += 1;
            assert_eq!(command, "echo one\necho two");
            Ok("first second extra\nthird\nfourth fifth\n".to_string())
        };
        handle_shell_prompt(&mut sheet, KeyCode::Char('r'), runner);
        assert_eq!(calls, 1);
        press_shell_key(&mut sheet, KeyCode::Char('r'));
        assert_eq!(sheet.cells, cells);
        assert_eq!((sheet.num_rows, sheet.num_cols), (2, 3));

        let preview = render_sheet(&mut terminal, &mut sheet);
        assert!(preview.contains("Destination range B1:D3"));
        assert!(preview.contains("Cells to write 6"));
        assert!(preview.contains("Occupied cells to overwrite 2"));

        press_shell_key(&mut sheet, KeyCode::Char('a'));
        assert!(sheet.shell_prompt.is_none());
        assert_eq!(sheet.get_cell(0, 1), "first");
        assert_eq!(sheet.get_cell(0, 2), "second");
        assert_eq!(sheet.get_cell(0, 3), "extra");
        assert_eq!(sheet.get_cell(1, 1), "third");
        assert_eq!(sheet.get_cell(1, 2), "ragged gap");
        assert_eq!(sheet.get_cell(2, 1), "fourth");
        assert_eq!(sheet.get_cell(2, 2), "fifth");
        assert_eq!((sheet.num_rows, sheet.num_cols), (3, 4));
    }

    #[test]
    fn shell_cancel_empty_output_and_failure_keep_sheet_intact() {
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "=SHELL(echo example)".to_string());
        let cells = sheet.cells.clone();
        let dimensions = (sheet.num_rows, sheet.num_cols);
        let mut terminal = Terminal::new(TestBackend::new(80, 12)).unwrap();

        open_shell_preview(&mut sheet);
        render_sheet(&mut terminal, &mut sheet);
        handle_shell_prompt(&mut sheet, KeyCode::Char('r'), |_| Ok("one\ntwo".into()));
        let ShellStep::Apply(output) = &sheet.shell_prompt.as_ref().unwrap().step else {
            panic!("expected an output preview");
        };
        assert_eq!(output.range(), "A1");
        assert_eq!(output.counts(&sheet), (1, 1));
        press_shell_key(&mut sheet, KeyCode::Esc);

        for (result, expected) in [
            (Ok(" \n\t".into()), "Command produced no output."),
            (Err("could not run".into()), "could not run"),
        ] {
            open_shell_preview(&mut sheet);
            render_sheet(&mut terminal, &mut sheet);
            handle_shell_prompt(&mut sheet, KeyCode::Char('r'), |_| result);
            assert!(matches!(
                &sheet.shell_prompt.as_ref().unwrap().step,
                ShellStep::Message(message) if message == expected
            ));
            press_shell_key(&mut sheet, KeyCode::Char('a'));
            press_shell_key(&mut sheet, KeyCode::Esc);
            assert!(sheet.shell_prompt.is_none());
            assert_eq!(sheet.cells, cells);
            assert_eq!((sheet.num_rows, sheet.num_cols), dimensions);
        }

        open_shell_preview(&mut sheet);
        render_sheet(&mut terminal, &mut sheet);
        handle_shell_prompt(&mut sheet, KeyCode::Char('r'), |_| Ok("one\ntwo".into()));
        render_sheet(&mut terminal, &mut sheet);
        press_shell_key(&mut sheet, KeyCode::Char('a'));
        assert_eq!(sheet.get_cell(0, 0), "one\ntwo");
        assert_eq!((sheet.num_rows, sheet.num_cols), dimensions);
    }

    #[test]
    fn shell_confirmation_requires_a_visible_preview_after_resize() {
        let mut sheet = Spreadsheet::new();
        let formula = "=SHELL(echo example)";
        sheet.set_cell(0, 0, formula.to_string());
        let mut terminal = Terminal::new(TestBackend::new(80, 3)).unwrap();

        open_shell_preview(&mut sheet);
        press_shell_key(&mut sheet, KeyCode::Char('r'));
        let small = render_sheet(&mut terminal, &mut sheet);
        assert!(small.contains("Resize to review SHELL"));
        assert!(!small.contains("echo example"));
        press_shell_key(&mut sheet, KeyCode::Char('r'));
        press_shell_key(&mut sheet, KeyCode::Esc);
        assert!(sheet.shell_prompt.is_none());

        open_shell_preview(&mut sheet);
        terminal.backend_mut().resize(80, 8);
        assert!(render_sheet(&mut terminal, &mut sheet).contains("echo example"));
        terminal.backend_mut().resize(2, 8);
        render_sheet(&mut terminal, &mut sheet);
        press_shell_key(&mut sheet, KeyCode::Char('r'));
        terminal.backend_mut().resize(80, 8);
        assert!(render_sheet(&mut terminal, &mut sheet).contains("echo example"));
        let mut calls = 0;
        handle_shell_prompt(&mut sheet, KeyCode::Char('r'), |command| {
            calls += 1;
            assert_eq!(command, "echo example");
            Ok("result".into())
        });
        assert_eq!(calls, 1);

        press_shell_key(&mut sheet, KeyCode::Char('a'));
        assert_eq!(sheet.get_cell(0, 0), formula);
        assert!(render_sheet(&mut terminal, &mut sheet).contains("Destination range A1"));
        for height in [3, 4] {
            terminal.backend_mut().resize(80, height);
            assert!(render_sheet(&mut terminal, &mut sheet).contains("Resize to review SHELL"));
            press_shell_key(&mut sheet, KeyCode::Char('a'));
            assert_eq!(sheet.get_cell(0, 0), formula);
        }
        terminal.backend_mut().resize(80, 8);
        let apply = render_sheet(&mut terminal, &mut sheet);
        assert!(apply.contains("Destination range A1"));
        assert!(apply.contains("Occupied cells to overwrite 1"));
        press_shell_key(&mut sheet, KeyCode::Char('a'));
        assert_eq!(sheet.get_cell(0, 0), "result");
        assert!(sheet.shell_prompt.is_none());
    }

    #[test]
    fn test_ready_mode_quit() {
        let mut sheet = Spreadsheet::new();
        assert!(handle_ready_mode(
            &mut sheet,
            KeyCode::Char('q'),
            KeyModifiers::empty(),
            &mut OfflineClipboard,
        ));
        assert!(handle_ready_mode(
            &mut sheet,
            KeyCode::Char('Q'),
            KeyModifiers::empty(),
            &mut OfflineClipboard,
        ));
    }

    #[test]
    fn test_ready_mode_movement() {
        let mut sheet = Spreadsheet::new();

        handle_ready_mode(
            &mut sheet,
            KeyCode::Down,
            KeyModifiers::empty(),
            &mut OfflineClipboard,
        );
        assert_eq!(sheet.cursor_row, 1);

        handle_ready_mode(
            &mut sheet,
            KeyCode::Right,
            KeyModifiers::empty(),
            &mut OfflineClipboard,
        );
        assert_eq!(sheet.cursor_col, 1);
    }

    #[test]
    fn failed_open_keeps_dialog_ready_to_retry() {
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "keep this".into());
        sheet.enter_open_mode();
        sheet.open_filename = "Cargo.toml".into();

        handle_open_mode(&mut sheet, KeyCode::Enter);

        assert!(sheet.open_mode);
        assert_eq!(sheet.open_filename, "Cargo.toml");
        assert!(
            sheet.open_message.as_deref().unwrap().contains("Unsupported file format")
        );
        assert_eq!(sheet.get_cell(0, 0), "keep this");
    }

    #[test]
    fn test_editing_mode_enter() {
        let mut sheet = Spreadsheet::new();
        sheet.start_editing();
        sheet.edit_buffer = "test".to_string();

        handle_normal_editing(&mut sheet, KeyCode::Enter);

        assert!(!sheet.editing);
        assert_eq!(sheet.get_cell(0, 0), "test");
    }
}
