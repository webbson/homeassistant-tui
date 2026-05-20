use ratatui::layout::Rect;
#[allow(unused_imports)]
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::ha::{ConnStatus, InstanceRuntime};
use crate::ui::theme::Theme;

pub fn render<'a, I>(f: &mut Frame, area: Rect, runtimes: I, theme: &Theme)
where
    I: Iterator<Item = &'a InstanceRuntime>,
{
    let mut spans: Vec<Span<'a>> = vec![
        Span::raw(concat!("ha-tui:v", env!("CARGO_PKG_VERSION"))).bold(),
        Span::raw("  "),
    ];
    for rt in runtimes {
        let color = theme.instance_color(&rt.alias);
        let dot_color = match rt.status {
            ConnStatus::Connected => Color::Green,
            ConnStatus::Connecting | ConnStatus::Authenticating => Color::Yellow,
            ConnStatus::Failed => Color::Red,
            ConnStatus::Disconnected => Color::DarkGray,
        };
        spans.push(Span::styled("●", Style::new().fg(dot_color)));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            rt.alias.clone(),
            Style::new().fg(color).bold(),
        ));
        spans.push(Span::raw("  "));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}
