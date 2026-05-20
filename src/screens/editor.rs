use ratatui::layout::Rect;
#[allow(unused_imports)]
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::dashboard::editor::SeverityAccum;
use crate::dashboard::editor::{CardTypeStub, EditorMode, SeriesIndexOp};
use crate::dashboard::layout::cell_to_rect;
use crate::dashboard::{BarOrientation, CardSize};

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let Some(editor) = app.editor.as_ref() else {
        return;
    };
    let Some(dash) = app.dashboards.get(editor.dash_idx) else {
        return;
    };

    let bar_rect = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: 1,
    };
    let dirty = if editor.dirty { "*" } else { "" };
    let title = Line::from(vec![
        Span::styled(
            format!("◆ editing: {}{}", dash.name, dirty),
            Style::new().bold(),
        ),
        Span::raw("    "),
        Span::styled(
            format!("cursor: {},{}", editor.cursor_col, editor.cursor_row),
            Style::new().dim(),
        ),
    ]);
    f.render_widget(Paragraph::new(title), bar_rect);

    let body_rect = Rect {
        x: area.x,
        y: area.y + 1,
        width: area.width,
        height: area.height.saturating_sub(1),
    };

    crate::screens::dashboard::draw(f, body_rect, app, editor.dash_idx, usize::MAX, None);

    let cur_rect = cell_to_rect(
        body_rect,
        dash.grid,
        crate::dashboard::Pos {
            col: editor.cursor_col,
            row: editor.cursor_row,
            w: 1,
            h: 1,
        },
    );
    if cur_rect.width > 0 && cur_rect.height > 0 {
        f.render_widget(
            Block::new().style(Style::new().bg(Color::DarkGray)),
            cur_rect,
        );
    }

    if let Some(i) = editor.selected_card {
        if let Some(card) = dash.cards.get(i) {
            let r = cell_to_rect(body_rect, dash.grid, card.pos);
            f.render_widget(
                Block::bordered().border_style(Style::new().fg(Color::Yellow).bold()),
                r,
            );
        }
    }

    match &editor.mode {
        EditorMode::PickingType => draw_palette(f, area),
        EditorMode::PickingInstance { selected, .. } => {
            draw_instance_picker(f, area, app, *selected)
        }
        EditorMode::PickingEntity {
            card_type,
            instance,
            query,
            selected,
        } => draw_entity_picker(f, area, app, *card_type, instance, query, *selected),
        EditorMode::EditingTitle {
            card_type,
            instance,
            entity,
            friendly_name,
            title_buffer,
        } => draw_title_input(
            f,
            area,
            *card_type,
            instance,
            entity,
            friendly_name,
            title_buffer,
        ),
        EditorMode::EditingTextBody {
            title_buffer,
            body_buffer,
            focus_body,
        } => draw_text_body(f, area, title_buffer, body_buffer, *focus_body),
        EditorMode::PickingMulti {
            instance,
            query,
            selected,
            picked,
        } => draw_multi_picker(f, area, app, instance, query, *selected, picked),
        EditorMode::EditingEntityListTitle {
            instance,
            picked,
            title_buffer,
        } => draw_entity_list_title(f, area, instance, picked, title_buffer),
        EditorMode::ConfirmExit => draw_confirm(f, area, "Unsaved changes. Discard? (y/n)"),
        EditorMode::ConfirmDelete => draw_confirm(f, area, "Delete selected card? (y/n)"),
        EditorMode::Renaming { buffer } => draw_rename(f, area, buffer),
        EditorMode::RenamingCard { buffer, .. } => draw_card_rename(f, area, buffer),
        EditorMode::EditingWindow { buffer, .. } => draw_window_edit(f, area, buffer),
        EditorMode::EditingFilterQuery {
            instance,
            query_buffer,
            title_buffer,
            hide_state,
            focus,
        } => draw_filter_query(
            f,
            area,
            instance,
            query_buffer,
            title_buffer,
            *hide_state,
            *focus,
        ),
        EditorMode::Menu {
            context,
            items,
            selected,
        } => draw_menu(f, area, *context, items, *selected),
        EditorMode::ResizingGrid {
            cols_buffer,
            rows_buffer,
            focus_rows,
        } => draw_resize_grid(f, area, cols_buffer, rows_buffer, *focus_rows),
        EditorMode::EnterColorOverride { buf, .. } => draw_color_override(f, area, buf),
        EditorMode::PickCardSize { current, .. } => draw_pick_size(f, area, *current),
        // Graph add-flow
        EditorMode::GraphPickType => draw_graph_pick_type(f, area),
        EditorMode::GraphPickInstance { selected, .. } => {
            draw_instance_picker(f, area, app, *selected)
        }
        EditorMode::GraphAddEntities {
            instance,
            accumulated,
            query,
            selected,
            asking_more,
            ..
        } => draw_graph_add_entities(
            f,
            area,
            app,
            instance,
            accumulated,
            query,
            *selected,
            *asking_more,
        ),
        EditorMode::GraphEditWindowAdd {
            window_buf,
            title_buf,
            title_stage,
            ..
        } => draw_graph_window_add(f, area, window_buf, title_buf, *title_stage),
        EditorMode::GraphPickOrientationAdd {
            current,
            title_buf,
            title_stage,
            ..
        } => draw_graph_orientation_add(f, area, *current, title_buf, *title_stage),
        // Graph context-menu flows
        EditorMode::GraphAddOneSeries {
            card_idx,
            query,
            selected,
        } => draw_graph_add_one_series(f, area, app, *card_idx, query, *selected),
        EditorMode::GraphPickSeriesIndex {
            card_idx,
            op,
            selected,
        } => draw_graph_pick_series(f, area, app, editor, dash, *card_idx, *op, *selected),
        EditorMode::GraphEditSeriesColor { buf, .. } => draw_graph_series_color(f, area, buf),
        EditorMode::GraphEditSeriesLabel { buf, .. } => draw_graph_series_label(f, area, buf),
        EditorMode::GraphEditWindow { buf, .. } => draw_window_edit(f, area, buf),
        EditorMode::GraphPickOrientation { current, .. } => {
            draw_graph_pick_orientation(f, area, *current)
        }
        // Gauge severity flow
        EditorMode::EditSeverityGreen { buf, accum, .. } => {
            draw_numeric_prompt(
                f,
                area,
                " Severity thresholds (1/3) ",
                "Green threshold (lower bound)",
                buf,
                accum,
            );
        }
        EditorMode::EditSeverityYellow { buf, accum, .. } => {
            draw_numeric_prompt(
                f,
                area,
                " Severity thresholds (2/3) ",
                "Yellow threshold (warning)",
                buf,
                accum,
            );
        }
        EditorMode::EditSeverityRed { buf, accum, .. } => {
            draw_numeric_prompt(
                f,
                area,
                " Severity thresholds (3/3) ",
                "Red threshold (critical)",
                buf,
                accum,
            );
        }
        // Clock add-flow
        EditorMode::ClockAddTitle { title_buffer } => {
            draw_text_input(
                f,
                area,
                " New clock card (1/3) ",
                "Title (optional — Enter to skip)",
                title_buffer,
            );
        }
        EditorMode::ClockAddFormat { format_buffer, .. } => {
            draw_text_input(
                f,
                area,
                " New clock card (2/3) ",
                "Format string (e.g. %H:%M:%S)",
                format_buffer,
            );
        }
        EditorMode::ClockAddTimezone { tz_buffer, .. } => {
            draw_text_input(
                f,
                area,
                " New clock card (3/3) ",
                "Timezone (e.g. Europe/London — Enter to use local)",
                tz_buffer,
            );
        }
        // Clock context-menu flows
        EditorMode::ClockEditFormat { buf, .. } => {
            draw_text_input(
                f,
                area,
                " Edit clock format ",
                "Format string (e.g. %H:%M:%S)",
                buf,
            );
        }
        EditorMode::ClockEditTimezone { buf, .. } => {
            draw_text_input(
                f,
                area,
                " Edit clock timezone ",
                "Timezone (e.g. Europe/London — Enter to clear)",
                buf,
            );
        }
        // Statistics add-flow
        EditorMode::StatsPickMetric { selected, .. } => {
            draw_stats_pick_metric(f, area, *selected);
        }
        EditorMode::StatsEditWindowAdd { buf, .. } => {
            draw_text_input(
                f,
                area,
                " New statistics card (4/6) ",
                "Window (e.g. 1h, 24h, 7d — Enter for 1h)",
                buf,
            );
        }
        EditorMode::StatsEditUnitAdd { buf, .. } => {
            draw_text_input(
                f,
                area,
                " New statistics card (5/6) ",
                "Unit (e.g. °C, % — Enter to skip)",
                buf,
            );
        }
        EditorMode::StatsEditTitleAdd { buf, .. } => {
            draw_text_input(
                f,
                area,
                " New statistics card (6/6) ",
                "Title (optional — Enter to use entity name)",
                buf,
            );
        }
        // Statistics context-menu flows
        EditorMode::StatsEditMetric { selected, .. } => {
            draw_stats_pick_metric(f, area, *selected);
        }
        EditorMode::StatsEditWindow { buf, .. } => {
            draw_text_input(
                f,
                area,
                " Edit statistics window ",
                "Window (e.g. 1h, 24h, 7d — Enter for 1h)",
                buf,
            );
        }
        EditorMode::StatsEditUnit { buf, .. } => {
            draw_text_input(
                f,
                area,
                " Edit statistics unit ",
                "Unit (e.g. °C, % — Enter to clear)",
                buf,
            );
        }
        EditorMode::Browse => {}
    }
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

fn draw_palette(f: &mut Frame, area: Rect) {
    let r = modal_rect(area, 40, 10);
    f.render_widget(Clear, r);
    let lines: Vec<Line<'_>> = CardTypeStub::ALL
        .iter()
        .enumerate()
        .map(|(i, k)| {
            Line::from(vec![
                Span::styled(format!(" {} ", i + 1), Style::new().reversed()),
                Span::raw("  "),
                Span::raw(k.label()),
            ])
        })
        .collect();
    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(" Add card — pick type ")),
        r,
    );
}

fn draw_instance_picker(f: &mut Frame, area: Rect, app: &App, selected: usize) {
    let r = modal_rect(area, 40, 12);
    f.render_widget(Clear, r);
    let items: Vec<ListItem<'_>> = app
        .instances
        .runtimes
        .keys()
        .enumerate()
        .map(|(i, alias)| {
            let color = app.theme.instance_color(alias);
            ListItem::new(Line::from(vec![
                Span::styled(format!(" {} ", i + 1), Style::new().reversed()),
                Span::raw("  "),
                Span::styled(alias.clone(), Style::new().fg(color).bold()),
            ]))
        })
        .collect();
    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(Some(selected.min(items.len() - 1)));
    }
    f.render_stateful_widget(
        List::new(items)
            .block(Block::bordered().title(" Pick instance "))
            .highlight_style(Style::new().reversed())
            .highlight_symbol("▶ "),
        r,
        &mut state,
    );
}

fn draw_entity_picker(
    f: &mut Frame,
    area: Rect,
    app: &App,
    card_type: CardTypeStub,
    instance: &str,
    query: &str,
    selected: usize,
) {
    let w = 80u16.min(area.width.saturating_sub(2));
    let h = 24u16.min(area.height.saturating_sub(2));
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let r = Rect {
        x,
        y,
        width: w,
        height: h,
    };
    f.render_widget(Clear, r);

    let inner = Rect {
        x: r.x + 1,
        y: r.y + 1,
        width: r.width.saturating_sub(2),
        height: r.height.saturating_sub(2),
    };
    let search_row = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    };
    let list_row = Rect {
        x: inner.x,
        y: inner.y + 2,
        width: inner.width,
        height: inner.height.saturating_sub(3),
    };

    let color = app.theme.instance_color(instance);
    let title = format!(
        " {} → {}  (type to search, ↑/↓ select, ⏎ pick, Esc cancel) ",
        card_type.label(),
        instance,
    );
    f.render_widget(Block::bordered().title(title), r);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("search: ", Style::new().dim()),
            Span::styled(query.to_string(), Style::new().fg(color).bold()),
            Span::styled("_", Style::new().fg(color).rapid_blink()),
        ])),
        search_row,
    );

    let domain_prefix = crate::app::domain_prefix_for_type(card_type);
    let rows = crate::app::entity_search_filtered(&app.instances, instance, query, domain_prefix);
    let items: Vec<ListItem<'_>> = rows
        .iter()
        .map(|p| {
            let primary = if p.friendly_name.is_empty() {
                p.entity_id.clone()
            } else {
                p.friendly_name.clone()
            };
            ListItem::new(Line::from(vec![
                Span::styled(primary, Style::new().fg(color).bold()),
                Span::raw("  "),
                Span::styled(p.entity_id.clone(), Style::new().dim()),
            ]))
        })
        .collect();
    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(Some(selected.min(items.len() - 1)));
    }
    f.render_stateful_widget(
        List::new(items)
            .highlight_style(Style::new().reversed())
            .highlight_symbol("▶ "),
        list_row,
        &mut state,
    );
}

fn draw_title_input(
    f: &mut Frame,
    area: Rect,
    card_type: CardTypeStub,
    instance: &str,
    entity: &str,
    friendly: &str,
    title_buffer: &str,
) {
    let r = modal_rect(area, 68, 8);
    f.render_widget(Clear, r);
    let default_label = if friendly.is_empty() {
        entity
    } else {
        friendly
    };
    let lines = vec![
        Line::from(vec![
            Span::styled("type:    ", Style::new().dim()),
            Span::raw(card_type.label().to_string()),
        ]),
        Line::from(vec![
            Span::styled("instance:", Style::new().dim()),
            Span::raw(" "),
            Span::raw(instance.to_string()),
        ]),
        Line::from(vec![
            Span::styled("entity:  ", Style::new().dim()),
            Span::raw(entity.to_string()),
        ]),
        Line::raw(""),
        Line::from(vec![Span::styled(
            format!("title (default: \"{default_label}\"):"),
            Style::new().bold(),
        )]),
        Line::from(vec![
            Span::raw("> "),
            Span::styled(title_buffer.to_string(), Style::new().bold()),
            Span::styled("_", Style::new().rapid_blink()),
        ]),
    ];
    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(" Title (Enter=accept, Esc=cancel) ")),
        r,
    );
}

fn draw_text_body(f: &mut Frame, area: Rect, title: &str, body: &str, focus_body: bool) {
    let r = modal_rect(area, 72, 16);
    f.render_widget(Clear, r);
    let title_style = if focus_body {
        Style::new().dim()
    } else {
        Style::new().bold()
    };
    let body_style = if focus_body {
        Style::new().bold()
    } else {
        Style::new().dim()
    };
    let lines = vec![
        Line::from(vec![
            Span::styled("title: ", title_style),
            Span::raw(title.to_string()),
        ]),
        Line::raw(""),
        Line::from(Span::styled(
            "body (markdown, Enter=newline, F2=accept):",
            body_style,
        )),
        Line::raw(""),
        Line::raw(body.to_string()),
        Line::styled(
            "[Tab cycles fields · F2 save · Esc cancel]".to_string(),
            Style::new().dim(),
        ),
    ];
    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(" New text card ")),
        r,
    );
}

fn draw_menu(
    f: &mut Frame,
    area: Rect,
    context: crate::dashboard::editor::MenuContext,
    items: &[crate::dashboard::editor::MenuItem],
    selected: usize,
) {
    let title = match context {
        crate::dashboard::editor::MenuContext::Card(idx) => {
            format!(" Card #{} settings ", idx + 1)
        }
        crate::dashboard::editor::MenuContext::Dashboard => " Dashboard settings ".to_string(),
    };
    let h = (items.len() as u16 + 4).min(area.height.saturating_sub(4));
    let r = modal_rect(area, 44, h.max(6));
    f.render_widget(Clear, r);
    let list_items: Vec<ListItem<'_>> = items
        .iter()
        .map(|m| ListItem::new(Line::raw(m.label.to_string())))
        .collect();
    let mut state = ListState::default();
    if !list_items.is_empty() {
        state.select(Some(selected.min(list_items.len() - 1)));
    }
    f.render_stateful_widget(
        List::new(list_items)
            .block(Block::bordered().title(title))
            .highlight_style(Style::new().reversed())
            .highlight_symbol("▶ "),
        r,
        &mut state,
    );
}

fn draw_filter_query(
    f: &mut Frame,
    area: Rect,
    instance: &str,
    query: &str,
    title: &str,
    hide_state: bool,
    focus: crate::dashboard::editor::FilterFocus,
) {
    use crate::dashboard::editor::FilterFocus as F;
    let r = modal_rect(area, 84, 14);
    f.render_widget(Clear, r);
    let f_style = |target: F| -> Style {
        if focus == target {
            Style::new().bold()
        } else {
            Style::new().dim()
        }
    };
    let lines = vec![
        Line::from(vec![
            Span::styled("instance: ", Style::new().dim()),
            Span::raw(instance.to_string()),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled("query: ", f_style(F::Query)),
            Span::styled(query.to_string(), f_style(F::Query)),
            if matches!(focus, F::Query) {
                Span::styled("_", Style::new().rapid_blink())
            } else {
                Span::raw("")
            },
        ]),
        Line::from(Span::styled(
            "  format: glob[state=on][attr.location=\"HBG - Helsingborg\"]",
            Style::new().dim(),
        )),
        Line::raw(""),
        Line::from(vec![
            Span::styled("title: ", f_style(F::Title)),
            Span::styled(title.to_string(), f_style(F::Title)),
            if matches!(focus, F::Title) {
                Span::styled("_", Style::new().rapid_blink())
            } else {
                Span::raw("")
            },
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled("hide state column: ", f_style(F::HideToggle)),
            Span::styled(
                if hide_state { "[x]" } else { "[ ]" },
                f_style(F::HideToggle),
            ),
            if matches!(focus, F::HideToggle) {
                Span::styled("  (space to toggle)", Style::new().dim())
            } else {
                Span::raw("")
            },
        ]),
        Line::raw(""),
        Line::styled(
            "[Tab cycles fields · F2 save · Esc cancel]".to_string(),
            Style::new().dim(),
        ),
    ];
    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(" Filtered entity list ")),
        r,
    );
}

fn draw_window_edit(f: &mut Frame, area: Rect, buffer: &str) {
    let r = modal_rect(area, 56, 6);
    f.render_widget(Clear, r);
    let lines = vec![
        Line::raw("Graph history window — examples: 1h, 6h, 24h, 7d"),
        Line::raw(""),
        Line::from(vec![
            Span::raw("> "),
            Span::styled(buffer.to_string(), Style::new().bold()),
            Span::styled("_", Style::new().rapid_blink()),
        ]),
    ];
    f.render_widget(
        Paragraph::new(lines)
            .block(Block::bordered().title(" History window (Enter=apply + refetch, Esc=cancel) ")),
        r,
    );
}

fn draw_card_rename(f: &mut Frame, area: Rect, buffer: &str) {
    let r = modal_rect(area, 56, 5);
    f.render_widget(Clear, r);
    let lines = vec![
        Line::raw("Rename selected card (blank = clear, reverts to default):"),
        Line::raw(""),
        Line::from(vec![
            Span::raw("> "),
            Span::styled(buffer.to_string(), Style::new().bold()),
            Span::styled("_", Style::new().rapid_blink()),
        ]),
    ];
    f.render_widget(
        Paragraph::new(lines)
            .block(Block::bordered().title(" Card title (Enter=save, Esc=cancel) ")),
        r,
    );
}

fn draw_rename(f: &mut Frame, area: Rect, buffer: &str) {
    let r = modal_rect(area, 56, 5);
    f.render_widget(Clear, r);
    let lines = vec![
        Line::raw("Rename dashboard:"),
        Line::raw(""),
        Line::from(vec![
            Span::raw("> "),
            Span::styled(buffer.to_string(), Style::new().bold()),
            Span::styled("_", Style::new().rapid_blink()),
        ]),
    ];
    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(" Rename (Enter=save, Esc=cancel) ")),
        r,
    );
}

fn draw_text_input(f: &mut Frame, area: Rect, title: &str, hint: &str, buffer: &str) {
    let r = modal_rect(area, 64, 6);
    f.render_widget(Clear, r);
    let lines = vec![
        Line::raw(hint.to_string()),
        Line::raw(""),
        Line::from(vec![
            Span::raw("> "),
            Span::styled(buffer.to_string(), Style::new().bold()),
            Span::styled("_", Style::new().rapid_blink()),
        ]),
    ];
    f.render_widget(
        Paragraph::new(lines)
            .block(Block::bordered().title(format!(" {} (Enter=apply, Esc=cancel) ", title))),
        r,
    );
}

fn draw_resize_grid(f: &mut Frame, area: Rect, cols: &str, rows: &str, focus_rows: bool) {
    let r = modal_rect(area, 56, 7);
    f.render_widget(Clear, r);
    let cstyle = if focus_rows {
        Style::new().dim()
    } else {
        Style::new().bold()
    };
    let rstyle = if focus_rows {
        Style::new().bold()
    } else {
        Style::new().dim()
    };
    let lines = vec![
        Line::from(vec![
            Span::styled("cols: ", cstyle),
            Span::raw(cols.to_string()),
        ]),
        Line::from(vec![
            Span::styled("rows: ", rstyle),
            Span::raw(rows.to_string()),
        ]),
        Line::raw(""),
        Line::styled(
            "Tab cycle · Enter accept · Esc cancel".to_string(),
            Style::new().dim(),
        ),
    ];
    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(" Grid size ")),
        r,
    );
}

fn draw_multi_picker(
    f: &mut Frame,
    area: Rect,
    app: &App,
    instance: &str,
    query: &str,
    selected: usize,
    picked: &[(String, String)],
) {
    let w = 90u16.min(area.width.saturating_sub(2));
    let h = 26u16.min(area.height.saturating_sub(2));
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let r = Rect {
        x,
        y,
        width: w,
        height: h,
    };
    f.render_widget(Clear, r);
    let color = app.theme.instance_color(instance);
    let title = format!(
        " entity list → {}  (Space=toggle, Enter=done, ↑/↓ select, Esc cancel) ",
        instance
    );
    f.render_widget(Block::bordered().title(title), r);

    let inner = Rect {
        x: r.x + 1,
        y: r.y + 1,
        width: r.width.saturating_sub(2),
        height: r.height.saturating_sub(2),
    };
    let search_row = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    };
    let picked_label = Rect {
        x: inner.x,
        y: inner.y + 1,
        width: inner.width,
        height: 1,
    };
    let list_row = Rect {
        x: inner.x,
        y: inner.y + 3,
        width: inner.width,
        height: inner.height.saturating_sub(4),
    };

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("search: ", Style::new().dim()),
            Span::styled(query.to_string(), Style::new().fg(color).bold()),
            Span::styled("_", Style::new().fg(color).rapid_blink()),
        ])),
        search_row,
    );

    let picked_text = if picked.is_empty() {
        "(none yet)".to_string()
    } else {
        picked
            .iter()
            .map(|(eid, _)| eid.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("picked: ", Style::new().dim()),
            Span::styled(picked_text, Style::new().fg(color)),
        ])),
        picked_label,
    );

    let rows = crate::app::entity_search(&app.instances, instance, query);
    let items: Vec<ListItem<'_>> = rows
        .iter()
        .map(|p| {
            let chosen = picked.iter().any(|(eid, _)| eid == &p.entity_id);
            let primary = if p.friendly_name.is_empty() {
                p.entity_id.clone()
            } else {
                p.friendly_name.clone()
            };
            let mark = if chosen { "[x] " } else { "[ ] " };
            ListItem::new(Line::from(vec![
                Span::styled(mark.to_string(), Style::new().bold()),
                Span::styled(primary, Style::new().fg(color).bold()),
                Span::raw("  "),
                Span::styled(p.entity_id.clone(), Style::new().dim()),
            ]))
        })
        .collect();
    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(Some(selected.min(items.len() - 1)));
    }
    f.render_stateful_widget(
        List::new(items)
            .highlight_style(Style::new().reversed())
            .highlight_symbol("▶ "),
        list_row,
        &mut state,
    );
}

fn draw_entity_list_title(
    f: &mut Frame,
    area: Rect,
    instance: &str,
    picked: &[(String, String)],
    title_buffer: &str,
) {
    let r = modal_rect(area, 72, 10);
    f.render_widget(Clear, r);
    let count = picked.len();
    let preview = picked
        .iter()
        .take(3)
        .map(|(eid, _)| eid.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let preview = if count > 3 {
        format!("{preview}, … ({count} total)")
    } else {
        preview
    };
    let lines = vec![
        Line::from(vec![
            Span::styled("instance: ", Style::new().dim()),
            Span::raw(instance.to_string()),
        ]),
        Line::from(vec![
            Span::styled("entities: ", Style::new().dim()),
            Span::raw(preview),
        ]),
        Line::raw(""),
        Line::from(Span::styled(
            "title (default: \"Entities\"):",
            Style::new().bold(),
        )),
        Line::from(vec![
            Span::raw("> "),
            Span::styled(title_buffer.to_string(), Style::new().bold()),
            Span::styled("_", Style::new().rapid_blink()),
        ]),
    ];
    f.render_widget(
        Paragraph::new(lines)
            .block(Block::bordered().title(" Entity list title (Enter=accept, Esc=cancel) ")),
        r,
    );
}

fn draw_confirm(f: &mut Frame, area: Rect, msg: &str) {
    let r = modal_rect(area, msg.len() as u16 + 8, 3);
    f.render_widget(Clear, r);
    f.render_widget(
        Paragraph::new(msg).block(Block::bordered().title(" Confirm ")),
        r,
    );
}

fn draw_color_override(f: &mut Frame, area: Rect, buf: &str) {
    let r = modal_rect(area, 64, 5);
    f.render_widget(Clear, r);
    let lines = vec![
        Line::styled(
            "Enter named color or #rrggbb · empty to clear · Esc cancel",
            Style::new().dim(),
        ),
        Line::raw(""),
        Line::from(vec![
            Span::raw("> "),
            Span::styled(buf.to_string(), Style::new().bold()),
            Span::styled("_", Style::new().rapid_blink()),
        ]),
    ];
    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(" Color override ")),
        r,
    );
}

fn draw_pick_size(f: &mut Frame, area: Rect, current: CardSize) {
    const SIZES: [(CardSize, &str); 3] = [
        (CardSize::Small, "Small"),
        (CardSize::Normal, "Normal"),
        (CardSize::Large, "Large"),
    ];
    let selected = SIZES.iter().position(|(s, _)| *s == current).unwrap_or(1);
    let items: Vec<ListItem<'_>> = SIZES
        .iter()
        .map(|(_, label)| ListItem::new(Line::raw(label.to_string())))
        .collect();
    let mut state = ListState::default();
    state.select(Some(selected));
    let r = modal_rect(area, 32, 7);
    f.render_widget(Clear, r);
    f.render_stateful_widget(
        List::new(items)
            .block(Block::bordered().title(" Size (j/k · Enter · Esc) "))
            .highlight_style(Style::new().reversed())
            .highlight_symbol("▶ "),
        r,
        &mut state,
    );
}

// ── Gauge severity draw helper ────────────────────────────────────────────────

fn draw_numeric_prompt(
    f: &mut Frame,
    area: Rect,
    title: &str,
    hint: &str,
    buf: &str,
    accum: &SeverityAccum,
) {
    let r = modal_rect(area, 64, 9);
    f.render_widget(Clear, r);
    let lines = vec![
        Line::styled(hint.to_string(), Style::new().bold()),
        Line::raw(""),
        Line::from(vec![
            Span::raw("> "),
            Span::styled(buf.to_string(), Style::new().bold()),
            Span::styled("_", Style::new().rapid_blink()),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled("green: ", Style::new().fg(Color::Green).dim()),
            Span::styled(
                format!("{:.1}", accum.green),
                Style::new().fg(Color::Green).dim(),
            ),
            Span::raw("    "),
            Span::styled("yellow: ", Style::new().fg(Color::Yellow).dim()),
            Span::styled(
                format!("{:.1}", accum.yellow),
                Style::new().fg(Color::Yellow).dim(),
            ),
        ]),
        Line::raw(""),
        Line::styled(
            "Enter number · Enter to advance · Esc to cancel",
            Style::new().dim(),
        ),
    ];
    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(title.to_string())),
        r,
    );
}

// ── Graph add-flow draw helpers ───────────────────────────────────────────────

fn draw_graph_pick_type(f: &mut Frame, area: Rect) {
    const TYPES: [(&str, &str); 3] = [
        ("1", "Line  — time series chart"),
        ("2", "Bar   — bar chart"),
        ("3", "Pie   — pie chart"),
    ];
    let r = modal_rect(area, 40, 7);
    f.render_widget(Clear, r);
    let lines: Vec<Line<'_>> = TYPES
        .iter()
        .map(|(key, label)| {
            Line::from(vec![
                Span::styled(format!(" {key} "), Style::new().reversed()),
                Span::raw("  "),
                Span::raw(*label),
            ])
        })
        .collect();
    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(" Graph — pick type (Esc cancel) ")),
        r,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_graph_add_entities(
    f: &mut Frame,
    area: Rect,
    app: &App,
    instance: &str,
    accumulated: &[crate::dashboard::GraphSeries],
    query: &str,
    selected: usize,
    asking_more: bool,
) {
    let w = 80u16.min(area.width.saturating_sub(2));
    let h = 26u16.min(area.height.saturating_sub(2));
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let r = Rect {
        x,
        y,
        width: w,
        height: h,
    };
    f.render_widget(Clear, r);
    let color = app.theme.instance_color(instance);

    let added = accumulated
        .iter()
        .map(|s| s.entity.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let added_label = if added.is_empty() {
        "(none yet)".into()
    } else {
        added
    };

    if asking_more {
        let inner = Rect {
            x: r.x + 1,
            y: r.y + 1,
            width: r.width.saturating_sub(2),
            height: r.height.saturating_sub(2),
        };
        f.render_widget(Block::bordered().title(" Graph — add entity "), r);
        let lines = vec![
            Line::from(vec![
                Span::styled("added: ", Style::new().dim()),
                Span::styled(added_label, Style::new().fg(color)),
            ]),
            Line::raw(""),
            Line::from(Span::styled(
                "Add another entity? (y / n / Esc = done)",
                Style::new().bold(),
            )),
        ];
        f.render_widget(Paragraph::new(lines), inner);
        return;
    }

    let title = format!(
        " graph → {}  (type to search, ↑/↓ select, ⏎ pick, Esc = done) ",
        instance
    );
    f.render_widget(Block::bordered().title(title), r);

    let inner = Rect {
        x: r.x + 1,
        y: r.y + 1,
        width: r.width.saturating_sub(2),
        height: r.height.saturating_sub(2),
    };
    let search_row = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    };
    let picked_row = Rect {
        x: inner.x,
        y: inner.y + 1,
        width: inner.width,
        height: 1,
    };
    let list_row = Rect {
        x: inner.x,
        y: inner.y + 3,
        width: inner.width,
        height: inner.height.saturating_sub(4),
    };

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("search: ", Style::new().dim()),
            Span::styled(query.to_string(), Style::new().fg(color).bold()),
            Span::styled("_", Style::new().fg(color).rapid_blink()),
        ])),
        search_row,
    );
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("added: ", Style::new().dim()),
            Span::styled(added_label, Style::new().fg(color)),
        ])),
        picked_row,
    );

    let rows = crate::app::entity_search(&app.instances, instance, query);
    let items: Vec<ListItem<'_>> = rows
        .iter()
        .map(|p| {
            let primary = if p.friendly_name.is_empty() {
                p.entity_id.clone()
            } else {
                p.friendly_name.clone()
            };
            ListItem::new(Line::from(vec![
                Span::styled(primary, Style::new().fg(color).bold()),
                Span::raw("  "),
                Span::styled(p.entity_id.clone(), Style::new().dim()),
            ]))
        })
        .collect();
    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(Some(selected.min(items.len() - 1)));
    }
    f.render_stateful_widget(
        List::new(items)
            .highlight_style(Style::new().reversed())
            .highlight_symbol("▶ "),
        list_row,
        &mut state,
    );
}

fn draw_graph_window_add(
    f: &mut Frame,
    area: Rect,
    window_buf: &str,
    title_buf: &str,
    title_stage: bool,
) {
    let r = modal_rect(area, 60, 8);
    f.render_widget(Clear, r);
    let lines = if title_stage {
        vec![
            Line::raw("Optional card title (blank = no title):"),
            Line::raw(""),
            Line::from(vec![
                Span::raw("> "),
                Span::styled(title_buf.to_string(), Style::new().bold()),
                Span::styled("_", Style::new().rapid_blink()),
            ]),
            Line::raw(""),
            Line::from(vec![
                Span::styled("window: ", Style::new().dim()),
                Span::raw(window_buf.to_string()),
            ]),
        ]
    } else {
        vec![
            Line::raw("History window — examples: 1h, 6h, 24h, 7d"),
            Line::raw(""),
            Line::from(vec![
                Span::raw("> "),
                Span::styled(window_buf.to_string(), Style::new().bold()),
                Span::styled("_", Style::new().rapid_blink()),
            ]),
        ]
    };
    f.render_widget(
        Paragraph::new(lines)
            .block(Block::bordered().title(" Graph config (Enter=next, Esc=cancel) ")),
        r,
    );
}

fn draw_graph_orientation_add(
    f: &mut Frame,
    area: Rect,
    current: BarOrientation,
    title_buf: &str,
    title_stage: bool,
) {
    if title_stage {
        let r = modal_rect(area, 60, 6);
        f.render_widget(Clear, r);
        let lines = vec![
            Line::raw("Optional card title (blank = no title):"),
            Line::raw(""),
            Line::from(vec![
                Span::raw("> "),
                Span::styled(title_buf.to_string(), Style::new().bold()),
                Span::styled("_", Style::new().rapid_blink()),
            ]),
        ];
        f.render_widget(
            Paragraph::new(lines)
                .block(Block::bordered().title(" Graph config (Enter=done, Esc=cancel) ")),
            r,
        );
        return;
    }
    const OPTS: [(BarOrientation, &str); 2] = [
        (BarOrientation::Vertical, "Vertical"),
        (BarOrientation::Horizontal, "Horizontal"),
    ];
    let selected = OPTS.iter().position(|(o, _)| *o == current).unwrap_or(0);
    let items: Vec<ListItem<'_>> = OPTS
        .iter()
        .map(|(_, label)| ListItem::new(Line::raw(label.to_string())))
        .collect();
    let mut state = ListState::default();
    state.select(Some(selected));
    let r = modal_rect(area, 36, 6);
    f.render_widget(Clear, r);
    f.render_stateful_widget(
        List::new(items)
            .block(Block::bordered().title(" Bar orientation (j/k · Enter · Esc) "))
            .highlight_style(Style::new().reversed())
            .highlight_symbol("▶ "),
        r,
        &mut state,
    );
}

// ── Graph context-menu draw helpers ──────────────────────────────────────────

fn draw_graph_add_one_series(
    f: &mut Frame,
    area: Rect,
    app: &App,
    card_idx: usize,
    query: &str,
    selected: usize,
) {
    // Determine instance from card
    let instance = app
        .editor
        .as_ref()
        .and_then(|ed| app.dashboards.get(ed.dash_idx))
        .and_then(|d| d.cards.get(card_idx))
        .and_then(|c| {
            if let crate::dashboard::CardKind::Graph { instance, .. } = &c.kind {
                Some(instance.clone())
            } else {
                None
            }
        })
        .unwrap_or_default();

    let w = 80u16.min(area.width.saturating_sub(2));
    let h = 24u16.min(area.height.saturating_sub(2));
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let r = Rect {
        x,
        y,
        width: w,
        height: h,
    };
    f.render_widget(Clear, r);
    let color = app.theme.instance_color(&instance);
    f.render_widget(
        Block::bordered().title(format!(
            " Add series → {}  (type to search, ⏎ pick, Esc cancel) ",
            instance
        )),
        r,
    );

    let inner = Rect {
        x: r.x + 1,
        y: r.y + 1,
        width: r.width.saturating_sub(2),
        height: r.height.saturating_sub(2),
    };
    let search_row = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    };
    let list_row = Rect {
        x: inner.x,
        y: inner.y + 2,
        width: inner.width,
        height: inner.height.saturating_sub(3),
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("search: ", Style::new().dim()),
            Span::styled(query.to_string(), Style::new().fg(color).bold()),
            Span::styled("_", Style::new().fg(color).rapid_blink()),
        ])),
        search_row,
    );
    let rows = crate::app::entity_search(&app.instances, &instance, query);
    let items: Vec<ListItem<'_>> = rows
        .iter()
        .map(|p| {
            let primary = if p.friendly_name.is_empty() {
                p.entity_id.clone()
            } else {
                p.friendly_name.clone()
            };
            ListItem::new(Line::from(vec![
                Span::styled(primary, Style::new().fg(color).bold()),
                Span::raw("  "),
                Span::styled(p.entity_id.clone(), Style::new().dim()),
            ]))
        })
        .collect();
    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(Some(selected.min(items.len() - 1)));
    }
    f.render_stateful_widget(
        List::new(items)
            .highlight_style(Style::new().reversed())
            .highlight_symbol("▶ "),
        list_row,
        &mut state,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_graph_pick_series(
    f: &mut Frame,
    area: Rect,
    _app: &App,
    editor: &crate::dashboard::editor::EditorState,
    dash: &crate::dashboard::Dashboard,
    card_idx: usize,
    op: SeriesIndexOp,
    selected: usize,
) {
    let entities: Vec<String> = dash
        .cards
        .get(card_idx)
        .and_then(|c| {
            if let crate::dashboard::CardKind::Graph { entities, .. } = &c.kind {
                Some(
                    entities
                        .iter()
                        .map(|s| s.label.clone().unwrap_or_else(|| s.entity.clone()))
                        .collect(),
                )
            } else {
                None
            }
        })
        .unwrap_or_default();

    let op_label = match op {
        SeriesIndexOp::Remove => "Remove series",
        SeriesIndexOp::SetColor => "Set series colour",
        SeriesIndexOp::SetLabel => "Set series label",
    };
    let h = (entities.len() as u16 + 4)
        .max(6)
        .min(area.height.saturating_sub(4));
    let r = modal_rect(area, 60, h);
    f.render_widget(Clear, r);
    let items: Vec<ListItem<'_>> = entities
        .iter()
        .map(|label| ListItem::new(Line::raw(label.clone())))
        .collect();
    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(Some(selected.min(items.len() - 1)));
        let _ = editor.dash_idx; // suppress unused warning
    }
    f.render_stateful_widget(
        List::new(items)
            .block(
                Block::bordered()
                    .title(format!(" {} — pick series (j/k · Enter · Esc) ", op_label)),
            )
            .highlight_style(Style::new().reversed())
            .highlight_symbol("▶ "),
        r,
        &mut state,
    );
}

fn draw_graph_series_color(f: &mut Frame, area: Rect, buf: &str) {
    let r = modal_rect(area, 64, 5);
    f.render_widget(Clear, r);
    let lines = vec![
        Line::styled(
            "Enter named color or #rrggbb · empty to clear · Esc cancel",
            Style::new().dim(),
        ),
        Line::raw(""),
        Line::from(vec![
            Span::raw("> "),
            Span::styled(buf.to_string(), Style::new().bold()),
            Span::styled("_", Style::new().rapid_blink()),
        ]),
    ];
    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(" Series colour ")),
        r,
    );
}

fn draw_graph_series_label(f: &mut Frame, area: Rect, buf: &str) {
    let r = modal_rect(area, 64, 5);
    f.render_widget(Clear, r);
    let lines = vec![
        Line::styled(
            "Enter label · empty to clear · Esc cancel",
            Style::new().dim(),
        ),
        Line::raw(""),
        Line::from(vec![
            Span::raw("> "),
            Span::styled(buf.to_string(), Style::new().bold()),
            Span::styled("_", Style::new().rapid_blink()),
        ]),
    ];
    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(" Series label ")),
        r,
    );
}

fn draw_graph_pick_orientation(f: &mut Frame, area: Rect, current: BarOrientation) {
    const OPTS: [(BarOrientation, &str); 2] = [
        (BarOrientation::Vertical, "Vertical"),
        (BarOrientation::Horizontal, "Horizontal"),
    ];
    let selected = OPTS.iter().position(|(o, _)| *o == current).unwrap_or(0);
    let items: Vec<ListItem<'_>> = OPTS
        .iter()
        .map(|(_, label)| ListItem::new(Line::raw(label.to_string())))
        .collect();
    let mut state = ListState::default();
    state.select(Some(selected));
    let r = modal_rect(area, 36, 6);
    f.render_widget(Clear, r);
    f.render_stateful_widget(
        List::new(items)
            .block(Block::bordered().title(" Bar orientation (j/k · Enter · Esc) "))
            .highlight_style(Style::new().reversed())
            .highlight_symbol("▶ "),
        r,
        &mut state,
    );
}

// ── Statistics metric picker ──────────────────────────────────────────────────

fn draw_stats_pick_metric(f: &mut Frame, area: Rect, selected: usize) {
    const METRICS: [(&str, &str); 5] = [
        ("1", "avg   — average value"),
        ("2", "min   — minimum value"),
        ("3", "max   — maximum value"),
        ("4", "sum   — sum of values"),
        ("5", "count — number of samples"),
    ];
    let items: Vec<ListItem<'_>> = METRICS
        .iter()
        .map(|(key, label)| {
            ListItem::new(Line::from(vec![
                Span::styled(format!(" {key} "), Style::new().reversed()),
                Span::raw("  "),
                Span::raw(*label),
            ]))
        })
        .collect();
    let mut state = ListState::default();
    state.select(Some(selected.min(METRICS.len() - 1)));
    let r = modal_rect(area, 44, 9);
    f.render_widget(Clear, r);
    f.render_stateful_widget(
        List::new(items)
            .block(Block::bordered().title(" Statistics — pick metric (j/k · Enter · Esc) "))
            .highlight_style(Style::new().reversed())
            .highlight_symbol("▶ "),
        r,
        &mut state,
    );
}
