use ratatui::style::Color;

pub const DEFAULT_ROWS: usize = 100;
pub const DEFAULT_COLS: usize = 26;
pub const DEFAULT_COL_WIDTH: u16 = 10;
pub const DEFAULT_ROW_HEIGHT: u16 = 1;
pub const MIN_COL_WIDTH: u16 = 4;
pub const MAX_COL_WIDTH: u16 = 40;
pub const MIN_ROW_HEIGHT: u16 = 1;
pub const MAX_ROW_HEIGHT: u16 = 5;

// Color palette for visual mode
pub const COLOR_PALETTE: [(Color, &str); 10] = [
    (Color::White, "White"),
    (Color::Black, "Black"),
    (Color::Red, "Red"),
    (Color::Green, "Green"),
    (Color::Blue, "Blue"),
    (Color::Yellow, "Yellow"),
    (Color::Magenta, "Magenta"),
    (Color::Cyan, "Cyan"),
    (Color::Rgb(255, 165, 0), "Orange"),
    (Color::Rgb(128, 128, 128), "Gray"),
];

// Excel-like color scheme
pub const HEADER_BG: Color = Color::Rgb(217, 217, 217);
pub const HEADER_FG: Color = Color::Rgb(0, 0, 0);
pub const SELECTED_BG: Color = Color::Rgb(180, 198, 231);
pub const SELECTED_HEADER_BG: Color = Color::Rgb(142, 169, 219);
pub const FORMULA_BAR_BG: Color = Color::Rgb(240, 240, 240);
pub const GRID_COLOR: Color = Color::Rgb(200, 200, 200);
pub const CELL_NAME_BG: Color = Color::Rgb(200, 200, 200);
pub const FORMULA_BG: Color = Color::White;
pub const REF_SELECTION_BG: Color = Color::Rgb(198, 224, 180);
pub const REF_RANGE_BG: Color = Color::Rgb(221, 235, 247);
