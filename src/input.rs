use std::io;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use crossterm::event::{
    self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::constants::COLOR_PALETTE;
use crate::hit_test::{hit_test, HitTarget};
use crate::settings;
use crate::spreadsheet::Spreadsheet;
use crate::types::{DataType, RowColumnSelectMode, SaveFormat, TextAlignment, VerticalAlignment, VisualSubMode};
use crate::ui;
use crate::update::{self, UpdateMessage};

pub fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut spreadsheet: Spreadsheet,
    update_rx: Receiver<UpdateMessage>,
) -> io::Result<()> {

    loop {
        // Check for update messages (non-blocking)
        if let Ok(msg) = update_rx.try_recv() {
            match msg {
                UpdateMessage::Available(info) => {
                    spreadsheet.update_available = Some(info);
                    if !spreadsheet.hide_update_prompt {
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

        terminal.draw(|f| ui::render(f, &mut spreadsheet))?;

        // Use poll with timeout to allow checking update messages periodically
        if event::poll(Duration::from_millis(100))? {
            match event::read() {
                Ok(Event::Key(key)) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }

                    // Handle update prompt first if shown
                    if spreadsheet.update_prompt_shown && !spreadsheet.update_in_progress {
                        if handle_update_prompt(&mut spreadsheet, key.code) {
                            continue;
                        }
                    }

                    if spreadsheet.editing {
                        handle_editing_mode(&mut spreadsheet, key.code, key.modifiers);
                    } else if spreadsheet.command_mode {
                        if handle_command_mode(&mut spreadsheet, key.code) {
                            return Ok(());
                        }
                    } else if spreadsheet.open_mode {
                        if handle_open_mode(&mut spreadsheet, key.code) {
                            continue;
                        }
                    } else if spreadsheet.save_mode {
                        if handle_save_mode(&mut spreadsheet, key.code) {
                            continue;
                        }
                    } else if spreadsheet.visual_mode {
                        handle_visual_mode(&mut spreadsheet, key.code);
                    } else if spreadsheet.row_column_select_mode != RowColumnSelectMode::None {
                        handle_row_column_select_mode(&mut spreadsheet, key.code, key.modifiers);
                    } else if spreadsheet.find_mode {
                        handle_find_mode(&mut spreadsheet, key.code, key.modifiers);
                    } else {
                        if handle_ready_mode(&mut spreadsheet, key.code, key.modifiers) {
                            return Ok(());
                        }
                    }
                }
                Ok(Event::Mouse(mouse)) => {
                    handle_mouse(&mut spreadsheet, mouse);
                }
                Ok(_) => {} // Ignore the remaining event kinds
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

/// How many rows one notch of the wheel moves.
const ROW_SCROLL_STEP: isize = 3;

/// How many columns one notch of the horizontal wheel moves.
///
/// One, not three. A column is ten characters wide by default and a row is one
/// line tall, so moving three of each would make a sideways notch travel about
/// ten times as far across the screen as a vertical one — the grid jumps
/// instead of scrolling, and it stops being obvious where you ended up.
const COLUMN_SCROLL_STEP: isize = 1;

/// Acts on a mouse event, if the grid is the thing the user is looking at.
///
/// Returns whether anything changed.
///
/// Clicks are ignored whenever a modal owns the screen — saving, opening, the
/// command line, the update prompt, or an in-progress edit. A click that changed
/// the grid behind a dialog would be a state change the user never saw, which is
/// the worst kind: silent. Ignoring is the safe default; a mode that wants the
/// mouse can ask for it later.
pub fn handle_mouse(spreadsheet: &mut Spreadsheet, event: MouseEvent) -> bool {
    let modal_owns_the_screen = spreadsheet.editing
        || spreadsheet.command_mode
        || spreadsheet.open_mode
        || spreadsheet.save_mode
        || spreadsheet.update_prompt_shown
        || spreadsheet.update_in_progress;
    if modal_owns_the_screen {
        return false;
    }

    let geometry = spreadsheet.grid_geometry.clone();
    let target = hit_test(event.column, event.row, &geometry);
    let sideways = event.modifiers.contains(KeyModifiers::SHIFT);

    match event.kind {
        // Shift turns the vertical wheel sideways. Spreadsheets and browsers
        // have done this for long enough that it is what people try, and it is
        // the only horizontal scrolling available on a mouse that has no thumb
        // wheel.
        MouseEventKind::ScrollDown if sideways => {
            spreadsheet.scroll_grid_horizontally(COLUMN_SCROLL_STEP)
        }
        MouseEventKind::ScrollUp if sideways => {
            spreadsheet.scroll_grid_horizontally(-COLUMN_SCROLL_STEP)
        }
        MouseEventKind::ScrollDown => spreadsheet.scroll_grid_vertically(ROW_SCROLL_STEP),
        MouseEventKind::ScrollUp => spreadsheet.scroll_grid_vertically(-ROW_SCROLL_STEP),
        MouseEventKind::ScrollRight => spreadsheet.scroll_grid_horizontally(COLUMN_SCROLL_STEP),
        MouseEventKind::ScrollLeft => spreadsheet.scroll_grid_horizontally(-COLUMN_SCROLL_STEP),
        MouseEventKind::Down(MouseButton::Left) => match target {
            HitTarget::Cell { row, col } => spreadsheet.select_cell(row, col),
            HitTarget::ColumnHeader(col) => spreadsheet.select_whole_column(col),
            HitTarget::RowHeader(row) => spreadsheet.select_whole_row(row),
            HitTarget::SheetIndicator => spreadsheet.next_sheet(),
            HitTarget::Outside => false,
        },
        // Dragging extends the selection from wherever the press started.
        MouseEventKind::Drag(MouseButton::Left) => match target {
            HitTarget::Cell { row, col } => spreadsheet.extend_selection_to(row, col),
            _ => false,
        },
        _ => false,
    }
}

/// Handle update prompt (y/n)
/// Returns true if the key was handled by the update prompt
fn handle_update_prompt(spreadsheet: &mut Spreadsheet, code: KeyCode) -> bool {
    match code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            if let Some(ref update_info) = spreadsheet.update_available.clone() {
                spreadsheet.update_in_progress = true;
                spreadsheet.update_message = Some("Downloading update...".to_string());

                // Perform the update
                match update::download_and_install(update_info) {
                    Ok(()) => {
                        spreadsheet.update_message = Some(format!(
                            "Updated to {}! Please restart xl to use the new version.",
                            update_info.latest_version
                        ));
                        spreadsheet.update_in_progress = false;
                        spreadsheet.update_prompt_shown = false;
                    }
                    Err(e) => {
                        spreadsheet.update_message = Some(format!("Update failed: {}", e));
                        spreadsheet.update_in_progress = false;
                        spreadsheet.update_prompt_shown = false;
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
                } else {
                    spreadsheet.open_message = Some(format!("Loaded {}", filename));
                    spreadsheet.exit_open_mode();
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

fn handle_ready_mode(
    spreadsheet: &mut Spreadsheet,
    code: KeyCode,
    modifiers: KeyModifiers,
) -> bool {
    let shift = modifiers.contains(KeyModifiers::SHIFT);
    let ctrl_or_cmd = modifiers.contains(KeyModifiers::CONTROL) || modifiers.contains(KeyModifiers::SUPER);
    let cmd = modifiers.contains(KeyModifiers::SUPER);
    let alt = modifiers.contains(KeyModifiers::ALT);
    
    match code {
        // Copy (Ctrl+C / Cmd+C)
        KeyCode::Char('c') if ctrl_or_cmd => {
            spreadsheet.copy_selection();
        }
        // Cut (Ctrl+X / Cmd+X)
        KeyCode::Char('x') if ctrl_or_cmd => {
            spreadsheet.cut_selection();
        }
        // Paste (Ctrl+V / Cmd+V)
        KeyCode::Char('v') if ctrl_or_cmd => {
            spreadsheet.paste();
        }
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
        // Sheet navigation, matching Excel. These must come before the
        // unmodified Page keys below, which would otherwise swallow them.
        KeyCode::PageDown if ctrl_or_cmd => {
            spreadsheet.next_sheet();
            return false;
        }
        KeyCode::PageUp if ctrl_or_cmd => {
            spreadsheet.previous_sheet();
            return false;
        }
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

fn handle_find_mode(spreadsheet: &mut Spreadsheet, code: KeyCode, modifiers: KeyModifiers) {
    let ctrl_or_cmd = modifiers.contains(KeyModifiers::CONTROL) || modifiers.contains(KeyModifiers::SUPER);
    
    match code {
        // Copy (Ctrl+C / Cmd+C)
        KeyCode::Char('c') if ctrl_or_cmd => {
            spreadsheet.copy_selection();
        }
        // Cut (Ctrl+X / Cmd+X)
        KeyCode::Char('x') if ctrl_or_cmd => {
            spreadsheet.cut_selection();
        }
        // Paste (Ctrl+V / Cmd+V)
        KeyCode::Char('v') if ctrl_or_cmd => {
            spreadsheet.paste();
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ready_mode_quit() {
        let mut sheet = Spreadsheet::new();
        assert!(handle_ready_mode(&mut sheet, KeyCode::Char('q'), KeyModifiers::empty()));
        assert!(handle_ready_mode(&mut sheet, KeyCode::Char('Q'), KeyModifiers::empty()));
    }

    #[test]
    fn test_ready_mode_movement() {
        let mut sheet = Spreadsheet::new();

        handle_ready_mode(&mut sheet, KeyCode::Down, KeyModifiers::empty());
        assert_eq!(sheet.cursor_row, 1);

        handle_ready_mode(&mut sheet, KeyCode::Right, KeyModifiers::empty());
        assert_eq!(sheet.cursor_col, 1);
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

#[cfg(test)]
mod sheet_navigation_keys {
    use super::*;
    use crate::sheet::Sheet;

    fn workbook() -> Spreadsheet {
        let mut sheet = Spreadsheet::new();
        sheet.replace_sheets(vec![Sheet::new("Alpha"), Sheet::new("Beta")]);
        sheet
    }

    #[test]
    fn ctrl_page_keys_move_between_sheets() {
        let mut sheet = workbook();

        handle_ready_mode(&mut sheet, KeyCode::PageDown, KeyModifiers::CONTROL);
        assert_eq!(sheet.active_sheet_name(), "Beta");

        handle_ready_mode(&mut sheet, KeyCode::PageUp, KeyModifiers::CONTROL);
        assert_eq!(sheet.active_sheet_name(), "Alpha");
    }

    /// The plain Page keys still jump within the sheet, as they always have.
    ///
    /// The sheet arms are matched first, so this is the check that they did not
    /// swallow the unmodified keys.
    #[test]
    fn unmodified_page_keys_still_jump_inside_the_sheet() {
        let mut sheet = workbook();
        // The Page keys jump between the populated rows of the current column,
        // so the column needs a top and a bottom to travel between.
        sheet.set_cell(5, 0, "top".to_string());
        sheet.set_cell(20, 0, "bottom".to_string());
        sheet.cursor_row = 10;

        handle_ready_mode(&mut sheet, KeyCode::PageDown, KeyModifiers::empty());
        assert_eq!(sheet.active_sheet_name(), "Alpha", "still on the same sheet");
        assert_eq!(sheet.cursor_row, 20, "moved to the last populated row");

        handle_ready_mode(&mut sheet, KeyCode::PageUp, KeyModifiers::empty());
        assert_eq!(sheet.active_sheet_name(), "Alpha");
        assert_eq!(sheet.cursor_row, 5, "moved to the first populated row");
    }

    /// Typing a letter edits the cell; it must not be read as a sheet command.
    ///
    /// `handle_ready_mode` ends in a `KeyCode::Char(c)` catch-all that starts
    /// editing, which is why a two-key sequence like vim's `gt` cannot be used
    /// for sheet navigation here.
    #[test]
    fn letters_still_start_editing_rather_than_switching_sheets() {
        let mut sheet = workbook();

        handle_ready_mode(&mut sheet, KeyCode::Char('g'), KeyModifiers::empty());

        assert_eq!(sheet.active_sheet_name(), "Alpha");
        assert!(sheet.editing, "a letter begins a cell edit");
    }

    /// Tab keeps opening Visual mode, which is why it is not the sheet key.
    #[test]
    fn tab_still_opens_visual_mode() {
        let mut sheet = workbook();

        handle_ready_mode(&mut sheet, KeyCode::Tab, KeyModifiers::empty());

        assert!(sheet.visual_mode);
        assert_eq!(sheet.active_sheet_name(), "Alpha");
    }
}

#[cfg(test)]
mod mouse_input {
    use super::*;
    use crate::hit_test::GridGeometry;
    use crate::sheet::Sheet;
    use ratatui::layout::Rect;

    /// A grid drawn at the origin: 5-wide row numbers, three 10-wide columns,
    /// eight single-height rows, showing the top-left of the sheet.
    fn grid() -> GridGeometry {
        GridGeometry {
            area: Rect {
                x: 0,
                y: 0,
                width: 40,
                height: 12,
            },
            row_header_width: 5,
            scroll_col: 0,
            col_widths: vec![10, 10, 10],
            scroll_row: 0,
            row_heights: vec![1; 8],
            sheet_indicator_width: 13,
        }
    }

    fn sheet() -> Spreadsheet {
        let mut sheet = Spreadsheet::new();
        sheet.grid_geometry = grid();
        sheet
    }

    fn at(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::empty(),
        }
    }

    fn click(column: u16, row: u16) -> MouseEvent {
        at(MouseEventKind::Down(MouseButton::Left), column, row)
    }

    fn with_shift(kind: MouseEventKind) -> MouseEvent {
        MouseEvent {
            kind,
            column: 10,
            row: 5,
            modifiers: KeyModifiers::SHIFT,
        }
    }

    #[test]
    fn clicking_a_cell_moves_the_cursor_there() {
        let mut sheet = sheet();

        assert!(handle_mouse(&mut sheet, click(16, 4)));

        assert_eq!((sheet.cursor_row, sheet.cursor_col), (2, 1));
        assert_eq!(sheet.selection_anchor, None);
    }

    #[test]
    fn clicking_outside_the_grid_does_nothing() {
        let mut sheet = sheet();
        sheet.cursor_row = 3;
        sheet.cursor_col = 2;

        assert!(!handle_mouse(&mut sheet, click(0, 0)), "the corner");
        assert!(!handle_mouse(&mut sheet, click(39, 5)), "the right border");

        assert_eq!((sheet.cursor_row, sheet.cursor_col), (3, 2));
    }

    #[test]
    fn dragging_grows_the_selection_from_where_it_started() {
        let mut sheet = sheet();
        handle_mouse(&mut sheet, click(6, 2));

        assert!(handle_mouse(
            &mut sheet,
            at(MouseEventKind::Drag(MouseButton::Left), 26, 5)
        ));

        assert_eq!(sheet.selection_anchor, Some((0, 0)));
        assert_eq!((sheet.cursor_row, sheet.cursor_col), (3, 2));
    }

    #[test]
    fn clicking_a_column_letter_selects_that_column() {
        let mut sheet = sheet();

        assert!(handle_mouse(&mut sheet, click(16, 1)));

        assert_eq!(sheet.selected_cols, Some((1, 1)));
        assert_eq!(sheet.row_column_select_mode, RowColumnSelectMode::ColumnSelect);
    }

    #[test]
    fn clicking_a_row_number_selects_that_row() {
        let mut sheet = sheet();

        assert!(handle_mouse(&mut sheet, click(2, 4)));

        assert_eq!(sheet.selected_rows, Some((2, 2)));
        assert_eq!(sheet.row_column_select_mode, RowColumnSelectMode::RowSelect);
    }

    #[test]
    fn clicking_the_sheet_name_moves_to_the_next_sheet() {
        let mut sheet = sheet();
        sheet.replace_sheets(vec![Sheet::new("Alpha"), Sheet::new("Beta")]);
        sheet.grid_geometry = grid();

        assert!(handle_mouse(&mut sheet, click(4, 0)));

        assert_eq!(sheet.active_sheet_name(), "Beta");
    }

    /// The wheel moves the view, and the cursor rides along.
    ///
    /// Leaving the cursor behind would look tidier, but the renderer keeps the
    /// cursor on screen and would drag the view straight back — see
    /// `the_wheel_still_has_an_effect_once_the_frame_is_drawn`.
    #[test]
    fn the_wheel_scrolls_the_view_and_takes_the_cursor_with_it() {
        let mut sheet = sheet();
        let (row, col) = (sheet.cursor_row, sheet.cursor_col);

        assert!(handle_mouse(&mut sheet, at(MouseEventKind::ScrollDown, 10, 5)));
        assert_eq!(sheet.scroll_row, 3);
        assert_eq!(sheet.cursor_row, row + 3, "the cursor followed the view");
        assert_eq!(sheet.cursor_col, col, "and nothing moved sideways");

        let back = at(MouseEventKind::ScrollUp, 10, 5);
        assert!(handle_mouse(&mut sheet, back));
        assert_eq!(sheet.scroll_row, 0);
        assert_eq!(sheet.cursor_row, row, "scrolling back returns the cursor");
    }

    /// Scrolling has to survive the frame that comes after it.
    ///
    /// The renderer calls `adjust_scroll` on every frame to keep the cursor on
    /// screen. A wheel event that moved the view away from the cursor was
    /// therefore undone before the user ever saw it: the test above passed
    /// while the wheel did nothing at all.
    #[test]
    fn the_wheel_still_has_an_effect_once_the_frame_is_drawn() {
        let mut sheet = sheet();
        let area = ratatui::layout::Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        };

        let wheel = at(MouseEventKind::ScrollDown, 10, 5);
        assert!(handle_mouse(&mut sheet, wheel));
        let scrolled_to = sheet.scroll_row;
        assert!(scrolled_to > 0, "the wheel moved the view");

        sheet.adjust_scroll(area);

        assert_eq!(
            sheet.scroll_row, scrolled_to,
            "drawing the next frame undid the scroll"
        );
    }

    /// A thumb wheel moves the grid sideways.
    ///
    /// Untested until now: the code was there, but nothing said it worked, so
    /// a change to the match arms could have removed it without a failure.
    #[test]
    fn the_horizontal_wheel_scrolls_sideways() {
        let mut sheet = sheet();
        let (row, col) = (sheet.cursor_row, sheet.cursor_col);

        let right = at(MouseEventKind::ScrollRight, 10, 5);
        assert!(handle_mouse(&mut sheet, right));
        assert_eq!(sheet.scroll_col, 1);
        assert_eq!(sheet.cursor_col, col + 1, "the cursor came along");
        assert_eq!(sheet.cursor_row, row, "and nothing moved up or down");

        let left = at(MouseEventKind::ScrollLeft, 10, 5);
        assert!(handle_mouse(&mut sheet, left));
        assert_eq!(sheet.scroll_col, 0);
        assert_eq!(sheet.cursor_col, col);
    }

    /// A sideways notch moves one column, not three.
    ///
    /// Columns are ten characters wide and rows are one line tall. Three of
    /// each would send the view about ten times as far sideways as it goes
    /// down, which reads as a jump rather than a scroll.
    #[test]
    fn a_sideways_notch_travels_less_far_than_a_vertical_one() {
        let mut scrolled_sideways = sheet();
        let sideways = at(MouseEventKind::ScrollRight, 10, 5);
        assert!(handle_mouse(&mut scrolled_sideways, sideways));
        let columns = scrolled_sideways.scroll_col;

        let mut scrolled_down = sheet();
        let down = at(MouseEventKind::ScrollDown, 10, 5);
        assert!(handle_mouse(&mut scrolled_down, down));
        let rows = scrolled_down.scroll_row;

        assert!(
            columns < rows,
            "a column is wider than a row is tall: {columns} columns vs {rows} rows"
        );
    }

    /// Holding shift turns the vertical wheel sideways.
    ///
    /// This is what a mouse without a thumb wheel has, and what people try
    /// first because spreadsheets and browsers have always worked this way.
    #[test]
    fn shift_turns_the_vertical_wheel_sideways() {
        let mut sheet = sheet();

        let down = with_shift(MouseEventKind::ScrollDown);
        assert!(handle_mouse(&mut sheet, down));
        assert_eq!(sheet.scroll_col, 1, "moved sideways");
        assert_eq!(sheet.scroll_row, 0, "and not down");

        let up = with_shift(MouseEventKind::ScrollUp);
        assert!(handle_mouse(&mut sheet, up));
        assert_eq!(sheet.scroll_col, 0);
        assert_eq!(sheet.scroll_row, 0);
    }

    /// Sideways scrolling survives the frame after it, like the vertical case.
    #[test]
    fn the_horizontal_wheel_still_has_an_effect_once_the_frame_is_drawn() {
        let mut sheet = sheet();
        let area = ratatui::layout::Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        };

        let right = at(MouseEventKind::ScrollRight, 10, 5);
        assert!(handle_mouse(&mut sheet, right));
        let scrolled_to = sheet.scroll_col;
        assert!(scrolled_to > 0, "the wheel moved the view");

        sheet.adjust_scroll(area);

        assert_eq!(
            sheet.scroll_col, scrolled_to,
            "drawing the next frame undid the sideways scroll"
        );
    }

    #[test]
    fn the_horizontal_wheel_cannot_scroll_past_the_edges_of_the_sheet() {
        let mut sheet = sheet();
        let left = at(MouseEventKind::ScrollLeft, 10, 5);
        assert!(!handle_mouse(&mut sheet, left), "already at column A");
        assert_eq!(sheet.scroll_col, 0);

        sheet.scroll_col = sheet.num_cols - 1;
        let right = at(MouseEventKind::ScrollRight, 10, 5);
        assert!(!handle_mouse(&mut sheet, right), "already at column Z");
        assert_eq!(sheet.scroll_col, sheet.num_cols - 1);
    }

    #[test]
    fn the_wheel_cannot_scroll_past_the_start_of_the_sheet() {
        let mut sheet = sheet();

        assert!(!handle_mouse(&mut sheet, at(MouseEventKind::ScrollUp, 10, 5)));

        assert_eq!(sheet.scroll_row, 0);
    }

    #[test]
    fn the_wheel_cannot_scroll_past_the_end_of_the_sheet() {
        let mut sheet = sheet();
        sheet.scroll_row = sheet.num_rows - 1;

        assert!(!handle_mouse(&mut sheet, at(MouseEventKind::ScrollDown, 10, 5)));

        assert_eq!(sheet.scroll_row, sheet.num_rows - 1);
    }

    /// A click behind a dialog would change the grid where the user cannot see
    /// it. Every modal keeps the mouse out.
    fn assert_modal_swallows_clicks(name: &str, open_modal: impl Fn(&mut Spreadsheet)) {
        let mut sheet = sheet();
        open_modal(&mut sheet);

        assert!(
            !handle_mouse(&mut sheet, click(16, 4)),
            "{name} let a click through"
        );
        assert_eq!(
            (sheet.cursor_row, sheet.cursor_col),
            (0, 0),
            "{name} moved the cursor"
        );
    }

    #[test]
    fn modals_swallow_mouse_input() {
        assert_modal_swallows_clicks("editing", |s| s.editing = true);
        assert_modal_swallows_clicks("command mode", |s| s.command_mode = true);
        assert_modal_swallows_clicks("open dialog", |s| s.open_mode = true);
        assert_modal_swallows_clicks("save dialog", |s| s.save_mode = true);
        assert_modal_swallows_clicks("update prompt", |s| s.update_prompt_shown = true);
    }

    /// Before the first frame there is no geometry, so nothing can be hit.
    #[test]
    fn a_click_before_the_first_draw_is_ignored() {
        let mut sheet = Spreadsheet::new();

        assert!(!handle_mouse(&mut sheet, click(10, 5)));
    }

    #[test]
    fn scrolled_views_resolve_clicks_to_the_right_sheet_coordinates() {
        let mut sheet = sheet();
        sheet.grid_geometry.scroll_row = 40;
        sheet.grid_geometry.scroll_col = 6;

        assert!(handle_mouse(&mut sheet, click(16, 4)));

        assert_eq!((sheet.cursor_row, sheet.cursor_col), (42, 7));
    }
}
