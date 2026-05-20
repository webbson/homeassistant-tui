use ratatui::layout::Rect;
use ratatui::style::Color;
#[allow(unused_imports)]
use ratatui::style::{Style, Stylize};
use ratatui::widgets::{Block, Paragraph, Wrap};
use ratatui::Frame;

pub fn render(
    f: &mut Frame,
    area: Rect,
    title: &str,
    markdown: &str,
    card_color: Option<&str>,
    selected: bool,
) {
    let color = card_color
        .and_then(crate::ui::theme::parse_color)
        .unwrap_or(Color::White);
    let mut block = Block::bordered()
        .title(format!(" {title} "))
        .border_style(Style::new().fg(color));
    if selected {
        block = block.border_style(Style::new().fg(color).bold());
    }
    // Minimal "markdown": render as-is. Full markdown parsing can come later.
    f.render_widget(
        Paragraph::new(markdown.to_string())
            .wrap(Wrap { trim: false })
            .block(block),
        area,
    );
}
