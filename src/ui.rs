use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

use crate::constants::{
    CELL_BG, CELL_FG, CELL_NAME_BG, FORMULA_BAR_BG, FORMULA_BG, GRID_COLOR, HEADER_BG, HEADER_FG,
    REF_RANGE_BG, REF_SELECTION_BG, SELECTED_BG, SELECTED_HEADER_BG, FIND_MATCH_BG,
    DARK_CELL_BG, DARK_CELL_FG, DARK_CELL_NAME_BG, DARK_FORMULA_BAR_BG, DARK_FORMULA_BG,
    DARK_GRID_COLOR, DARK_HEADER_BG, DARK_HEADER_FG, DARK_REF_RANGE_BG, DARK_REF_SELECTION_BG,
    DARK_SELECTED_BG, DARK_SELECTED_HEADER_BG, DARK_FIND_MATCH_BG,
};
use crate::spreadsheet::Spreadsheet;
use crate::types::{DataType, RowColumnSelectMode, SaveFormat, TextAlignment, VerticalAlignment, VisualSubMode};

fn format_cell_by_type(value: &str, data_type: DataType) -> String {
    if value.is_empty() {
        return value.to_string();
    }

    match data_type {
        DataType::Text => value.to_string(),
        DataType::Number => {
            if let Ok(num) = value.parse::<f64>() {
                // Format number with appropriate decimal places
                if num.fract() == 0.0 {
                    format!("{:.0}", num)
                } else {
                    format!("{}", num)
                }
            } else {
                value.to_string()
            }
        }
        DataType::Currency => {
            if let Ok(num) = value.parse::<f64>() {
                if num.fract() == 0.0 {
                    format!("${:.0}", num)
                } else {
                    format!("${:.2}", num)
                }
            } else {
                value.to_string()
            }
        }
        DataType::Percentage => {
            if let Ok(num) = value.parse::<f64>() {
                format!("{:.1}%", num * 100.0)
            } else {
                value.to_string()
            }
        }
        DataType::Date => {
            // For now, just return as-is. Could add date parsing/formatting later
            value.to_string()
        }
        DataType::Time => {
            // For now, just return as-is. Could add time parsing/formatting later
            value.to_string()
        }
    }
}

pub fn render(f: &mut Frame, spreadsheet: &mut Spreadsheet) {
    let area = f.area();

    // Check if we have selection stats to show
    let has_stats = spreadsheet.get_selection_stats().is_some();

    let chunks = if has_stats {
        Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area)
    } else {
        Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(area)
    };

    let formula_bar_area = chunks[0];
    let grid_area = chunks[1];
    let (stats_area, status_area) = if has_stats {
        (Some(chunks[2]), chunks[3])
    } else {
        (None, chunks[2])
    };

    spreadsheet.adjust_scroll(grid_area);

    let visible_cols = spreadsheet.visible_cols(grid_area.width);
    let visible_rows = spreadsheet.visible_rows(area.height);

    render_formula_bar(f, spreadsheet, formula_bar_area);
    render_grid(f, spreadsheet, grid_area, visible_cols, visible_rows);
    if let Some(stats_area) = stats_area {
        render_stats_bar(f, spreadsheet, stats_area);
    }
    render_status_bar(f, spreadsheet, status_area);
}

fn render_formula_bar(f: &mut Frame, spreadsheet: &Spreadsheet, area: Rect) {
    let cell_content = spreadsheet.get_cell(spreadsheet.cursor_row, spreadsheet.cursor_col);
    let display_content = if spreadsheet.editing {
        spreadsheet.edit_buffer.clone()
    } else {
        cell_content.to_string()
    };

    // Choose colors based on dark mode
    let (formula_bar_bg, cell_name_bg, formula_bg, grid_color, text_fg) = if spreadsheet.dark_mode {
        (DARK_FORMULA_BAR_BG, DARK_CELL_NAME_BG, DARK_FORMULA_BG, DARK_GRID_COLOR, DARK_CELL_FG)
    } else {
        (FORMULA_BAR_BG, CELL_NAME_BG, FORMULA_BG, GRID_COLOR, CELL_FG)
    };

    let cell_name_width = 12;
    let formula_bar_inner = Layout::horizontal([
        Constraint::Length(cell_name_width),
        Constraint::Length(1),
        Constraint::Min(10),
    ])
    .split(Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: 1,
    });

    let formula_bar_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(grid_color))
        .style(Style::default().bg(formula_bar_bg));
    f.render_widget(formula_bar_block, area);

    let cell_name = Paragraph::new(spreadsheet.selection_ref())
        .style(Style::default().bg(cell_name_bg).fg(text_fg))
        .alignment(Alignment::Center);
    f.render_widget(cell_name, formula_bar_inner[0]);

    let sep = Paragraph::new("│").style(Style::default().fg(grid_color).bg(formula_bar_bg));
    f.render_widget(sep, formula_bar_inner[1]);

    let formula_display = if spreadsheet.editing {
        format!(" {}_", display_content)
    } else {
        format!(" {}", display_content)
    };
    let formula = Paragraph::new(formula_display)
        .style(Style::default().bg(formula_bg).fg(text_fg));
    f.render_widget(formula, formula_bar_inner[2]);
}

fn render_grid(
    f: &mut Frame,
    spreadsheet: &mut Spreadsheet,
    area: Rect,
    visible_cols: usize,
    visible_rows: usize,
) {
    // Choose colors based on dark mode
    let (header_bg, header_fg, selected_header_bg, selected_bg, grid_color, cell_bg, cell_fg, ref_selection_bg, ref_range_bg, find_match_bg) = 
        if spreadsheet.dark_mode {
            (DARK_HEADER_BG, DARK_HEADER_FG, DARK_SELECTED_HEADER_BG, DARK_SELECTED_BG, 
             DARK_GRID_COLOR, DARK_CELL_BG, DARK_CELL_FG, DARK_REF_SELECTION_BG, DARK_REF_RANGE_BG, DARK_FIND_MATCH_BG)
        } else {
            (HEADER_BG, HEADER_FG, SELECTED_HEADER_BG, SELECTED_BG,
             GRID_COLOR, CELL_BG, CELL_FG, REF_SELECTION_BG, REF_RANGE_BG, FIND_MATCH_BG)
        };

    let mut header_cells = vec![Cell::from("").style(Style::default().bg(header_bg))];
    for col in spreadsheet.scroll_col..spreadsheet.scroll_col + visible_cols {
        if col < spreadsheet.num_cols {
            let is_current_col = if spreadsheet.selecting_ref {
                col == spreadsheet.ref_cursor_col
            } else {
                col == spreadsheet.cursor_col
            };
            let is_selected_col = if let Some((min_col, max_col)) = spreadsheet.selected_cols {
                col >= min_col && col <= max_col
            } else {
                false
            };
            let bg = if is_current_col || is_selected_col { 
                selected_header_bg 
            } else { 
                header_bg 
            };
            header_cells.push(
                Cell::from(Spreadsheet::col_name(col)).style(
                    Style::default()
                        .bg(bg)
                        .fg(header_fg)
                        .add_modifier(Modifier::BOLD),
                ),
            );
        }
    }
    let header = Row::new(header_cells).height(1);

    let mut rows = Vec::new();
    for row in spreadsheet.scroll_row..spreadsheet.scroll_row + visible_rows {
        if row >= spreadsheet.num_rows {
            break;
        }

        let is_current_row = if spreadsheet.selecting_ref {
            row == spreadsheet.ref_cursor_row
        } else {
            row == spreadsheet.cursor_row
        };
        let is_selected_row = if let Some((min_row, max_row)) = spreadsheet.selected_rows {
            row >= min_row && row <= max_row
        } else {
            false
        };
        let row_header_bg = if is_current_row || is_selected_row { 
            selected_header_bg 
        } else { 
            header_bg 
        };

        let mut row_cells = vec![Cell::from(format!("{}", row + 1)).style(
            Style::default()
                .bg(row_header_bg)
                .fg(header_fg)
                .add_modifier(Modifier::BOLD),
        )];

        let ref_range = spreadsheet.get_ref_range();
        let selection_range = spreadsheet.get_selection_range();

        for col in spreadsheet.scroll_col..spreadsheet.scroll_col + visible_cols {
            if col >= spreadsheet.num_cols {
                break;
            }

            let is_cursor = row == spreadsheet.cursor_row && col == spreadsheet.cursor_col;
            let evaluated = {
                // Evaluate cell - this may modify the spreadsheet for SHELL formulas
                spreadsheet.evaluate_cell(row, col)
            };

            let cell_style = spreadsheet.get_cell_style(row, col);
            
            // Format content based on data type
            let formatted_content = if let Some(data_type) = cell_style.data_type {
                format_cell_by_type(&evaluated, data_type)
            } else {
                evaluated.clone()
            };

            let content = if is_cursor && spreadsheet.editing {
                format!("{}_", spreadsheet.edit_buffer)
            } else {
                formatted_content.clone()
            };

            let is_in_ref_range = if let Some(((min_row, min_col), (max_row, max_col))) = ref_range
            {
                row >= min_row && row <= max_row && col >= min_col && col <= max_col
            } else {
                false
            };

            let is_in_selection =
                if let Some(((min_row, min_col), (max_row, max_col))) = selection_range {
                    row >= min_row && row <= max_row && col >= min_col && col <= max_col
                } else {
                    false
                };

            let is_in_selected_row = if let Some((min_row, max_row)) = spreadsheet.selected_rows {
                row >= min_row && row <= max_row
            } else {
                false
            };

            let is_in_selected_col = if let Some((min_col, max_col)) = spreadsheet.selected_cols {
                col >= min_col && col <= max_col
            } else {
                false
            };

            let is_ref_cursor = spreadsheet.selecting_ref
                && row == spreadsheet.ref_cursor_row
                && col == spreadsheet.ref_cursor_col;

            let col_width = spreadsheet.get_col_width(col);
            let row_height = spreadsheet.get_row_height(row);

            let is_number = Spreadsheet::is_numeric(&formatted_content);
            // Determine alignment: use cell style if set, otherwise use data type default, 
            // or fall back to number/text detection
            let alignment = cell_style.alignment.unwrap_or_else(|| {
                if let Some(data_type) = cell_style.data_type {
                    match data_type {
                        DataType::Text => TextAlignment::Left,
                        DataType::Number | DataType::Currency | DataType::Percentage => TextAlignment::Right,
                        _ => if is_number { TextAlignment::Right } else { TextAlignment::Left },
                    }
                } else {
                    // No data type set, use number/text detection
                    if is_number {
                        TextAlignment::Right
                    } else {
                        TextAlignment::Left
                    }
                }
            });
            
            // Determine vertical alignment: use cell style if set, otherwise default to Top
            let vertical_alignment = cell_style.vertical_alignment.unwrap_or(VerticalAlignment::Top);
            
            let aligned_content = if !content.is_empty() {
                let width = (col_width - 1) as usize;
                let horizontal_line = match alignment {
                    TextAlignment::Left => format!(" {}", content),
                    TextAlignment::Center => {
                        let padding = width.saturating_sub(content.len());
                        let left_pad = padding / 2;
                        let right_pad = padding - left_pad;
                        format!("{}{}{}", " ".repeat(left_pad), content, " ".repeat(right_pad))
                    }
                    TextAlignment::Right => format!("{:>width$}", content, width = width),
                };
                
                // Apply vertical alignment by padding with empty lines
                let empty_line = " ".repeat(width + 1); // +1 for the leading space
                let height = row_height as usize;
                
                if height == 1 {
                    horizontal_line
                } else {
                    match vertical_alignment {
                        VerticalAlignment::Top => {
                            let mut result = horizontal_line;
                            for _ in 1..height {
                                result.push('\n');
                                result.push_str(&empty_line);
                            }
                            result
                        }
                        VerticalAlignment::Center => {
                            let top_lines = (height - 1) / 2;
                            let bottom_lines = height - 1 - top_lines;
                            let mut result = String::new();
                            for _ in 0..top_lines {
                                result.push_str(&empty_line);
                                result.push('\n');
                            }
                            result.push_str(&horizontal_line);
                            for _ in 0..bottom_lines {
                                result.push('\n');
                                result.push_str(&empty_line);
                            }
                            result
                        }
                        VerticalAlignment::Bottom => {
                            let mut result = String::new();
                            for _ in 1..height {
                                result.push_str(&empty_line);
                                result.push('\n');
                            }
                            result.push_str(&horizontal_line);
                            result
                        }
                    }
                }
            } else {
                // Empty content - still need to pad for vertical alignment if height > 1
                let empty_line = " ".repeat((col_width - 1) as usize + 1);
                let height = row_height as usize;
                
                if height == 1 {
                    format!(" {}", content)
                } else {
                    let mut result = String::new();
                    for i in 0..height {
                        if i > 0 {
                            result.push('\n');
                        }
                        result.push_str(&empty_line);
                    }
                    result
                }
            };

            // Use explicit foreground color if set, otherwise default based on dark mode
            let fg_color = cell_style.fg.unwrap_or(cell_fg);
            
            // Check if this cell is a find match (but not the cursor)
            let is_find_match = spreadsheet.is_find_match(row, col);
            
            let mut style = if is_cursor && !spreadsheet.selecting_ref {
                Style::default()
                    .bg(selected_bg)
                    .fg(fg_color)
            } else if is_ref_cursor {
                Style::default()
                    .bg(ref_selection_bg)
                    .fg(fg_color)
            } else if is_in_ref_range {
                Style::default()
                    .bg(ref_range_bg)
                    .fg(fg_color)
            } else if is_in_selected_row || is_in_selected_col {
                // Highlight selected rows or columns
                Style::default()
                    .bg(selected_bg)
                    .fg(fg_color)
            } else if is_in_selection {
                Style::default()
                    .bg(selected_bg)
                    .fg(fg_color)
            } else if is_find_match {
                // Highlight find matches with light yellow background
                Style::default()
                    .bg(find_match_bg)
                    .fg(Color::Black) // Use black text for visibility on yellow
            } else {
                Style::default()
                    .bg(cell_style.bg.unwrap_or(cell_bg))
                    .fg(fg_color)
            };
            
            // Apply bold modifier from cell style
            if cell_style.bold {
                style = style.add_modifier(Modifier::BOLD);
            }
            // Cursor and ref cursor are always bold for visibility
            if is_cursor || is_ref_cursor {
                style = style.add_modifier(Modifier::BOLD);
            }

            row_cells.push(Cell::from(aligned_content).style(style));
        }
        let row_height = spreadsheet.get_row_height(row);
        rows.push(Row::new(row_cells).height(row_height));
    }

    let mut widths = vec![Constraint::Length(5)];
    for col in spreadsheet.scroll_col..spreadsheet.scroll_col + visible_cols {
        widths.push(Constraint::Length(spreadsheet.get_col_width(col)));
    }

    let table = Table::new(rows, &widths)
        .header(header)
        .column_spacing(0)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(grid_color))
                .style(Style::default().bg(cell_bg)),
        );

    f.render_widget(table, area);
}

fn render_stats_bar(f: &mut Frame, spreadsheet: &mut Spreadsheet, area: Rect) {
    if let Some((row_count, cell_count, numeric_count, sum)) = spreadsheet.get_selection_stats() {
        let mut spans = vec![
            Span::styled("  Rows: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}", row_count), Style::default().fg(Color::White)),
            Span::styled("  Cells: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}", cell_count), Style::default().fg(Color::White)),
        ];

        if numeric_count > 0 {
            spans.push(Span::styled("  Sum: ", Style::default().fg(Color::DarkGray)));
            // Format sum nicely - remove trailing zeros for integers
            let sum_str = if sum.fract() == 0.0 {
                format!("{:.0}", sum)
            } else {
                format!("{}", sum)
            };
            spans.push(Span::styled(sum_str, Style::default().fg(Color::Cyan)));
            
            // Also show average
            let avg = sum / numeric_count as f64;
            spans.push(Span::styled("  Avg: ", Style::default().fg(Color::DarkGray)));
            let avg_str = if avg.fract() == 0.0 {
                format!("{:.0}", avg)
            } else {
                format!("{:.2}", avg)
            };
            spans.push(Span::styled(avg_str, Style::default().fg(Color::Cyan)));
        }

        let stats_line = Line::from(spans);
        f.render_widget(
            Paragraph::new(stats_line).style(Style::default().bg(Color::Rgb(35, 35, 35))),
            area,
        );
    }
}

fn render_status_bar(f: &mut Frame, spreadsheet: &Spreadsheet, area: Rect) {
    let (mode, mode_style) = if spreadsheet.command_mode {
        (
            " COMMAND ",
            Style::default().bg(Color::Rgb(138, 43, 226)).fg(Color::White),
        )
    } else if spreadsheet.open_mode {
        (
            " OPEN ",
            Style::default().bg(Color::Rgb(0, 100, 200)).fg(Color::White),
        )
    } else if spreadsheet.save_mode {
        (
            " SAVE ",
            Style::default().bg(Color::Rgb(220, 20, 60)).fg(Color::White),
        )
    } else if spreadsheet.find_mode {
        (
            " FIND ",
            Style::default().bg(Color::Rgb(255, 200, 0)).fg(Color::Black),
        )
    } else if spreadsheet.row_column_select_mode == RowColumnSelectMode::RowSelect {
        (
            " ROW SELECT ",
            Style::default().bg(Color::Rgb(200, 100, 0)).fg(Color::White),
        )
    } else if spreadsheet.row_column_select_mode == RowColumnSelectMode::ColumnSelect {
        (
            " COL SELECT ",
            Style::default().bg(Color::Rgb(200, 100, 0)).fg(Color::White),
        )
    } else if spreadsheet.visual_mode {
        (
            " VISUAL ",
            Style::default().bg(Color::Rgb(255, 140, 0)).fg(Color::Black),
        )
    } else if spreadsheet.selecting_ref {
        (
            " SELECT ",
            Style::default().bg(Color::Rgb(128, 0, 128)).fg(Color::White),
        )
    } else if spreadsheet.editing {
        (
            " EDIT ",
            Style::default().bg(Color::Rgb(34, 139, 34)).fg(Color::White),
        )
    } else {
        (
            " READY ",
            Style::default().bg(Color::Rgb(70, 130, 180)).fg(Color::White),
        )
    };

    let status = if spreadsheet.command_mode {
        render_command_status(spreadsheet, mode, mode_style)
    } else if spreadsheet.open_mode {
        render_open_status(spreadsheet, mode, mode_style)
    } else if spreadsheet.save_mode {
        render_save_status(spreadsheet, mode, mode_style)
    } else if spreadsheet.find_mode {
        render_find_status(spreadsheet, mode, mode_style)
    } else if spreadsheet.row_column_select_mode == RowColumnSelectMode::RowSelect {
        render_row_select_status(spreadsheet, mode, mode_style)
    } else if spreadsheet.row_column_select_mode == RowColumnSelectMode::ColumnSelect {
        render_column_select_status(spreadsheet, mode, mode_style)
    } else if spreadsheet.visual_mode {
        render_visual_status(spreadsheet, mode, mode_style)
    } else if spreadsheet.selecting_ref {
        render_select_status(mode, mode_style)
    } else {
        render_ready_status(mode, mode_style)
    };

    f.render_widget(
        Paragraph::new(status).style(Style::default().bg(Color::Rgb(45, 45, 45))),
        area,
    );
}

fn render_find_status<'a>(spreadsheet: &Spreadsheet, mode: &'a str, mode_style: Style) -> Line<'a> {
    let match_count = spreadsheet.find_matches.len();
    let match_info = if match_count == 0 {
        "No matches".to_string()
    } else if match_count == 1 {
        "1 match".to_string()
    } else {
        format!("{} matches", match_count)
    };
    
    Line::from(vec![
        Span::styled(mode, mode_style),
        Span::styled("  Search: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{}_", spreadsheet.find_query),
            Style::default().fg(Color::White),
        ),
        Span::styled("  ", Style::default().fg(Color::DarkGray)),
        Span::styled(match_info, Style::default().fg(Color::Cyan)),
        Span::styled("  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Enter", Style::default().fg(Color::Yellow)),
        Span::styled(" confirm  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Esc", Style::default().fg(Color::Yellow)),
        Span::styled(" cancel", Style::default().fg(Color::DarkGray)),
    ])
}

fn render_command_status<'a>(spreadsheet: &Spreadsheet, mode: &'a str, mode_style: Style) -> Line<'a> {
    let msg = spreadsheet.command_message.as_deref().unwrap_or("");
    Line::from(vec![
        Span::styled(mode, mode_style),
        Span::styled("  :", Style::default().fg(Color::White)),
        Span::styled(
            format!("{}_", spreadsheet.command_buffer),
            Style::default().fg(Color::White),
        ),
        Span::styled("  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Enter", Style::default().fg(Color::Yellow)),
        Span::styled(" execute  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Esc", Style::default().fg(Color::Yellow)),
        Span::styled(" cancel  ", Style::default().fg(Color::DarkGray)),
        Span::styled("(e.g. A1, B23, q)", Style::default().fg(Color::Cyan)),
        if !msg.is_empty() {
            Span::styled(format!("  {}", msg), Style::default().fg(Color::Red))
        } else {
            Span::raw("")
        },
    ])
}

fn render_open_status<'a>(spreadsheet: &Spreadsheet, mode: &'a str, mode_style: Style) -> Line<'a> {
    let msg = spreadsheet.open_message.as_deref().unwrap_or("");
    Line::from(vec![
        Span::styled(mode, mode_style),
        Span::styled("  File: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{}_", spreadsheet.open_filename),
            Style::default().fg(Color::White),
        ),
        Span::styled("  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Enter", Style::default().fg(Color::Yellow)),
        Span::styled(" to open, ", Style::default().fg(Color::DarkGray)),
        Span::styled("Esc", Style::default().fg(Color::Yellow)),
        Span::styled(" to cancel", Style::default().fg(Color::DarkGray)),
        if !msg.is_empty() {
            Span::styled(format!("  {}", msg), Style::default().fg(Color::Red))
        } else {
            Span::raw("")
        },
    ])
}

fn render_save_status<'a>(spreadsheet: &Spreadsheet, mode: &'a str, mode_style: Style) -> Line<'a> {
    let msg = spreadsheet.save_message.as_deref().unwrap_or("");
    let ext = match spreadsheet.save_format {
        SaveFormat::Csv => ".csv",
        SaveFormat::Tsv => ".tsv",
    };
    Line::from(vec![
        Span::styled(mode, mode_style),
        Span::styled("  File: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{}_", spreadsheet.save_filename),
            Style::default().fg(Color::White),
        ),
        Span::styled(ext, Style::default().fg(Color::Cyan)),
        Span::styled("  ", Style::default().fg(Color::DarkGray)),
        Span::styled("1", Style::default().fg(Color::Yellow)),
        Span::styled(
            if spreadsheet.save_format == SaveFormat::Csv {
                "-CSV* "
            } else {
                "-CSV "
            },
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled("2", Style::default().fg(Color::Yellow)),
        Span::styled(
            if spreadsheet.save_format == SaveFormat::Tsv {
                "-TSV* "
            } else {
                "-TSV "
            },
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled("Enter", Style::default().fg(Color::White)),
        Span::styled("-Save ", Style::default().fg(Color::DarkGray)),
        Span::styled("Esc", Style::default().fg(Color::White)),
        Span::styled("-Cancel ", Style::default().fg(Color::DarkGray)),
        Span::styled(msg.to_string(), Style::default().fg(Color::Green)),
    ])
}

fn render_visual_status<'a>(
    spreadsheet: &Spreadsheet,
    mode: &'a str,
    mode_style: Style,
) -> Line<'a> {
    match spreadsheet.visual_sub_mode {
        VisualSubMode::Main => {
            let mode_label = if spreadsheet.dark_mode { "Dark" } else { "Light" };
            Line::from(vec![
                Span::styled(mode, mode_style),
                Span::styled("  f", Style::default().fg(Color::White)),
                Span::styled(" Text  ", Style::default().fg(Color::DarkGray)),
                Span::styled("b", Style::default().fg(Color::White)),
                Span::styled(" Bg  ", Style::default().fg(Color::DarkGray)),
                Span::styled("s", Style::default().fg(Color::White)),
                Span::styled(" Size  ", Style::default().fg(Color::DarkGray)),
                Span::styled("a", Style::default().fg(Color::White)),
                Span::styled(" Align  ", Style::default().fg(Color::DarkGray)),
                Span::styled("v", Style::default().fg(Color::White)),
                Span::styled(" VAlign  ", Style::default().fg(Color::DarkGray)),
                Span::styled("w", Style::default().fg(Color::White)),
                Span::styled(" Width  ", Style::default().fg(Color::DarkGray)),
                Span::styled("h", Style::default().fg(Color::White)),
                Span::styled(" Height  ", Style::default().fg(Color::DarkGray)),
                Span::styled("t", Style::default().fg(Color::White)),
                Span::styled(" Type  ", Style::default().fg(Color::DarkGray)),
                Span::styled("c", Style::default().fg(Color::White)),
                Span::styled(" Clear  ", Style::default().fg(Color::DarkGray)),
                Span::styled("m", Style::default().fg(Color::White)),
                Span::styled(format!(" {} ", mode_label), Style::default().fg(Color::DarkGray)),
                Span::styled("Esc", Style::default().fg(Color::White)),
                Span::styled(" Exit", Style::default().fg(Color::DarkGray)),
            ])
        }
        VisualSubMode::TextColor | VisualSubMode::BackgroundColor => {
            let label = if spreadsheet.visual_sub_mode == VisualSubMode::TextColor {
                "Text Color: "
            } else {
                "Background: "
            };
            Line::from(vec![
                Span::styled(mode, mode_style),
                Span::styled(format!("  {}", label), Style::default().fg(Color::DarkGray)),
                Span::styled("0", Style::default().fg(Color::White)),
                Span::styled("-White ", Style::default().fg(Color::DarkGray)),
                Span::styled("1", Style::default().fg(Color::White)),
                Span::styled("-Black ", Style::default().fg(Color::DarkGray)),
                Span::styled("2", Style::default().fg(Color::Red)),
                Span::styled("-Red ", Style::default().fg(Color::DarkGray)),
                Span::styled("3", Style::default().fg(Color::Green)),
                Span::styled("-Green ", Style::default().fg(Color::DarkGray)),
                Span::styled("4", Style::default().fg(Color::Blue)),
                Span::styled("-Blue ", Style::default().fg(Color::DarkGray)),
                Span::styled("5", Style::default().fg(Color::Yellow)),
                Span::styled("-Yel ", Style::default().fg(Color::DarkGray)),
                Span::styled("Esc", Style::default().fg(Color::White)),
                Span::styled(" Back", Style::default().fg(Color::DarkGray)),
            ])
        }
        VisualSubMode::ColumnWidth => Line::from(vec![
            Span::styled(mode, mode_style),
            Span::styled("  Column Width: ", Style::default().fg(Color::DarkGray)),
            Span::styled("←→", Style::default().fg(Color::White)),
            Span::styled(" Adjust  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("Current: {} ", spreadsheet.get_col_width(spreadsheet.cursor_col)),
                Style::default().fg(Color::White),
            ),
            Span::styled("Esc", Style::default().fg(Color::White)),
            Span::styled(" Back", Style::default().fg(Color::DarkGray)),
        ]),
        VisualSubMode::RowHeight => Line::from(vec![
            Span::styled(mode, mode_style),
            Span::styled("  Row Height: ", Style::default().fg(Color::DarkGray)),
            Span::styled("↑↓", Style::default().fg(Color::White)),
            Span::styled(" Adjust  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("Current: {} ", spreadsheet.get_row_height(spreadsheet.cursor_row)),
                Style::default().fg(Color::White),
            ),
            Span::styled("Esc", Style::default().fg(Color::White)),
            Span::styled(" Back", Style::default().fg(Color::DarkGray)),
        ]),
        VisualSubMode::TextAlignment => Line::from(vec![
            Span::styled(mode, mode_style),
            Span::styled("  Text Alignment: ", Style::default().fg(Color::DarkGray)),
            Span::styled("1", Style::default().fg(Color::White)),
            Span::styled("-Left ", Style::default().fg(Color::DarkGray)),
            Span::styled("2", Style::default().fg(Color::White)),
            Span::styled("-Center ", Style::default().fg(Color::DarkGray)),
            Span::styled("3", Style::default().fg(Color::White)),
            Span::styled("-Right ", Style::default().fg(Color::DarkGray)),
            Span::styled("0", Style::default().fg(Color::White)),
            Span::styled("-Default ", Style::default().fg(Color::DarkGray)),
            Span::styled("Esc", Style::default().fg(Color::White)),
            Span::styled(" Back", Style::default().fg(Color::DarkGray)),
        ]),
        VisualSubMode::VerticalAlignment => Line::from(vec![
            Span::styled(mode, mode_style),
            Span::styled("  Vertical Alignment: ", Style::default().fg(Color::DarkGray)),
            Span::styled("1", Style::default().fg(Color::White)),
            Span::styled("-Top ", Style::default().fg(Color::DarkGray)),
            Span::styled("2", Style::default().fg(Color::White)),
            Span::styled("-Center ", Style::default().fg(Color::DarkGray)),
            Span::styled("3", Style::default().fg(Color::White)),
            Span::styled("-Bottom ", Style::default().fg(Color::DarkGray)),
            Span::styled("0", Style::default().fg(Color::White)),
            Span::styled("-Default ", Style::default().fg(Color::DarkGray)),
            Span::styled("Esc", Style::default().fg(Color::White)),
            Span::styled(" Back", Style::default().fg(Color::DarkGray)),
        ]),
        VisualSubMode::FontSize => Line::from(vec![
            Span::styled(mode, mode_style),
            Span::styled("  Font Size: ", Style::default().fg(Color::DarkGray)),
            Span::styled("+/↑", Style::default().fg(Color::White)),
            Span::styled(" Increase (Bold)  ", Style::default().fg(Color::DarkGray)),
            Span::styled("-/↓", Style::default().fg(Color::White)),
            Span::styled(" Decrease (Normal)  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Esc", Style::default().fg(Color::White)),
            Span::styled(" Back", Style::default().fg(Color::DarkGray)),
        ]),
        VisualSubMode::DataType => Line::from(vec![
            Span::styled(mode, mode_style),
            Span::styled("  Data Type: ", Style::default().fg(Color::DarkGray)),
            Span::styled("1", Style::default().fg(Color::White)),
            Span::styled("-Text ", Style::default().fg(Color::DarkGray)),
            Span::styled("2", Style::default().fg(Color::White)),
            Span::styled("-Number ", Style::default().fg(Color::DarkGray)),
            Span::styled("3", Style::default().fg(Color::White)),
            Span::styled("-Currency ", Style::default().fg(Color::DarkGray)),
            Span::styled("4", Style::default().fg(Color::White)),
            Span::styled("-Percentage ", Style::default().fg(Color::DarkGray)),
            Span::styled("5", Style::default().fg(Color::White)),
            Span::styled("-Date ", Style::default().fg(Color::DarkGray)),
            Span::styled("6", Style::default().fg(Color::White)),
            Span::styled("-Time ", Style::default().fg(Color::DarkGray)),
            Span::styled("0", Style::default().fg(Color::White)),
            Span::styled("-Default ", Style::default().fg(Color::DarkGray)),
            Span::styled("Esc", Style::default().fg(Color::White)),
            Span::styled(" Back", Style::default().fg(Color::DarkGray)),
        ]),
    }
}

fn render_select_status<'a>(mode: &'a str, mode_style: Style) -> Line<'a> {
    Line::from(vec![
        Span::styled(mode, mode_style),
        Span::styled("  ←↑↓→ Select Cell  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Shift+Arrow", Style::default().fg(Color::White)),
        Span::styled(" Range  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Enter", Style::default().fg(Color::White)),
        Span::styled(" Confirm  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Esc", Style::default().fg(Color::White)),
        Span::styled(" Cancel", Style::default().fg(Color::DarkGray)),
    ])
}

fn render_row_select_status<'a>(
    spreadsheet: &Spreadsheet,
    mode: &'a str,
    mode_style: Style,
) -> Line<'a> {
    let row_info = if let Some((min_row, max_row)) = spreadsheet.selected_rows {
        if min_row == max_row {
            format!("Row {}", min_row + 1)
        } else {
            format!("Rows {}:{}", min_row + 1, max_row + 1)
        }
    } else {
        "No selection".to_string()
    };
    Line::from(vec![
        Span::styled(mode, mode_style),
        Span::styled(format!("  {}  ", row_info), Style::default().fg(Color::White)),
        Span::styled("↑↓", Style::default().fg(Color::White)),
        Span::styled(" Select  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Shift+↑↓", Style::default().fg(Color::White)),
        Span::styled(" Extend  ", Style::default().fg(Color::DarkGray)),
        Span::styled("d", Style::default().fg(Color::White)),
        Span::styled(" Delete  ", Style::default().fg(Color::DarkGray)),
        Span::styled("i", Style::default().fg(Color::White)),
        Span::styled(" Insert  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Esc", Style::default().fg(Color::White)),
        Span::styled(" Exit", Style::default().fg(Color::DarkGray)),
    ])
}

fn render_column_select_status<'a>(
    spreadsheet: &Spreadsheet,
    mode: &'a str,
    mode_style: Style,
) -> Line<'a> {
    let col_info = if let Some((min_col, max_col)) = spreadsheet.selected_cols {
        let min_name = Spreadsheet::col_name(min_col);
        let max_name = Spreadsheet::col_name(max_col);
        if min_col == max_col {
            format!("Column {}", min_name)
        } else {
            format!("Columns {}:{}", min_name, max_name)
        }
    } else {
        "No selection".to_string()
    };
    Line::from(vec![
        Span::styled(mode, mode_style),
        Span::styled(format!("  {}  ", col_info), Style::default().fg(Color::White)),
        Span::styled("←→", Style::default().fg(Color::White)),
        Span::styled(" Select  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Shift+←→", Style::default().fg(Color::White)),
        Span::styled(" Extend  ", Style::default().fg(Color::DarkGray)),
        Span::styled("d", Style::default().fg(Color::White)),
        Span::styled(" Delete  ", Style::default().fg(Color::DarkGray)),
        Span::styled("i", Style::default().fg(Color::White)),
        Span::styled(" Insert  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Esc", Style::default().fg(Color::White)),
        Span::styled(" Exit", Style::default().fg(Color::DarkGray)),
    ])
}

fn render_ready_status<'a>(mode: &'a str, mode_style: Style) -> Line<'a> {
    Line::from(vec![
        Span::styled(mode, mode_style),
        Span::styled("  ←↑↓→ Navigate  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Enter", Style::default().fg(Color::White)),
        Span::styled(" Edit  ", Style::default().fg(Color::DarkGray)),
        Span::styled("f", Style::default().fg(Color::White)),
        Span::styled(" Find  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Tab", Style::default().fg(Color::White)),
        Span::styled(" Visual  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Shift+R", Style::default().fg(Color::White)),
        Span::styled(" Row  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Shift+C", Style::default().fg(Color::White)),
        Span::styled(" Col  ", Style::default().fg(Color::DarkGray)),
        Span::styled("o", Style::default().fg(Color::White)),
        Span::styled(" Open  ", Style::default().fg(Color::DarkGray)),
        Span::styled("s", Style::default().fg(Color::White)),
        Span::styled(" Save  ", Style::default().fg(Color::DarkGray)),
        Span::styled("t", Style::default().fg(Color::White)),
        Span::styled(" Table  ", Style::default().fg(Color::DarkGray)),
        Span::styled("q", Style::default().fg(Color::White)),
        Span::styled(" Quit", Style::default().fg(Color::DarkGray)),
    ])
}
