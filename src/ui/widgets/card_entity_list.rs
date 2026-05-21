use ratatui::layout::Rect;
#[allow(unused_imports)]
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, ListState};
use ratatui::Frame;

use crate::ha::InstanceRuntime;
use crate::ui::theme::Theme;

/// One row in a rendered entity list — pre-resolved from either an
/// `EntityListItem` or a `FilteredEntityList` resolved entity + overrides.
pub struct EntityListRow<'a> {
    pub entity_id: &'a str,
    pub name_override: Option<&'a str>,
    pub hide_state: bool,
}

#[allow(clippy::too_many_arguments)]
pub fn render(
    f: &mut Frame,
    area: Rect,
    title: &str,
    instance: &str,
    rows: &[EntityListRow<'_>],
    runtime: Option<&InstanceRuntime>,
    card_color: Option<&str>,
    theme: &Theme,
    selected: bool,
    sub_index: Option<usize>,
) {
    let color = crate::ui::theme::resolve_card_color(card_color, instance, theme);
    let mut block = Block::bordered()
        .title(format!(" {title} "))
        .border_style(Style::new().fg(color));
    if selected {
        block = block.border_style(Style::new().fg(color).bold());
    }
    let items: Vec<ListItem<'_>> = rows
        .iter()
        .map(|row| {
            let eid = row.entity_id;
            let state_obj = runtime.and_then(|rt| rt.states.get(eid));
            let (label, state_str) = match state_obj {
                Some(s) => {
                    let friendly = row.name_override.map(str::to_string).unwrap_or_else(|| {
                        s.attributes
                            .get("friendly_name")
                            .and_then(|v| v.as_str())
                            .map(str::to_string)
                            .unwrap_or_else(|| eid.to_string())
                    });
                    let unit = crate::ui::format::unit_of(s);
                    let val = crate::ui::format::format_state(s, 1);
                    let formatted = if unit.is_empty() {
                        val
                    } else {
                        format!("{val} {unit}")
                    };
                    (friendly, formatted)
                }
                None => (
                    row.name_override
                        .map(str::to_string)
                        .unwrap_or_else(|| eid.to_string()),
                    "—".into(),
                ),
            };
            let state_style = match state_obj.map(|s| s.state.as_str()) {
                Some("on" | "open" | "home") => Style::new().green(),
                Some("off" | "closed" | "not_home") => Style::new().dim(),
                Some("unavailable" | "unknown") => Style::new().red(),
                _ => Style::new(),
            };
            let mut spans = vec![Span::styled(
                label,
                Style::new().fg(color).add_modifier(Modifier::BOLD),
            )];
            if !row.hide_state {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(state_str, state_style));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();
    let list = List::new(items)
        .block(block)
        .highlight_style(Style::new().reversed())
        .highlight_symbol("▶ ");
    let mut state = ListState::default();
    if let Some(i) = sub_index {
        if !rows.is_empty() {
            state.select(Some(i.min(rows.len() - 1)));
        }
    }
    f.render_stateful_widget(list, area, &mut state);
}
