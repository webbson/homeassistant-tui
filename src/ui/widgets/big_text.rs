use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::Frame;
use tui_big_text::{BigText, PixelSize};

/// One quadrant glyph is 4×4 cells.
const GLYPH_W: u16 = 4;
const GLYPH_H: u16 = 4;

pub fn render_big(f: &mut Frame, area: Rect, text: &str, style: Style) {
    let big = BigText::builder()
        .pixel_size(PixelSize::Quadrant)
        .centered()
        .style(style)
        .lines(vec![text.into()])
        .build();
    // Vertically centre by shifting the render area down inside the parent.
    let y_offset = area.height.saturating_sub(GLYPH_H) / 2;
    let centered = Rect {
        x: area.x,
        y: area.y + y_offset,
        width: area.width,
        height: area.height.saturating_sub(y_offset),
    };
    f.render_widget(big, centered);
}

pub fn fits(area: Rect) -> bool {
    area.width >= 2 * GLYPH_W && area.height >= GLYPH_H
}
