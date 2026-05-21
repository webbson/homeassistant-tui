use ratatui::layout::Rect;
#[allow(unused_imports)]
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, ListState};
use ratatui::Frame;

use crate::ha::InstanceRuntime;
use crate::ui::theme::Theme;

#[allow(clippy::too_many_arguments)]
pub fn render(
    f: &mut Frame,
    area: Rect,
    title: &str,
    instance: &str,
    entity: &str,
    attribute: &str,
    template: &str,
    limit: Option<usize>,
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

    let state_obj = runtime.and_then(|rt| rt.states.get(entity));
    let arr = state_obj.and_then(|s| s.attributes.get(attribute).and_then(|v| v.as_array()));

    let items: Vec<ListItem<'_>> = match arr {
        None => {
            let msg = if state_obj.is_none() {
                "—".to_string()
            } else {
                format!("<no array at \"{attribute}\">")
            };
            let style = if state_obj.is_none() {
                Style::new().dim()
            } else {
                Style::new().red()
            };
            vec![ListItem::new(Line::from(Span::styled(msg, style)))]
        }
        Some(arr) => {
            let iter: Box<dyn Iterator<Item = (usize, &serde_json::Value)>> = match limit {
                Some(n) => Box::new(arr.iter().take(n).enumerate()),
                None => Box::new(arr.iter().enumerate()),
            };
            iter.map(|(idx, item)| {
                let rendered = if item.is_object() {
                    crate::ui::template::render_row(template, item, idx)
                } else {
                    item.to_string()
                };
                ListItem::new(Line::from(Span::styled(rendered, Style::new().fg(color))))
            })
            .collect()
        }
    };

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::new().reversed())
        .highlight_symbol("▶ ");

    let mut state = ListState::default();
    if let Some(i) = sub_index {
        let len = arr
            .map(|a| limit.map(|n| n.min(a.len())).unwrap_or(a.len()))
            .unwrap_or(0);
        if len > 0 {
            state.select(Some(i.min(len - 1)));
        }
    }
    f.render_stateful_widget(list, area, &mut state);
}
