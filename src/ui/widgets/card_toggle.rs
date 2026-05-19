use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::ha::EntityState;
use crate::ui::theme::Theme;

pub fn render(
    f: &mut Frame,
    area: Rect,
    title: &str,
    instance: &str,
    state: Option<&EntityState>,
    theme: &Theme,
    selected: bool,
) {
    let color = theme.instance_color(instance);
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
