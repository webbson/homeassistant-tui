use std::collections::HashMap;

use ratatui::layout::Rect;
#[allow(unused_imports)]
use ratatui::style::{Color, Style, Stylize};
use ratatui::widgets::{Block, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;

use crate::app::App;
use crate::dashboard::layout::{cell_to_rect, grid_layout};
use crate::dashboard::{CardKind, DashboardLayout, GraphType};
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
    // Clone the dashboard so we can mutably borrow `app` inside `render_card`.
    let Some(dash) = app.dashboards.get(idx).cloned() else {
        f.render_widget(
            Paragraph::new("no dashboard").block(Block::bordered()),
            area,
        );
        return;
    };

    match &dash.layout {
        DashboardLayout::Free { grid, .. } => {
            let grid = *grid;
            for (i, card) in dash.cards_iter().enumerate() {
                let Some(pos) = card.pos else {
                    continue;
                };
                let rect = cell_to_rect(area, grid, pos);
                if rect.width < 3 || rect.height < 3 {
                    continue;
                }
                let sel = i == selected_card;
                let sub = if sel { sub_index } else { None };
                render_card(f, rect, card, app, sel, sub, in_editor);
            }
        }
        DashboardLayout::Grid { rows } => {
            // Build col_scrolls from app state for this dashboard.
            let col_scrolls: HashMap<(usize, usize), u16> = app
                .column_scroll
                .iter()
                .filter(|((di, _, _), _)| *di == idx)
                .map(|((_, ri, ci), &offset)| ((*ri, *ci), offset))
                .collect();

            // Compute per-card preferred heights using column widths from the layout.
            let card_heights: Vec<u16> = {
                let mut widths: Vec<u16> = Vec::new();
                for row in rows.iter() {
                    let n = row.columns.len() as u16;
                    let base = if n > 0 { area.width / n } else { area.width };
                    let rem = if n > 0 { area.width % n } else { 0 };
                    for (ci, col) in row.columns.iter().enumerate() {
                        let w = base + if ci == row.columns.len() - 1 { rem } else { 0 };
                        for _ in &col.cards {
                            widths.push(w);
                        }
                    }
                }
                dash.cards_iter()
                    .zip(widths.iter())
                    .map(|(c, &w)| {
                        let dynamic_count = match &c.kind {
                            CardKind::FilteredEntityList {
                                instance, query, ..
                            } => {
                                let rt = app.instances.runtimes.get(instance);
                                Some(crate::dashboard::query::resolve(rt, query).len())
                            }
                            CardKind::AttributeList {
                                instance,
                                entity,
                                attribute,
                                limit,
                                ..
                            } => {
                                let rt = app.instances.runtimes.get(instance);
                                rt.and_then(|rt| rt.states.get(entity.as_str()))
                                    .and_then(|s| s.attributes.get(attribute.as_str()))
                                    .and_then(|v| v.as_array())
                                    .map(|a| limit.map_or(a.len(), |lim| a.len().min(lim)))
                            }
                            _ => None,
                        };
                        c.preferred_height(w, dynamic_count)
                    })
                    .collect()
            };

            let card_fills: Vec<bool> = dash
                .cards_iter()
                .map(|c| matches!(c.kind, CardKind::Image { .. } | CardKind::Graph { .. }))
                .collect();

            let (slots, col_infos) =
                grid_layout(rows, area, &col_scrolls, &card_heights, &card_fills);

            // Re-clamp scroll offsets and render scrollbars.
            for info in &col_infos {
                let max_scroll = info.content_height.saturating_sub(info.rect.height);
                let current = app
                    .column_scroll
                    .get(&(idx, info.row_idx, info.col_idx))
                    .copied()
                    .unwrap_or(0);
                if current > max_scroll {
                    app.column_scroll
                        .insert((idx, info.row_idx, info.col_idx), max_scroll);
                }

                if info.needs_scrollbar && info.content_height > 0 {
                    let scroll_pos = col_scrolls
                        .get(&(info.row_idx, info.col_idx))
                        .copied()
                        .unwrap_or(0);
                    let mut state = ScrollbarState::new(info.content_height as usize)
                        .position(scroll_pos as usize);
                    f.render_stateful_widget(
                        Scrollbar::new(ScrollbarOrientation::VerticalRight),
                        info.rect,
                        &mut state,
                    );
                }
            }

            // Render visible card slots.
            for slot in &slots {
                let Some(card) = dash.cards_iter().nth(slot.flat_idx) else {
                    continue;
                };
                if slot.rect.width < 3 || slot.rect.height < 2 {
                    continue;
                }
                let sel = slot.flat_idx == selected_card;
                let sub = if sel { sub_index } else { None };
                render_card(f, slot.rect, card, app, sel, sub, in_editor);
            }
        }
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
            let empty_attrs = serde_json::Map::new();
            let entity_state = app
                .instances
                .runtimes
                .get(instance)
                .and_then(|rt| rt.states.get(entity));
            let value = entity_state.and_then(|s| s.state.parse::<f64>().ok());
            let attrs = entity_state
                .and_then(|s| s.attributes.as_object())
                .unwrap_or(&empty_attrs);
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
                attrs,
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
            let current_states: Vec<Option<&crate::ha::EntityState>> = entities
                .iter()
                .map(|s| {
                    app.instances
                        .runtimes
                        .get(instance)
                        .and_then(|rt| rt.states.get(&s.entity))
                })
                .collect();
            let graph_args = GraphRender {
                area: rect,
                title: &title,
                instance,
                series: entities,
                histories: &histories,
                current_states: &current_states,
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
            let rows: Vec<widgets::card_entity_list::EntityListRow<'_>> = entities
                .iter()
                .map(|item| widgets::card_entity_list::EntityListRow {
                    entity_id: item.entity_id(),
                    name_override: item.name_override(),
                    hide_state: item.hide_state_override().unwrap_or(false),
                })
                .collect();
            widgets::card_entity_list::render(
                f,
                rect,
                &title,
                instance,
                &rows,
                rt,
                card.color.as_deref(),
                &app.theme,
                selected,
                sub_index,
            );
        }
        CardKind::FilteredEntityList {
            instance,
            query,
            hide_state,
            hide_when_empty,
            overrides,
            ..
        } => {
            let rt = app.instances.runtimes.get(instance);
            let entities = crate::dashboard::query::resolve(rt, query);
            if *hide_when_empty && entities.is_empty() {
                if in_editor {
                    // Draw a dimmed placeholder so the card stays selectable in the editor.
                    let block = Block::bordered()
                        .title(title.as_str())
                        .style(Style::new().dim());
                    let inner = block.inner(rect);
                    f.render_widget(block, rect);
                    f.render_widget(
                        Paragraph::new("(hidden — no matches)").style(Style::new().dim()),
                        inner,
                    );
                }
                // On the normal dashboard, skip rendering entirely (grid cells still occupied).
                return;
            }
            let rows: Vec<widgets::card_entity_list::EntityListRow<'_>> = entities
                .iter()
                .map(|eid| {
                    let ov = overrides.get(eid);
                    widgets::card_entity_list::EntityListRow {
                        entity_id: eid,
                        name_override: ov.and_then(|o| o.name.as_deref()),
                        hide_state: ov.and_then(|o| o.hide_state).unwrap_or(*hide_state),
                    }
                })
                .collect();
            widgets::card_entity_list::render(
                f,
                rect,
                &title,
                instance,
                &rows,
                rt,
                card.color.as_deref(),
                &app.theme,
                selected,
                sub_index,
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
        CardKind::AttributeList {
            instance,
            entity,
            attribute,
            template,
            limit,
            ..
        } => {
            let rt = app.instances.runtimes.get(instance);
            widgets::card_attribute_list::render(
                f,
                rect,
                &title,
                instance,
                entity,
                attribute,
                template,
                *limit,
                rt,
                card.color.as_deref(),
                &app.theme,
                selected,
                sub_index,
            );
        }
    }
}
