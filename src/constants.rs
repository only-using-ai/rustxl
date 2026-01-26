use ratatui::style::Color;

pub const DEFAULT_ROWS: usize = 100;
pub const DEFAULT_COLS: usize = 26;
pub const DEFAULT_COL_WIDTH: u16 = 10;
pub const DEFAULT_ROW_HEIGHT: u16 = 1;
pub const MIN_COL_WIDTH: u16 = 4;
pub const MAX_COL_WIDTH: u16 = 40;
pub const MIN_ROW_HEIGHT: u16 = 1;
pub const MAX_ROW_HEIGHT: u16 = 10;

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

// Light mode color scheme (Excel-like)
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
pub const CELL_BG: Color = Color::White;
pub const CELL_FG: Color = Color::Rgb(0, 0, 0);

// Dark mode color scheme
pub const DARK_HEADER_BG: Color = Color::Rgb(50, 50, 50);
pub const DARK_HEADER_FG: Color = Color::Rgb(220, 220, 220);
pub const DARK_SELECTED_BG: Color = Color::Rgb(60, 80, 120);
pub const DARK_SELECTED_HEADER_BG: Color = Color::Rgb(70, 100, 150);
pub const DARK_FORMULA_BAR_BG: Color = Color::Rgb(40, 40, 40);
pub const DARK_GRID_COLOR: Color = Color::Rgb(80, 80, 80);
pub const DARK_CELL_NAME_BG: Color = Color::Rgb(60, 60, 60);
pub const DARK_FORMULA_BG: Color = Color::Rgb(30, 30, 30);
pub const DARK_REF_SELECTION_BG: Color = Color::Rgb(60, 100, 60);
pub const DARK_REF_RANGE_BG: Color = Color::Rgb(50, 70, 90);
pub const DARK_CELL_BG: Color = Color::Rgb(25, 25, 25);
pub const DARK_CELL_FG: Color = Color::Rgb(220, 220, 220);

// Find mode highlight colors
pub const FIND_MATCH_BG: Color = Color::Rgb(255, 255, 180);  // Light yellow
pub const DARK_FIND_MATCH_BG: Color = Color::Rgb(120, 120, 60);  // Darker yellow for dark mode
