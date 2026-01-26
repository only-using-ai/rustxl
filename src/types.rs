use ratatui::style::Color;

#[derive(Clone, Copy, PartialEq, Default)]
pub enum TextAlignment {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, PartialEq, Default)]
pub enum VerticalAlignment {
    #[default]
    Top,
    Center,
    Bottom,
}

#[derive(Clone, Copy, Default)]
pub struct CellStyle {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
    pub alignment: Option<TextAlignment>,
    pub vertical_alignment: Option<VerticalAlignment>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum VisualSubMode {
    Main,
    TextColor,
    BackgroundColor,
    ColumnWidth,
    RowHeight,
    TextAlignment,
    VerticalAlignment,
}

#[derive(Clone, Copy, PartialEq)]
pub enum SaveFormat {
    Csv,
    Tsv,
}

#[derive(Clone, Copy, PartialEq)]
pub enum RowColumnSelectMode {
    None,
    RowSelect,
    ColumnSelect,
}
