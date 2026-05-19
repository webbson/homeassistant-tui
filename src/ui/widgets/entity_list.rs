use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, ListState};
use ratatui::Frame;

use crate::screens::entities::EntityRow;
use crate::ui::theme::Theme;

pub fn render(
    f: &mut Frame,
    area: Rect,
    rows: &[EntityRow<'_>],
    selected: usize,
    multi_instance: bool,
    title: &str,
    theme: &Theme,
) {
    let items: Vec<ListItem<'_>> = rows
        .iter()
        .map(|row| {
            let color = theme.instance_color(row.instance);
            let mut spans: Vec<Span<'_>> = Vec::new();
            if multi_instance {
                spans.push(Span::styled(
                    format!("[{}] ", row.instance),
                    Style::new().fg(color).add_modifier(Modifier::BOLD),
                ));
            }
            spans.push(Span::styled(
                row.state.entity_id.clone(),
                Style::new().fg(color),
            ));
            spans.push(Span::raw("  "));
            spans.push(state_span(&row.state.state));
            ListItem::new(Line::from(spans))
        })
        .collect();

    let list = List::new(items)
        .block(Block::bordered().title(format!(" {title} ")))
        .highlight_style(Style::new().reversed())
        .highlight_symbol("▶ ");

    let mut state = ListState::default();
    if !rows.is_empty() {
        state.select(Some(selected.min(rows.len().saturating_sub(1))));
    }
    f.render_stateful_widget(list, area, &mut state);
}

fn state_span(state: &str) -> Span<'_> {
    let style = match state {
        "on" | "open" | "home" => Style::new().green(),
        "off" | "closed" | "not_home" => Style::new().dim(),
        "unavailable" | "unknown" => Style::new().red(),
        _ => Style::new(),
    };
    Span::styled(state.to_string(), style)
}
