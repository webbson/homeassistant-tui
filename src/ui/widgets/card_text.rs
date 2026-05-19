use ratatui::layout::Rect;
use ratatui::style::{Style, Stylize};
use ratatui::widgets::{Block, Paragraph, Wrap};
use ratatui::Frame;

pub fn render(f: &mut Frame, area: Rect, title: &str, markdown: &str, selected: bool) {
    let mut block = Block::bordered().title(format!(" {title} "));
    if selected {
        block = block.border_style(Style::new().bold());
    }
    // Minimal "markdown": render as-is. Full markdown parsing can come later.
    f.render_widget(
        Paragraph::new(markdown.to_string())
            .wrap(Wrap { trim: false })
            .block(block),
        area,
    );
}
