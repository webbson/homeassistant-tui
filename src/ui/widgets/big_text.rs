use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::Frame;
use tui_big_text::{BigText, PixelSize};

pub fn render_big(f: &mut Frame, area: Rect, text: &str, color: Color) {
    let big = BigText::builder()
        .pixel_size(PixelSize::Quadrant)
        .style(Style::new().fg(color))
        .lines(vec![text.into()])
        .build();
    f.render_widget(big, area);
}

pub fn fits(area: Rect) -> bool {
    area.width >= 8 && area.height >= 4
}
