use ratatui::layout::Rect;
#[allow(unused_imports)]
use ratatui::style::{Color, Style, Stylize};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::dashboard::layout::cell_to_rect;
use crate::dashboard::{CardKind, GraphType, Grid};
use crate::ui::widgets;
use crate::ui::widgets::card_graph::GraphRender;
use crate::util::history::RingBuf;

pub fn draw(
    f: &mut Frame,
    area: Rect,
    app: &mut App,
    idx: usize,
    selected_card: usize,
    sub_index: Option<usize>,
) {
    let in_editor = app.editor.is_some();
    // Clone the dashboard so we can mutably borrow `app` inside `render_card` (needed for
    // stateful image rendering via `&mut StatefulProtocol`).
    let Some(dash) = app.dashboards.get(idx).cloned() else {
        f.render_widget(
            Paragraph::new("no dashboard").block(Block::bordered()),
            area,
        );
        return;
    };
    for (i, card) in dash.cards_iter().enumerate() {
        let Some(pos) = card.pos else { continue; };
        let rect = cell_to_rect(area, dash.free_grid().unwrap_or(Grid { cols: 12, rows: 8 }), pos);
        if rect.width < 3 || rect.height < 3 {
            continue;
        }
        let sel = i == selected_card;
        let sub = if sel { sub_index } else { None };
        render_card(f, rect, card, app, sel, sub, in_editor);
    }
}

fn render_card(
    f: &mut Frame,
    rect: Rect,
    card: &crate::dashboard::Card,
    app: &mut App,
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
        CardKind::Clock {
            format, timezone, ..
        } => {
            widgets::card_clock::render(
                f,
                rect,
                &title,
                format,
                timezone.as_deref(),
                card.color.as_deref(),
                card.size,
                selected,
            );
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
        CardKind::Statistics {
            instance,
            entity,
            window,
            metric,
            unit,
            ..
        } => {
            let history = app.history.get(&(instance.clone(), entity.clone()));
            widgets::card_statistics::render(
                f,
                rect,
                &title,
                instance,
                history,
                *metric,
                window,
                unit.as_deref(),
                card.color.as_deref(),
                card.size,
                &app.theme,
                selected,
            );
        }
        CardKind::MediaPlayer {
            instance, entity, ..
        } => {
            let s = app
                .instances
                .runtimes
                .get(instance)
                .and_then(|rt| rt.states.get(entity));
            widgets::card_media_player::render(
                f,
                rect,
                &title,
                instance,
                s,
                card.color.as_deref(),
                card.size,
                &app.theme,
                selected,
            );
        }
        CardKind::Image {
            instance, source, ..
        } => {
            let entity = match source {
                crate::dashboard::ImageSource::ImageEntity { entity } => entity.clone(),
                crate::dashboard::ImageSource::Camera { entity } => entity.clone(),
            };
            let key = (instance.clone(), entity.clone());
            // On first render of a freshly-added card, the cache has no entry
            // yet — kick off a fetch. send_image_fetch is a no-op when already
            // in-flight, so the 250ms tick won't spam requests.
            if !app.image_cache.contains_key(&key) {
                let inst = instance.clone();
                let ent = entity.clone();
                app.send_image_fetch(&inst, &ent);
            }
            let error = app
                .image_cache
                .get(&key)
                .and_then(|e| e.error.as_deref())
                .map(str::to_string);
            let protocol = app.image_cache.get_mut(&key).map(|e| &mut e.protocol);
            widgets::card_image::render(
                f,
                rect,
                &title,
                instance,
                protocol,
                error.as_deref(),
                card.color.as_deref(),
                &app.theme,
                selected,
            );
        }
        CardKind::Weather {
            instance,
            entity,
            show_forecast,
            forecast_days,
            ..
        } => {
            let state = app
                .instances
                .runtimes
                .get(instance)
                .and_then(|rt| rt.states.get(entity));
            let forecast = app
                .weather_forecasts
                .get(&(instance.clone(), entity.clone()));
            widgets::card_weather::render(
                f,
                rect,
                &title,
                instance,
                state,
                forecast,
                *show_forecast,
                *forecast_days,
                card.color.as_deref(),
                card.size,
                &app.theme,
                selected,
            );
        }
    }
}
