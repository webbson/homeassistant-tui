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
    let body = match state {
        None => Line::from(Span::styled("(unknown)", Style::new().fg(Color::DarkGray))),
        Some(s) => {
            let unit = crate::ui::format::unit_of(s);
            let value = crate::ui::format::format_state(s, 1);
            Line::from(vec![
                Span::styled(value, Style::new().bold()),
                Span::raw(" "),
                Span::styled(unit.to_string(), Style::new().dim()),
            ])
        }
    };
    let p = Paragraph::new(body)
        .alignment(Alignment::Center)
        .block(block);
    f.render_widget(p, area);
}
