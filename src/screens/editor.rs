use ratatui::layout::Rect;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::dashboard::editor::{CardTypeStub, EditorMode};
use crate::dashboard::layout::cell_to_rect;

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
        EditorMode::ResizingGrid {
            cols_buffer,
            rows_buffer,
            focus_rows,
        } => draw_resize_grid(f, area, cols_buffer, rows_buffer, *focus_rows),
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
    let r = modal_rect(area, 32, 9);
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

fn draw_window_edit(f: &mut Frame, area: Rect, buffer: &str) {
    let r = modal_rect(area, 56, 6);
    f.render_widget(Clear, r);
    let lines = vec![
        Line::raw("Sparkline window — examples: 1h, 6h, 24h, 7d"),
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
