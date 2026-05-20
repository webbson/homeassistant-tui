use ratatui::layout::Rect;

use crate::dashboard::{Grid, GridRow, Pos, RowHeight};

/// Minimum terminal rows allocated to any row to ensure a border + 1 content line.
const MIN_ROW_HEIGHT: u16 = 3;

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

/// One rendered card slot in a grid layout.
pub struct GridCardSlot {
    /// Flat card index into `Dashboard::cards_iter()`.
    pub flat_idx: usize,
    /// The screen rect to render into (already clipped to the column viewport).
    pub rect: Rect,
    /// How many terminal rows the card would need without scrolling.
    #[allow(dead_code)]
    pub natural_height: u16,
}

/// Metadata about a rendered column in a grid layout.
pub struct ColInfo {
    pub row_idx: usize,
    pub col_idx: usize,
    pub rect: Rect,
    pub needs_scrollbar: bool,
    /// Sum of natural card heights in this column (for scrollbar sizing and offset clamping).
    pub content_height: u16,
}

/// Compute card rects for a grid-layout dashboard.
///
/// `rows`         — the grid layout rows.
/// `area`         — the full available screen area.
/// `col_scrolls`  — per-column scroll offsets: `col_scrolls[(row_idx, col_idx)]` = rows scrolled.
/// `card_heights` — per-flat-index preferred height (pre-computed by the caller).
///
/// Returns a flat list of `GridCardSlot` for each card that is at least partially visible.
/// Also returns per-column metadata so the caller can draw scrollbars and clamp offsets.
pub fn grid_layout(
    rows: &[GridRow],
    area: Rect,
    col_scrolls: &std::collections::HashMap<(usize, usize), u16>,
    card_heights: &[u16],
) -> (Vec<GridCardSlot>, Vec<ColInfo>) {
    let mut slots: Vec<GridCardSlot> = Vec::new();
    let mut col_infos: Vec<ColInfo> = Vec::new();

    if rows.is_empty() || area.height == 0 || area.width == 0 {
        return (slots, col_infos);
    }

    // ── Step 1: compute row heights ──────────────────────────────────────
    let n_auto = rows.iter().filter(|r| r.height == RowHeight::Auto).count() as u16;
    let sum_fixed: u16 = rows
        .iter()
        .map(|r| match r.height {
            RowHeight::Fixed(h) => h,
            RowHeight::Auto => 0,
        })
        .sum();

    // If fixed rows alone overflow, scale everything proportionally.
    let scale = if sum_fixed >= area.height && sum_fixed > 0 {
        area.height as f32 / sum_fixed as f32
    } else {
        1.0_f32
    };

    let remaining = area.height.saturating_sub((sum_fixed as f32 * scale) as u16);
    let auto_share = if n_auto > 0 {
        (remaining / n_auto).max(MIN_ROW_HEIGHT)
    } else {
        0
    };

    let row_heights: Vec<u16> = rows
        .iter()
        .map(|r| match r.height {
            RowHeight::Fixed(h) => ((h as f32 * scale) as u16).max(MIN_ROW_HEIGHT),
            RowHeight::Auto => auto_share,
        })
        .collect();

    // ── Step 2: lay out rows and columns ─────────────────────────────────
    let mut flat_idx: usize = 0; // global card index across all rows/cols
    let mut row_y = area.y;

    for (ri, row) in rows.iter().enumerate() {
        let row_h = row_heights[ri];
        let n_cols = row.columns.len() as u16;
        if n_cols == 0 {
            // Count the cards in all columns of this row as flat slots; skip rendering.
            for col in &row.columns {
                flat_idx += col.cards.len();
            }
            row_y += row_h;
            continue;
        }

        let col_w_base = area.width / n_cols;
        let col_w_remainder = area.width % n_cols;

        let mut col_x = area.x;
        for (ci, col) in row.columns.iter().enumerate() {
            let col_w = col_w_base + if ci == row.columns.len() - 1 { col_w_remainder } else { 0 };
            let col_rect = Rect { x: col_x, y: row_y, width: col_w, height: row_h };

            let fill = col.effective_fill_height(row.fill_height_default());
            let scroll = col_scrolls.get(&(ri, ci)).copied().unwrap_or(0);

            // Gather this column's card heights.
            let col_card_heights: Vec<u16> = col
                .cards
                .iter()
                .enumerate()
                .map(|(ci_local, _)| {
                    card_heights
                        .get(flat_idx + ci_local)
                        .copied()
                        .unwrap_or(4)
                })
                .collect();

            let sum_natural: u16 = col_card_heights.iter().sum();

            // Compute final rendered heights.
            let rendered_heights: Vec<u16> = if fill && sum_natural > 0 {
                // Proportional fill: scale up to col_rect.height.
                let target = col_rect.height as f32;
                let mut heights: Vec<u16> = col_card_heights
                    .iter()
                    .map(|&h| {
                        let frac = h as f32 / sum_natural as f32;
                        (frac * target).floor() as u16
                    })
                    .collect();
                // Correct rounding drift on the last card.
                let sum: u16 = heights.iter().sum();
                if let Some(last) = heights.last_mut() {
                    *last = last.saturating_add(col_rect.height.saturating_sub(sum));
                }
                heights
            } else {
                col_card_heights.clone()
            };

            let needs_scrollbar = !fill && sum_natural > col_rect.height;
            col_infos.push(ColInfo {
                row_idx: ri,
                col_idx: ci,
                rect: col_rect,
                needs_scrollbar,
                content_height: sum_natural,
            });

            // Position cards within the column, applying scroll offset.
            let mut card_y = col_rect.y as i32 - scroll as i32;
            for (card_local_idx, card_h) in rendered_heights.iter().enumerate() {
                let card_h = *card_h;
                let card_rect = Rect {
                    x: col_rect.x,
                    y: card_y.max(col_rect.y as i32) as u16,
                    width: col_rect.width,
                    height: card_h,
                };
                // Only include cards that are at least partially visible.
                let bottom = card_y + card_h as i32;
                let col_bottom = col_rect.y as i32 + col_rect.height as i32;
                if bottom > col_rect.y as i32 && card_y < col_bottom {
                    // Clip to column bounds.
                    let visible_y = card_rect.y.max(col_rect.y);
                    let visible_bottom = (card_y + card_h as i32).min(col_bottom) as u16;
                    let visible_h = visible_bottom.saturating_sub(visible_y);
                    if visible_h >= 2 {
                        slots.push(GridCardSlot {
                            flat_idx: flat_idx + card_local_idx,
                            rect: Rect {
                                x: col_rect.x,
                                y: visible_y,
                                width: col_rect.width,
                                height: visible_h,
                            },
                            natural_height: card_h,
                        });
                    }
                }
                card_y += card_h as i32;
            }

            flat_idx += col.cards.len();
            col_x += col_w;
        }
        row_y += row_h;
    }

    (slots, col_infos)
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

    fn make_grid_rows(specs: &[(RowHeight, usize, bool)]) -> Vec<crate::dashboard::GridRow> {
        use crate::dashboard::{GridColumn, GridRow};
        specs
            .iter()
            .map(|(height, n_cols, fill)| GridRow {
                height: *height,
                fill_height: Some(*fill),
                columns: (0..*n_cols)
                    .map(|_| GridColumn { fill_height: None, cards: vec![] })
                    .collect(),
            })
            .collect()
    }

    #[test]
    fn grid_layout_fixed_and_auto_rows() {
        let area = Rect { x: 0, y: 0, width: 120, height: 30 };
        // Two fixed rows (10 each) + one auto row → auto gets remaining 10.
        let rows = make_grid_rows(&[
            (RowHeight::Fixed(10), 2, false),
            (RowHeight::Fixed(10), 2, false),
            (RowHeight::Auto, 2, false),
        ]);
        let (slots, col_infos) = grid_layout(&rows, area, &Default::default(), &[]);
        assert!(slots.is_empty(), "no cards → no slots");
        assert_eq!(col_infos.len(), 6, "3 rows × 2 cols");
        // Third row starts at y=20 and has height=10.
        let auto_cols: Vec<_> = col_infos.iter().filter(|c| c.row_idx == 2).collect();
        assert_eq!(auto_cols.len(), 2);
        assert_eq!(auto_cols[0].rect.y, 20);
        assert_eq!(auto_cols[0].rect.height, 10);
    }

    #[test]
    fn grid_layout_fill_height_proportional() {
        use crate::dashboard::{Card, CardId, CardKind, CardSize, GridColumn, GridRow};
        let area = Rect { x: 0, y: 0, width: 60, height: 20 };
        // One row, one column, fill_height=true, two cards with heights 4 and 6.
        let card = |h: u16| Card {
            id: CardId::ZERO,
            kind: CardKind::Text { markdown: String::new(), title: None },
            pos: None,
            height: Some(h),
            color: None,
            size: CardSize::Normal,
        };
        let rows = vec![GridRow {
            height: RowHeight::Fixed(20),
            fill_height: Some(true),
            columns: vec![GridColumn {
                fill_height: None,
                cards: vec![card(4), card(6)],
            }],
        }];
        let card_heights = [4u16, 6u16];
        let (slots, col_infos) = grid_layout(&rows, area, &Default::default(), &card_heights);
        assert_eq!(slots.len(), 2);
        assert_eq!(col_infos.len(), 1);
        assert!(!col_infos[0].needs_scrollbar, "fill_height → no scrollbar");
        // Heights must sum to exactly the column height.
        let total: u16 = slots.iter().map(|s| s.rect.height).sum();
        assert_eq!(total, 20, "fill slots must sum to column height");
    }

    #[test]
    fn grid_layout_scroll_hides_top_card() {
        use crate::dashboard::{Card, CardId, CardKind, CardSize, GridColumn, GridRow};
        let area = Rect { x: 0, y: 0, width: 60, height: 10 };
        let card = || Card {
            id: CardId::ZERO,
            kind: CardKind::Text { markdown: String::new(), title: None },
            pos: None,
            height: None,
            color: None,
            size: CardSize::Normal,
        };
        let rows = vec![GridRow {
            height: RowHeight::Fixed(10),
            fill_height: Some(false),
            columns: vec![GridColumn {
                fill_height: None,
                cards: vec![card(), card()],
            }],
        }];
        // Each card is 8 tall; scroll by 8 → first card fully hidden.
        let card_heights = [8u16, 8u16];
        let mut scrolls = std::collections::HashMap::new();
        scrolls.insert((0usize, 0usize), 8u16);
        let (slots, col_infos) = grid_layout(&rows, area, &scrolls, &card_heights);
        assert_eq!(col_infos[0].needs_scrollbar, true);
        // First card (flat_idx 0) should not appear in slots.
        assert!(!slots.iter().any(|s| s.flat_idx == 0), "scrolled-away card must not render");
        assert!(slots.iter().any(|s| s.flat_idx == 1), "second card must be visible");
    }
}
