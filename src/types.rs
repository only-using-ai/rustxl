use ratatui::style::Color;

#[derive(Clone, Copy, Default)]
pub struct CellStyle {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum VisualSubMode {
    Main,
    TextColor,
    BackgroundColor,
    ColumnWidth,
    RowHeight,
}

#[derive(Clone, Copy, PartialEq)]
pub enum SaveFormat {
    Csv,
    Tsv,
}
