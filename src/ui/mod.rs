pub mod format;
pub mod template;
pub mod theme;
pub mod widgets;

use ratatui::layout::{Alignment, Constraint, Layout};
#[allow(unused_imports)]
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;
use crate::screens::{Overlay, Screen};

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(area);

    draw_header(f, header, app);
    draw_body(f, body, app);
    draw_overlay(f, area, app);
    draw_footer(f, footer, app);
    if app.show_help {
        widgets::help::render(f, area);
    }
}

fn draw_header(f: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let [left, center, right] =
        Layout::horizontal([Constraint::Min(20), Constraint::Fill(1), Constraint::Min(8)])
            .areas(area);

    widgets::instance_bar::render(f, left, app.instances.runtimes.values(), &app.theme);

    let screen_idx = if let Screen::Dashboard { idx, .. } = &app.screen {
        Some(*idx)
    } else {
        None
    };
    let active_idx = app.editor.as_ref().map(|e| e.dash_idx).or(screen_idx);
    let dash_name = active_idx.and_then(|i| app.dashboards.get(i)).map(|d| {
        let dirty = app.editor.as_ref().is_some_and(|e| e.dirty);
        let mark = if dirty { "*" } else { "" };
        format!("◆ {}{}", d.name, mark)
    });
    if let Some(name) = dash_name {
        f.render_widget(
            Paragraph::new(name)
                .style(Style::new().bold().dim())
                .alignment(Alignment::Center),
            center,
        );
    }

    let mut right_spans: Vec<Span<'_>> = Vec::new();
    if let Some(editor) = app.editor.as_ref() {
        right_spans.push(Span::styled(
            format!("✎ {},{}", editor.cursor_col, editor.cursor_row),
            Style::new().fg(Color::Yellow).bold(),
        ));
    }
    if let Some(ver) = app.update_available.as_deref() {
        if !right_spans.is_empty() {
            right_spans.push(Span::raw("  "));
        }
        right_spans.push(Span::styled(
            format!("↑ v{ver}"),
            Style::new().fg(Color::Yellow).bold(),
        ));
    }
    if !right_spans.is_empty() {
        f.render_widget(
            Paragraph::new(Line::from(right_spans)).alignment(Alignment::Right),
            right,
        );
    }
}

fn draw_body(f: &mut Frame, area: ratatui::layout::Rect, app: &mut App) {
    match app.screen.clone() {
        Screen::Dashboard {
            idx,
            selected_card,
            sub_index,
        } => {
            crate::screens::dashboard::draw(f, area, app, idx, selected_card, Some(sub_index));
        }
        Screen::Editor => {
            crate::screens::editor::draw(f, area, app);
        }
    }
}

fn draw_overlay(f: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let Some(overlay) = app.overlay.as_ref() else {
        return;
    };
    match overlay {
        Overlay::EntitySearch {
            query,
            selected,
            instance_filter,
        } => {
            let rows = crate::screens::entities::build_rows(
                app.instances.runtimes.values(),
                instance_filter.as_ref(),
                query,
            );
            let multi = app.instances.runtimes.len() > 1;
            widgets::entity_modal::render_entity_search(
                f,
                area,
                &rows,
                *selected,
                query,
                instance_filter.as_ref(),
                multi,
                &app.theme,
            );
        }
        Overlay::InstanceList { selected } => {
            widgets::entity_modal::render_instance_list(
                f,
                area,
                app.instances.runtimes.values(),
                *selected,
                &app.theme,
                app.instances.runtimes.len(),
            );
        }
        Overlay::InputValue(ref modal_state) => {
            widgets::input_modal::render(f, area, modal_state);
        }
        Overlay::EntityDetails {
            alias,
            entity_id,
            scroll,
        } => {
            let state = app
                .instances
                .runtimes
                .get(alias)
                .and_then(|rt| rt.states.get(entity_id));
            widgets::entity_modal::render_entity_details(f, area, entity_id, state, *scroll);
        }
    }
}

fn draw_footer(f: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let (text, style) = if let Some(err) = &app.last_error {
        (format!("error: {err}"), Style::new().fg(Color::Red))
    } else if let Some(msg) = &app.status_msg {
        (msg.clone(), Style::new().fg(Color::Green))
    } else {
        let s = match &app.screen {
            Screen::Dashboard { .. } => {
                "1..9 switch · h/l cards · j/k rows · ⏎ activate · D details · e edit · E entities · i instances · ? help"
                    .to_string()
            }
            Screen::Editor => {
                "editor · hjkl move · HJKL resize · a add · m menu · ⏎ select/place · u undo · s save · Esc exit"
                    .to_string()
            }
        };
        (s, Style::new().dim())
    };
    f.render_widget(Paragraph::new(text).style(style), area);
}
