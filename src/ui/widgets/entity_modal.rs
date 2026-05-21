use ratatui::layout::{Constraint, Layout, Rect};
#[allow(unused_imports)]
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::ha::EntityId;

use crate::config::Alias;
use crate::ha::{ConnStatus, EntityState, InstanceRuntime};
use crate::screens::entities::EntityRow;
use crate::ui::theme::Theme;

#[allow(clippy::too_many_arguments)]
pub fn render_entity_search(
    f: &mut Frame,
    area: Rect,
    rows: &[EntityRow<'_>],
    selected: usize,
    query: &str,
    instance_filter: Option<&Alias>,
    multi_instance: bool,
    theme: &Theme,
) {
    let want_w = 120u16;
    let want_h = area.height.saturating_sub(4).max(12);
    let r = modal_rect(area, want_w, want_h);
    f.render_widget(Clear, r);

    let title = match instance_filter {
        Some(a) => format!(" Entities · filter:{a} ({}) ", rows.len()),
        None => format!(" Entities ({}) ", rows.len()),
    };
    let block = Block::bordered().title(title);
    let inner = block.inner(r);
    f.render_widget(block, r);

    let [search_row, hint_row, body] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .areas(inner);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("> ", Style::new().fg(Color::Yellow).bold()),
            Span::raw(query),
            Span::styled(
                "_",
                Style::new()
                    .fg(Color::Yellow)
                    .add_modifier(ratatui::style::Modifier::SLOW_BLINK),
            ),
        ])),
        search_row,
    );

    f.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            "↑/↓ navigate · type to filter · f instance · ⏎ default action · Esc close",
            Style::new().dim(),
        )])),
        hint_row,
    );

    // Split body into list (left) + detail (right) when wide enough.
    let split_detail = body.width >= 60;
    let (list_area, detail_area) = if split_detail {
        let [l, r] = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .areas(body);
        (l, Some(r))
    } else {
        (body, None)
    };

    let items: Vec<ListItem<'_>> = rows
        .iter()
        .map(|row| {
            let color = theme.instance_color(row.instance);
            let mut spans: Vec<Span<'_>> = Vec::new();
            if multi_instance {
                spans.push(Span::styled(
                    format!("[{}] ", row.instance),
                    Style::new().fg(color).bold(),
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

    let mut state = ListState::default();
    if !rows.is_empty() {
        state.select(Some(selected.min(rows.len().saturating_sub(1))));
    }
    let list = List::new(items)
        .highlight_style(Style::new().reversed())
        .highlight_symbol("▶ ");
    f.render_stateful_widget(list, list_area, &mut state);

    if let Some(detail) = detail_area {
        let sel_state = rows.get(selected).map(|r| r.state);
        render_detail(f, detail, sel_state);
    }
}

fn render_detail(f: &mut Frame, area: Rect, sel: Option<&EntityState>) {
    let block = Block::bordered().title(" Detail ");
    let inner = block.inner(area);
    f.render_widget(block, area);
    let text = match sel {
        None => "no selection".to_string(),
        Some(s) => {
            let attrs = serde_json::to_string_pretty(&s.attributes)
                .unwrap_or_else(|_| "<unserializable>".into());
            format!(
                "{}\nstate: {}\nlast_changed: {}\nlast_updated: {}\n\nattributes:\n{}",
                s.entity_id,
                s.state,
                s.last_changed.as_deref().unwrap_or("-"),
                s.last_updated.as_deref().unwrap_or("-"),
                attrs
            )
        }
    };
    f.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }), inner);
}

pub fn render_instance_list<'a, I>(
    f: &mut Frame,
    area: Rect,
    runtimes: I,
    selected: usize,
    theme: &Theme,
    count: usize,
) where
    I: Iterator<Item = &'a InstanceRuntime>,
{
    let h = (count as u16)
        .saturating_add(4)
        .clamp(6, area.height.saturating_sub(4));
    let r = modal_rect(area, 60, h);
    f.render_widget(Clear, r);

    let block = Block::bordered().title(format!(" Instances ({count}) "));
    let inner = block.inner(r);
    f.render_widget(block, r);

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
            ListItem::new(Line::from(vec![
                Span::styled(format!("{:<12} ", rt.alias), Style::new().fg(color).bold()),
                status,
                Span::raw(format!("   {} entities", rt.states.len())),
            ]))
        })
        .collect();

    let mut state = ListState::default();
    if count > 0 {
        state.select(Some(selected.min(count - 1)));
    }
    let list = List::new(items)
        .highlight_style(Style::new().reversed())
        .highlight_symbol("▶ ");
    f.render_stateful_widget(list, inner, &mut state);
}

pub fn render_entity_details(
    f: &mut Frame,
    area: Rect,
    entity_id: &EntityId,
    state: Option<&crate::ha::EntityState>,
    scroll: u16,
) {
    let w = (area.width as f32 * 0.85) as u16;
    let h = (area.height as f32 * 0.85) as u16;
    let r = modal_rect(area, w, h);
    f.render_widget(Clear, r);

    let title = state
        .and_then(|s| s.attributes.get("friendly_name"))
        .and_then(|v| v.as_str())
        .unwrap_or(entity_id.as_str());
    let block = Block::bordered().title(format!(" {title} "));
    let inner = block.inner(r);
    f.render_widget(block, r);

    let [body, footer] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(inner);

    let content = match state {
        None => "No state available.".to_string(),
        Some(s) => {
            let attrs = serde_json::to_string_pretty(&s.attributes)
                .unwrap_or_else(|_| "<unserializable>".into());
            format!(
                "entity_id:    {}\nstate:        {}\nlast_changed: {}\nlast_updated: {}\n\nattributes:\n{}",
                s.entity_id,
                s.state,
                s.last_changed.as_deref().unwrap_or("-"),
                s.last_updated.as_deref().unwrap_or("-"),
                attrs
            )
        }
    };

    f.render_widget(
        Paragraph::new(content)
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0)),
        body,
    );

    f.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            " j/k scroll · PgUp/PgDn page · Esc/q/D close",
            Style::new().dim(),
        )])),
        footer,
    );
}

fn modal_rect(parent: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(parent.width.saturating_sub(2));
    let h = h.min(parent.height.saturating_sub(2));
    let x = parent.x + parent.width.saturating_sub(w) / 2;
    let y = parent.y + parent.height.saturating_sub(h) / 2;
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
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
