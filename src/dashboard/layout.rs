use ratatui::layout::Rect;

use crate::dashboard::{Grid, Pos};

/// Map a grid cell rect to a screen rect within `area`.
pub fn cell_to_rect(area: Rect, grid: Grid, pos: Pos) -> Rect {
    if grid.cols == 0 || grid.rows == 0 {
        return Rect::default();
    }
    let cell_w = area.width as f32 / grid.cols as f32;
    let cell_h = area.height as f32 / grid.rows as f32;
    let x = area.x as f32 + cell_w * pos.col as f32;
    let y = area.y as f32 + cell_h * pos.row as f32;
    let w = cell_w * pos.w as f32;
    let h = cell_h * pos.h as f32;
    Rect {
        x: x.round() as u16,
        y: y.round() as u16,
        width: w.round().max(1.0) as u16,
        height: h.round().max(1.0) as u16,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_full_grid() {
        let area = Rect {
            x: 0,
            y: 0,
            width: 120,
            height: 80,
        };
        let grid = Grid { cols: 12, rows: 8 };
        let pos = Pos {
            col: 0,
            row: 0,
            w: 12,
            h: 8,
        };
        let r = cell_to_rect(area, grid, pos);
        assert_eq!(r.width, 120);
        assert_eq!(r.height, 80);
    }

    #[test]
    fn quarter_corner() {
        let area = Rect {
            x: 0,
            y: 0,
            width: 120,
            height: 80,
        };
        let grid = Grid { cols: 12, rows: 8 };
        let pos = Pos {
            col: 0,
            row: 0,
            w: 6,
            h: 4,
        };
        let r = cell_to_rect(area, grid, pos);
        assert_eq!(r.width, 60);
        assert_eq!(r.height, 40);
    }
}
