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
        let dot_style = match rt.status {
            ConnStatus::Connected => Style::new().fg(Color::Green),
            ConnStatus::Connecting | ConnStatus::Authenticating => Style::new().fg(Color::Yellow),
            ConnStatus::Failed => Style::new().fg(Color::Red),
            ConnStatus::Disconnected => Style::new().dim(),
        };
        spans.push(Span::styled("●", dot_style));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            rt.alias.clone(),
            Style::new().fg(color).bold(),
        ));
        spans.push(Span::raw("  "));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}
