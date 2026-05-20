use ratatui::layout::Rect;
#[allow(unused_imports)]
use ratatui::style::{Color, Style, Stylize};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::dashboard::layout::cell_to_rect;
use crate::dashboard::{CardKind, Dashboard, GraphType};
use crate::ui::widgets;
use crate::ui::widgets::card_graph::GraphRender;
use crate::util::history::RingBuf;

pub fn draw(
    f: &mut Frame,
    area: Rect,
    app: &App,
    idx: usize,
    selected_card: usize,
    sub_index: Option<usize>,
) {
    let in_editor = app.editor.is_some();
    let Some(dash) = app.dashboards.get(idx) else {
        f.render_widget(
            Paragraph::new("no dashboard").block(Block::bordered()),
            area,
        );
        return;
    };
    draw_title(f, area, dash);
    let inner = Rect {
        x: area.x,
        y: area.y + 1,
        width: area.width,
        height: area.height.saturating_sub(1),
    };
    for (i, card) in dash.cards.iter().enumerate() {
        let rect = cell_to_rect(inner, dash.grid, card.pos);
        if rect.width < 3 || rect.height < 3 {
            continue;
        }
        let sel = i == selected_card;
        let sub = if sel { sub_index } else { None };
        render_card(f, rect, card, app, sel, sub, in_editor);
    }
}

fn draw_title(f: &mut Frame, area: Rect, dash: &Dashboard) {
    let bar = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: 1,
    };
    f.render_widget(
        Paragraph::new(format!("◆ {}", dash.name)).style(Style::new().bold().dim()),
        bar,
    );
}

fn render_card(
    f: &mut Frame,
    rect: Rect,
    card: &crate::dashboard::Card,
    app: &App,
    selected: bool,
    sub_index: Option<usize>,
    in_editor: bool,
) {
    let title = card.title().to_string();
    match &card.kind {
        CardKind::Entity {
            instance,
            entity,
            ticker,
            ..
        } => {
            let s = app
                .instances
                .runtimes
                .get(instance)
                .and_then(|rt| rt.states.get(entity));
            widgets::card_entity::render(
                f,
                rect,
                &title,
                instance,
                s,
                card.color.as_deref(),
                &app.theme,
                selected,
                *ticker,
                app.ticker_offset,
                card.size,
            );
        }
        CardKind::Toggle {
            instance, entity, ..
        } => {
            let s = app
                .instances
                .runtimes
                .get(instance)
                .and_then(|rt| rt.states.get(entity));
            widgets::card_toggle::render(
                f,
                rect,
                &title,
                instance,
                s,
                card.color.as_deref(),
                &app.theme,
                selected,
                card.size,
            );
        }
        CardKind::Gauge {
            instance,
            entity,
            min,
            max,
            unit,
            severity,
            needle,
            ..
        } => {
            let value = app
                .instances
                .runtimes
                .get(instance)
                .and_then(|rt| rt.states.get(entity))
                .and_then(|s| s.state.parse::<f64>().ok());
            widgets::card_gauge::render(
                f,
                rect,
                &title,
                instance,
                value,
                *min,
                *max,
                unit.as_deref(),
                severity.as_ref(),
                *needle,
                card.color.as_deref(),
                card.size,
                &app.theme,
                selected,
            );
        }
        CardKind::Graph {
            instance,
            entities,
            graph_type,
            window,
            orientation,
            ..
        } => {
            let histories: Vec<(crate::ha::EntityId, Option<&RingBuf>)> = entities
                .iter()
                .map(|s| {
                    (
                        s.entity.clone(),
                        app.history.get(&(instance.clone(), s.entity.clone())),
                    )
                })
                .collect();
            let graph_args = GraphRender {
                area: rect,
                title: &title,
                instance,
                series: entities,
                histories: &histories,
                window,
                card_color: card.color.as_deref(),
                theme: &app.theme,
                selected,
            };
            match graph_type {
                GraphType::Line => {
                    widgets::card_graph::render_line(f, graph_args);
                }
                GraphType::Bar => {
                    let current: Vec<(crate::ha::EntityId, Option<f64>)> = entities
                        .iter()
                        .map(|s| {
                            let val = app
                                .instances
                                .runtimes
                                .get(instance)
                                .and_then(|rt| rt.states.get(&s.entity))
                                .and_then(|st| st.state.parse::<f64>().ok());
                            (s.entity.clone(), val)
                        })
                        .collect();
                    widgets::card_graph::render_bar(f, graph_args, *orientation, &current);
                }
                GraphType::Pie => {
                    let current: Vec<(crate::ha::EntityId, Option<f64>)> = entities
                        .iter()
                        .map(|s| {
                            let val = app
                                .instances
                                .runtimes
                                .get(instance)
                                .and_then(|rt| rt.states.get(&s.entity))
                                .and_then(|st| st.state.parse::<f64>().ok());
                            (s.entity.clone(), val)
                        })
                        .collect();
                    widgets::card_graph::render_pie(f, graph_args, &current);
                }
            }
        }
        CardKind::Text { markdown, .. } => {
            widgets::card_text::render(f, rect, &title, markdown, card.color.as_deref(), selected);
        }
        CardKind::EntityList {
            instance, entities, ..
        } => {
            let rt = app.instances.runtimes.get(instance);
            widgets::card_entity_list::render(
                f,
                rect,
                &title,
                instance,
                entities,
                rt,
                card.color.as_deref(),
                &app.theme,
                selected,
                sub_index,
                false,
            );
        }
        CardKind::FilteredEntityList {
            instance,
            query,
            hide_state,
            hide_when_empty,
            ..
        } => {
            let rt = app.instances.runtimes.get(instance);
            let entities = crate::dashboard::query::resolve(rt, query);
            if *hide_when_empty && entities.is_empty() {
                if in_editor {
                    // Draw a dimmed placeholder so the card stays selectable in the editor.
                    let block = Block::bordered()
                        .title(title.as_str())
                        .style(Style::new().fg(Color::DarkGray));
                    let inner = block.inner(rect);
                    f.render_widget(block, rect);
                    f.render_widget(
                        Paragraph::new("(hidden — no matches)")
                            .style(Style::new().fg(Color::DarkGray)),
                        inner,
                    );
                }
                // On the normal dashboard, skip rendering entirely (grid cells still occupied).
                return;
            }
            widgets::card_entity_list::render(
                f,
                rect,
                &title,
                instance,
                &entities,
                rt,
                card.color.as_deref(),
                &app.theme,
                selected,
                sub_index,
                *hide_state,
            );
        }
    }
}
