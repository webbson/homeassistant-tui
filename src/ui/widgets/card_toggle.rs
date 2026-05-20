use ratatui::layout::{Alignment, Rect};
#[allow(unused_imports)]
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::dashboard::CardSize;
use crate::ha::EntityState;
use crate::ui::theme::Theme;

#[allow(clippy::too_many_arguments)]
pub fn render(
    f: &mut Frame,
    area: Rect,
    title: &str,
    instance: &str,
    state: Option<&EntityState>,
    card_color: Option<&str>,
    theme: &Theme,
    selected: bool,
    size: CardSize,
) {
    let color = crate::ui::theme::resolve_card_color(card_color, instance, theme);
    let mut block = Block::bordered()
        .title(format!(" {title} "))
        .border_style(Style::new().fg(color));
    if selected {
        block = block.border_style(Style::new().fg(color).bold());
    }
    let (label, label_color) = match state.map(|s| s.state.as_str()) {
        Some("on" | "open" | "home") => ("ON", Color::Green),
        Some("off" | "closed" | "not_home") => ("OFF", Color::DarkGray),
        Some(other) => (other, Color::Yellow),
        None => ("—", Color::DarkGray),
    };

    // Inner area (inside the 1-char border on each side).
    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    if size == CardSize::Large && state.is_some() && super::big_text::fits(inner) {
        f.render_widget(block, area);
        super::big_text::render_big(f, inner, &format!("[{label}]"), label_color);
        return;
    }

    let line = Line::from(vec![
        Span::styled(format!("[{label}]"), Style::new().fg(label_color).bold()),
        Span::raw("  "),
        Span::styled("⏎ toggle", Style::new().dim()),
    ]);
    f.render_widget(
        Paragraph::new(line)
            .alignment(Alignment::Center)
            .block(block),
        area,
    );
}
