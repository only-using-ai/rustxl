use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::constants::COLOR_PALETTE;
use crate::spreadsheet::Spreadsheet;
use crate::types::{SaveFormat, VisualSubMode};
use crate::ui;

pub fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let mut spreadsheet = Spreadsheet::new();

    loop {
        terminal.draw(|f| ui::render(f, &mut spreadsheet))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            if spreadsheet.editing {
                handle_editing_mode(&mut spreadsheet, key.code, key.modifiers);
            } else if spreadsheet.save_mode {
                if handle_save_mode(&mut spreadsheet, key.code) {
                    continue;
                }
            } else if spreadsheet.visual_mode {
                handle_visual_mode(&mut spreadsheet, key.code);
            } else {
                if handle_ready_mode(&mut spreadsheet, key.code, key.modifiers) {
                    return Ok(());
                }
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
        KeyCode::Char('w') | KeyCode::Char('W') => {
            spreadsheet.visual_sub_mode = VisualSubMode::ColumnWidth;
        }
        KeyCode::Char('h') | KeyCode::Char('H') => {
            spreadsheet.visual_sub_mode = VisualSubMode::RowHeight;
        }
        KeyCode::Char('c') | KeyCode::Char('C') => {
            spreadsheet.clear_formatting_from_selection();
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

fn handle_ready_mode(
    spreadsheet: &mut Spreadsheet,
    code: KeyCode,
    modifiers: KeyModifiers,
) -> bool {
    let shift = modifiers.contains(KeyModifiers::SHIFT);
    match code {
        KeyCode::Char('q') | KeyCode::Char('Q') => return true,
        KeyCode::Char('s') | KeyCode::Char('S') => spreadsheet.enter_save_mode(),
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
