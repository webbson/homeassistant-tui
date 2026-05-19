use ratatui::layout::Rect;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, ListState};
use ratatui::Frame;

use crate::ha::{ConnStatus, InstanceRuntime};
use crate::ui::theme::Theme;

pub fn render<'a, I>(f: &mut Frame, area: Rect, runtimes: I, selected: usize, theme: &Theme)
where
    I: Iterator<Item = &'a InstanceRuntime>,
{
    let items: Vec<ListItem<'_>> = runtimes
        .map(|rt| {
            let color = theme.instance_color(&rt.alias);
            let status = match rt.status {
                ConnStatus::Connected => Span::styled("connected", Style::new().green()),
                ConnStatus::Connecting => Span::styled("connecting", Style::new().yellow()),
                ConnStatus::Authenticating => Span::styled("authenticating", Style::new().yellow()),
                ConnStatus::Failed => Span::styled("failed", Style::new().red()),
                ConnStatus::Disconnected => Span::styled("disconnected", Style::new().dim()),
            };
            let line = Line::from(vec![
                Span::styled(format!("{:<12} ", rt.alias), Style::new().fg(color).bold()),
                status,
                Span::raw(format!("   {} entities   ", rt.states.len())),
                Span::styled(
                    rt.last_error.clone().unwrap_or_default(),
                    Style::new()
                        .fg(Color::Red)
                        .add_modifier(ratatui::style::Modifier::DIM),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(Some(selected.min(items.len() - 1)));
    }
    let list = List::new(items)
        .block(Block::bordered().title(" Instances "))
        .highlight_style(Style::new().reversed())
        .highlight_symbol("▶ ");
    f.render_stateful_widget(list, area, &mut state);
}
