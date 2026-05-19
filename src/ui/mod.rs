pub mod theme;
pub mod widgets;

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::Style;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;
use crate::screens::entities;
use crate::screens::Screen;

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(area);

    widgets::instance_bar::render(f, header, app.instances.runtimes.values(), &app.theme);
    draw_body(f, body, app);
    draw_footer(f, footer, app);
    if app.show_help {
        widgets::help::render(f, area);
    }
}

fn draw_body(f: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    match &app.screen {
        Screen::Entities {
            instance_filter,
            search,
            selected,
        } => {
            let multi = app.instances.runtimes.len() > 1;
            let rows = entities::build_rows(
                app.instances.runtimes.values(),
                instance_filter.as_ref(),
                search,
            );
            let cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(area);
            let filter_tag = instance_filter
                .as_ref()
                .map(|f| format!(" filter:{f}"))
                .unwrap_or_default();
            let title = format!("Entities ({}){filter_tag}", rows.len());
            widgets::entity_list::render(f, cols[0], &rows, *selected, multi, &title, &app.theme);
            let sel_state = rows.get(*selected).map(|r| r.state);
            widgets::entity_detail::render(f, cols[1], sel_state);
        }
        Screen::Instances { selected } => {
            widgets::instances_screen::render(
                f,
                area,
                app.instances.runtimes.values(),
                *selected,
                &app.theme,
            );
        }
        Screen::Dashboard { idx, selected_card } => {
            crate::screens::dashboard::draw(f, area, app, *idx, *selected_card);
        }
        Screen::Editor => {
            crate::screens::editor::draw(f, area, app);
        }
    }
}

fn draw_footer(f: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let info = if let Some(err) = &app.last_error {
        format!("error: {err}")
    } else {
        match &app.screen {
            Screen::Entities { .. } => format!(
                "{} entities · q quit · j/k navigate · f filter · n new dashboard · i instances",
                app.instances.total_entities()
            ),
            Screen::Instances { .. } => {
                "q quit · j/k navigate · n new dashboard · E entities".to_string()
            }
            Screen::Dashboard { idx, .. } => {
                let name = app.dashboards.get(*idx).map(|d| d.name.as_str()).unwrap_or("?");
                format!("dashboard: {name} · 1..9 switch · h/l select card · ⏎ activate · e edit · E entities · q quit")
            }
            Screen::Editor => {
                "editor · hjkl move · HJKL resize · a add · d del · ⏎ select/place · u undo · s save · Esc exit".to_string()
            }
        }
    };
    f.render_widget(Paragraph::new(info).style(Style::new().dim()), area);
}
