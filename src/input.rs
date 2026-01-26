use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::constants::COLOR_PALETTE;
use crate::spreadsheet::Spreadsheet;
use crate::types::{RowColumnSelectMode, SaveFormat, TextAlignment, VerticalAlignment, VisualSubMode};
use crate::ui;

pub fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut spreadsheet: Spreadsheet,
) -> io::Result<()> {

    loop {
        terminal.draw(|f| ui::render(f, &mut spreadsheet))?;

        match event::read() {
            Ok(Event::Key(key)) => {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                if spreadsheet.editing {
                    handle_editing_mode(&mut spreadsheet, key.code, key.modifiers);
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
        KeyCode::Enter => spreadsheet.finish_editing(),
        KeyCode::Up => spreadsheet.finish_editing_with_move(-1, 0),
        KeyCode::Down => spreadsheet.finish_editing_with_move(1, 0),
        KeyCode::Left => spreadsheet.finish_editing_with_move(0, -1),
        KeyCode::Right => spreadsheet.finish_editing_with_move(0, 1),
        KeyCode::Esc => spreadsheet.cancel_editing(),
        KeyCode::Backspace => {
            spreadsheet.edit_buffer.pop();
            spreadsheet.formula_mode = spreadsheet.edit_buffer.starts_with('=');
        }
        KeyCode::Char(c) => {
            spreadsheet.handle_char_input(c);
        }
        KeyCode::Tab => spreadsheet.finish_editing_with_move(0, 1),
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

fn handle_ready_mode(
    spreadsheet: &mut Spreadsheet,
    code: KeyCode,
    modifiers: KeyModifiers,
) -> bool {
    let shift = modifiers.contains(KeyModifiers::SHIFT);
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
