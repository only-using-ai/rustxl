use ratatui::layout::Rect;

/// Where a screen position falls on the grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitTarget {
    /// A data cell, in sheet coordinates.
    Cell { row: usize, col: usize },
    /// The lettered strip above the columns.
    ColumnHeader(usize),
    /// The numbered strip beside the rows.
    RowHeader(usize),
    /// The sheet name on the grid's top border.
    SheetIndicator,
    /// Anywhere else, including the borders and the space past the last column.
    Outside,
}

/// What the grid looked like the last time it was drawn.
///
/// Hit testing needs the same numbers the renderer used: which row and column
/// the viewport starts at, and how wide or tall each visible one is. Columns and
/// rows are sized individually, so neither can be derived from an offset and a
/// constant.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GridGeometry {
    /// The rectangle the grid block was drawn into, borders included.
    pub area: Rect,
    /// Width of the strip holding row numbers.
    pub row_header_width: u16,
    /// First visible column, and the width of each visible column in order.
    pub scroll_col: usize,
    pub col_widths: Vec<u16>,
    /// First visible row, and the height of each visible row in order.
    pub scroll_row: usize,
    pub row_heights: Vec<u16>,
    /// Width of the sheet name drawn on the top border.
    pub sheet_indicator_width: u16,
}

impl GridGeometry {
    /// The area inside the block's border.
    fn inner(&self) -> Option<Rect> {
        if self.area.width < 2 || self.area.height < 2 {
            return None;
        }
        Some(Rect {
            x: self.area.x + 1,
            y: self.area.y + 1,
            width: self.area.width - 2,
            height: self.area.height - 2,
        })
    }
}

/// Resolves a screen position to whatever the user was pointing at.
///
/// Pure: it reads the geometry captured at render time and nothing else, so the
/// arithmetic that decides which cell a click lands on can be tested without a
/// terminal.
pub fn hit_test(x: u16, y: u16, geometry: &GridGeometry) -> HitTarget {
    let area = geometry.area;

    // The sheet name sits on the top border line, starting after the corner.
    if geometry.sheet_indicator_width > 0
        && y == area.y
        && x > area.x
        && x <= area.x.saturating_add(geometry.sheet_indicator_width)
        && x < area.x.saturating_add(area.width)
    {
        return HitTarget::SheetIndicator;
    }

    let Some(inner) = geometry.inner() else {
        return HitTarget::Outside;
    };

    let inside = x >= inner.x
        && x < inner.x.saturating_add(inner.width)
        && y >= inner.y
        && y < inner.y.saturating_add(inner.height);
    if !inside {
        return HitTarget::Outside;
    }

    let column = column_at(x, inner.x, geometry);
    // The first line inside the border is the column letters.
    let on_header_row = y == inner.y;

    if x < inner.x.saturating_add(geometry.row_header_width) {
        return if on_header_row {
            // The corner where the two headers meet belongs to neither.
            HitTarget::Outside
        } else {
            match row_at(y, inner.y + 1, geometry) {
                Some(row) => HitTarget::RowHeader(row),
                None => HitTarget::Outside,
            }
        };
    }

    let Some(col) = column else {
        return HitTarget::Outside;
    };

    if on_header_row {
        return HitTarget::ColumnHeader(col);
    }

    match row_at(y, inner.y + 1, geometry) {
        Some(row) => HitTarget::Cell { row, col },
        None => HitTarget::Outside,
    }
}

/// Which column covers `x`, walking the widths the renderer used.
fn column_at(x: u16, inner_x: u16, geometry: &GridGeometry) -> Option<usize> {
    let mut edge = inner_x.checked_add(geometry.row_header_width)?;
    for (offset, width) in geometry.col_widths.iter().enumerate() {
        let next = edge.checked_add(*width)?;
        if x < next {
            return Some(geometry.scroll_col + offset);
        }
        edge = next;
    }
    None
}

/// Which row covers `y`, walking the heights the renderer used.
fn row_at(y: u16, first_row_y: u16, geometry: &GridGeometry) -> Option<usize> {
    let mut edge = first_row_y;
    for (offset, height) in geometry.row_heights.iter().enumerate() {
        let next = edge.checked_add(*height)?;
        if y < next {
            return Some(geometry.scroll_row + offset);
        }
        edge = next;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A grid at (0,0) 40x12, row numbers 5 wide, three 10-wide columns and
    /// eight single-height rows, scrolled to the origin.
    fn geometry() -> GridGeometry {
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
            sheet_indicator_width: 0,
        }
    }

    #[test]
    fn the_first_cell_starts_after_the_border_and_the_row_numbers() {
        let g = geometry();
        // x: 0 border, 1..=5 row numbers, 6 first column. y: 0 border, 1 header,
        // 2 first data row.
        assert_eq!(hit_test(6, 2, &g), HitTarget::Cell { row: 0, col: 0 });
        assert_eq!(hit_test(5, 2, &g), HitTarget::RowHeader(0));
    }

    #[test]
    fn columns_are_found_by_walking_their_own_widths() {
        let mut g = geometry();
        g.col_widths = vec![4, 20, 6];

        assert_eq!(hit_test(6, 2, &g), HitTarget::Cell { row: 0, col: 0 });
        assert_eq!(hit_test(9, 2, &g), HitTarget::Cell { row: 0, col: 0 });
        assert_eq!(hit_test(10, 2, &g), HitTarget::Cell { row: 0, col: 1 });
        assert_eq!(hit_test(29, 2, &g), HitTarget::Cell { row: 0, col: 1 });
        assert_eq!(hit_test(30, 2, &g), HitTarget::Cell { row: 0, col: 2 });
    }

    #[test]
    fn rows_are_found_by_walking_their_own_heights() {
        let mut g = geometry();
        g.row_heights = vec![1, 3, 1];

        assert_eq!(hit_test(6, 2, &g), HitTarget::Cell { row: 0, col: 0 });
        assert_eq!(hit_test(6, 3, &g), HitTarget::Cell { row: 1, col: 0 });
        assert_eq!(hit_test(6, 5, &g), HitTarget::Cell { row: 1, col: 0 });
        assert_eq!(hit_test(6, 6, &g), HitTarget::Cell { row: 2, col: 0 });
    }

    #[test]
    fn scrolling_shifts_which_sheet_coordinates_a_position_names() {
        let mut g = geometry();
        g.scroll_col = 7;
        g.scroll_row = 100;

        assert_eq!(hit_test(6, 2, &g), HitTarget::Cell { row: 100, col: 7 });
        assert_eq!(hit_test(16, 3, &g), HitTarget::Cell { row: 101, col: 8 });
        assert_eq!(hit_test(5, 4, &g), HitTarget::RowHeader(102));
        assert_eq!(hit_test(16, 1, &g), HitTarget::ColumnHeader(8));
    }

    #[test]
    fn the_strip_above_the_columns_is_the_column_header() {
        let g = geometry();

        assert_eq!(hit_test(6, 1, &g), HitTarget::ColumnHeader(0));
        assert_eq!(hit_test(16, 1, &g), HitTarget::ColumnHeader(1));
    }

    #[test]
    fn the_corner_between_the_two_headers_belongs_to_neither() {
        let g = geometry();

        assert_eq!(hit_test(2, 1, &g), HitTarget::Outside);
    }

    #[test]
    fn borders_and_the_space_past_the_last_column_are_outside() {
        let g = geometry();

        assert_eq!(hit_test(0, 5, &g), HitTarget::Outside, "left border");
        assert_eq!(hit_test(39, 5, &g), HitTarget::Outside, "right border");
        assert_eq!(hit_test(6, 11, &g), HitTarget::Outside, "bottom border");
        // Three 10-wide columns end at x = 35; the rest of the row is empty.
        assert_eq!(hit_test(36, 2, &g), HitTarget::Outside);
    }

    #[test]
    fn positions_beyond_the_last_visible_row_are_outside() {
        let mut g = geometry();
        g.row_heights = vec![1, 1];

        assert_eq!(hit_test(6, 3, &g), HitTarget::Cell { row: 1, col: 0 });
        assert_eq!(hit_test(6, 4, &g), HitTarget::Outside);
    }

    #[test]
    fn the_sheet_name_on_the_top_border_is_its_own_target() {
        let mut g = geometry();
        g.sheet_indicator_width = 13;

        assert_eq!(hit_test(1, 0, &g), HitTarget::SheetIndicator);
        assert_eq!(hit_test(13, 0, &g), HitTarget::SheetIndicator);
        assert_eq!(hit_test(14, 0, &g), HitTarget::Outside, "past the name");
        assert_eq!(hit_test(0, 0, &g), HitTarget::Outside, "the corner");
    }

    #[test]
    fn a_grid_too_small_to_have_an_inside_swallows_every_position() {
        let g = GridGeometry {
            area: Rect {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            },
            ..geometry()
        };

        assert_eq!(hit_test(0, 0, &g), HitTarget::Outside);
    }

    #[test]
    fn a_grid_drawn_away_from_the_origin_is_offset_too() {
        let mut g = geometry();
        g.area = Rect {
            x: 10,
            y: 4,
            width: 40,
            height: 12,
        };

        assert_eq!(hit_test(16, 6, &g), HitTarget::Cell { row: 0, col: 0 });
        assert_eq!(hit_test(15, 6, &g), HitTarget::RowHeader(0));
        assert_eq!(hit_test(16, 5, &g), HitTarget::ColumnHeader(0));
        assert_eq!(hit_test(9, 6, &g), HitTarget::Outside);
    }
}
