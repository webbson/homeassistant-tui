use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem};
use ratatui::Frame;

use crate::ha::InstanceRuntime;
use crate::ui::theme::Theme;

pub fn render(
    f: &mut Frame,
    area: Rect,
    title: &str,
    instance: &str,
    entities: &[String],
    runtime: Option<&InstanceRuntime>,
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
    let items: Vec<ListItem<'_>> = entities
        .iter()
        .map(|eid| {
            let state_obj = runtime.and_then(|rt| rt.states.get(eid));
            let (label, state_str) = match state_obj {
                Some(s) => {
                    let friendly = s
                        .attributes
                        .get("friendly_name")
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                        .unwrap_or_else(|| eid.clone());
                    let unit = crate::ui::format::unit_of(s);
                    let val = crate::ui::format::format_state(s, 1);
                    let formatted = if unit.is_empty() {
                        val
                    } else {
                        format!("{val} {unit}")
                    };
                    (friendly, formatted)
                }
                None => (eid.clone(), "—".into()),
            };
            let state_style = match state_obj.map(|s| s.state.as_str()) {
                Some("on" | "open" | "home") => Style::new().green(),
                Some("off" | "closed" | "not_home") => Style::new().dim(),
                Some("unavailable" | "unknown") => Style::new().red(),
                _ => Style::new(),
            };
            ListItem::new(Line::from(vec![
                Span::styled(label, Style::new().fg(color).add_modifier(Modifier::BOLD)),
                Span::raw("  "),
                Span::styled(state_str, state_style),
            ]))
        })
        .collect();
    f.render_widget(List::new(items).block(block), area);
}
