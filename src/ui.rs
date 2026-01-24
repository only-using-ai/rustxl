use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

use crate::constants::{
    CELL_NAME_BG, FORMULA_BAR_BG, FORMULA_BG, GRID_COLOR, HEADER_BG, HEADER_FG,
    REF_RANGE_BG, REF_SELECTION_BG, SELECTED_BG, SELECTED_HEADER_BG,
};
use crate::spreadsheet::Spreadsheet;
use crate::types::{SaveFormat, VisualSubMode};

pub fn render(f: &mut Frame, spreadsheet: &mut Spreadsheet) {
    let area = f.area();

    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(5),
        Constraint::Length(1),
    ])
    .split(area);

    let formula_bar_area = chunks[0];
    let grid_area = chunks[1];
    let status_area = chunks[2];

    spreadsheet.adjust_scroll(grid_area);

    let visible_cols = spreadsheet.visible_cols(grid_area.width);
    let visible_rows = spreadsheet.visible_rows(area.height);

    render_formula_bar(f, spreadsheet, formula_bar_area);
    render_grid(f, spreadsheet, grid_area, visible_cols, visible_rows);
    render_status_bar(f, spreadsheet, status_area);
}

fn render_formula_bar(f: &mut Frame, spreadsheet: &Spreadsheet, area: Rect) {
    let cell_content = spreadsheet.get_cell(spreadsheet.cursor_row, spreadsheet.cursor_col);
    let display_content = if spreadsheet.editing {
        spreadsheet.edit_buffer.clone()
    } else {
        cell_content.to_string()
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
        .border_style(Style::default().fg(GRID_COLOR))
        .style(Style::default().bg(FORMULA_BAR_BG));
    f.render_widget(formula_bar_block, area);

    let cell_name = Paragraph::new(spreadsheet.selection_ref())
        .style(Style::default().bg(CELL_NAME_BG).fg(Color::Black))
        .alignment(Alignment::Center);
    f.render_widget(cell_name, formula_bar_inner[0]);

    let sep = Paragraph::new("│").style(Style::default().fg(GRID_COLOR).bg(FORMULA_BAR_BG));
    f.render_widget(sep, formula_bar_inner[1]);

    let formula_display = if spreadsheet.editing {
        format!(" {}_", display_content)
    } else {
        format!(" {}", display_content)
    };
    let formula = Paragraph::new(formula_display)
        .style(Style::default().bg(FORMULA_BG).fg(Color::Black));
    f.render_widget(formula, formula_bar_inner[2]);
}

fn render_grid(
    f: &mut Frame,
    spreadsheet: &Spreadsheet,
    area: Rect,
    visible_cols: usize,
    visible_rows: usize,
) {
    let mut header_cells = vec![Cell::from("").style(Style::default().bg(HEADER_BG))];
    for col in spreadsheet.scroll_col..spreadsheet.scroll_col + visible_cols {
        if col < spreadsheet.num_cols {
            let is_current_col = if spreadsheet.selecting_ref {
                col == spreadsheet.ref_cursor_col
            } else {
                col == spreadsheet.cursor_col
            };
            let bg = if is_current_col { SELECTED_HEADER_BG } else { HEADER_BG };
            header_cells.push(
                Cell::from(Spreadsheet::col_name(col)).style(
                    Style::default()
                        .bg(bg)
                        .fg(HEADER_FG)
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
        let row_header_bg = if is_current_row { SELECTED_HEADER_BG } else { HEADER_BG };

        let mut row_cells = vec![Cell::from(format!("{}", row + 1)).style(
            Style::default()
                .bg(row_header_bg)
                .fg(HEADER_FG)
                .add_modifier(Modifier::BOLD),
        )];

        let ref_range = spreadsheet.get_ref_range();
        let selection_range = spreadsheet.get_selection_range();

        for col in spreadsheet.scroll_col..spreadsheet.scroll_col + visible_cols {
            if col >= spreadsheet.num_cols {
                break;
            }

            let is_cursor = row == spreadsheet.cursor_row && col == spreadsheet.cursor_col;
            let evaluated = spreadsheet.evaluate_cell(row, col);
            let content = if is_cursor && spreadsheet.editing && !spreadsheet.selecting_ref {
                format!("{}_", spreadsheet.edit_buffer)
            } else {
                evaluated.clone()
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

            let is_ref_cursor = spreadsheet.selecting_ref
                && row == spreadsheet.ref_cursor_row
                && col == spreadsheet.ref_cursor_col;

            let cell_style = spreadsheet.get_cell_style(row, col);
            let col_width = spreadsheet.get_col_width(col);

            let is_number = Spreadsheet::is_numeric(&evaluated);
            let aligned_content = if is_number && !content.is_empty() {
                format!("{:>width$}", content, width = (col_width - 1) as usize)
            } else {
                format!(" {}", content)
            };

            let style = if is_cursor && !spreadsheet.selecting_ref {
                Style::default()
                    .bg(SELECTED_BG)
                    .fg(cell_style.fg.unwrap_or(Color::Black))
                    .add_modifier(Modifier::BOLD)
            } else if is_ref_cursor {
                Style::default()
                    .bg(REF_SELECTION_BG)
                    .fg(cell_style.fg.unwrap_or(Color::Black))
                    .add_modifier(Modifier::BOLD)
            } else if is_in_ref_range {
                Style::default()
                    .bg(REF_RANGE_BG)
                    .fg(cell_style.fg.unwrap_or(Color::Black))
            } else if is_in_selection {
                Style::default()
                    .bg(SELECTED_BG)
                    .fg(cell_style.fg.unwrap_or(Color::Black))
            } else {
                Style::default()
                    .bg(cell_style.bg.unwrap_or(Color::White))
                    .fg(cell_style.fg.unwrap_or(Color::Black))
            };

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
                .border_style(Style::default().fg(GRID_COLOR))
                .style(Style::default().bg(Color::White)),
        );

    f.render_widget(table, area);
}

fn render_status_bar(f: &mut Frame, spreadsheet: &Spreadsheet, area: Rect) {
    let (mode, mode_style) = if spreadsheet.save_mode {
        (
            " SAVE ",
            Style::default().bg(Color::Rgb(220, 20, 60)).fg(Color::White),
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

    let status = if spreadsheet.save_mode {
        render_save_status(spreadsheet, mode, mode_style)
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
        VisualSubMode::Main => Line::from(vec![
            Span::styled(mode, mode_style),
            Span::styled("  f", Style::default().fg(Color::White)),
            Span::styled(" Text  ", Style::default().fg(Color::DarkGray)),
            Span::styled("b", Style::default().fg(Color::White)),
            Span::styled(" Bg  ", Style::default().fg(Color::DarkGray)),
            Span::styled("w", Style::default().fg(Color::White)),
            Span::styled(" Width  ", Style::default().fg(Color::DarkGray)),
            Span::styled("h", Style::default().fg(Color::White)),
            Span::styled(" Height  ", Style::default().fg(Color::DarkGray)),
            Span::styled("c", Style::default().fg(Color::White)),
            Span::styled(" Clear  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Esc", Style::default().fg(Color::White)),
            Span::styled(" Exit", Style::default().fg(Color::DarkGray)),
        ]),
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

fn render_ready_status<'a>(mode: &'a str, mode_style: Style) -> Line<'a> {
    Line::from(vec![
        Span::styled(mode, mode_style),
        Span::styled("  ←↑↓→ Navigate  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Enter", Style::default().fg(Color::White)),
        Span::styled(" Edit  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Tab", Style::default().fg(Color::White)),
        Span::styled(" Visual  ", Style::default().fg(Color::DarkGray)),
        Span::styled("s", Style::default().fg(Color::White)),
        Span::styled(" Save  ", Style::default().fg(Color::DarkGray)),
        Span::styled("q", Style::default().fg(Color::White)),
        Span::styled(" Quit", Style::default().fg(Color::DarkGray)),
    ])
}
