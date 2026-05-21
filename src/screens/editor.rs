use std::collections::HashMap;

use ratatui::layout::{Constraint, Layout, Rect};
#[allow(unused_imports)]
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::dashboard::editor::{GridFocus, SeverityAccum};
use crate::dashboard::editor::{CardTypeStub, EditorMode, SeriesIndexOp, TransferOp};
use crate::dashboard::DashboardLayout;
use crate::dashboard::layout::{cell_to_rect, grid_layout};
use crate::dashboard::{BarOrientation, CardSize};

pub fn draw(f: &mut Frame, area: Rect, app: &mut App) {
    // Reserve the last line for the editor help bar.
    let chunks = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(area);
    let main_area = chunks[0];
    let help_area = chunks[1];

    // Detect layout type before the mutable dashboard draw.
    let (dash_idx, is_grid, cursor_col, cursor_row, dash_grid, card_pos, grid_focus, selected_card) = {
        let Some(editor) = app.editor.as_ref() else { return; };
        let Some(dash) = app.dashboards.get(editor.dash_idx) else { return; };
        let is_grid = matches!(&dash.layout, DashboardLayout::Grid { .. });
        let card_pos = if !is_grid {
            editor.selected_card.and_then(|i| dash.card(i)).and_then(|c| c.pos)
        } else {
            None
        };
        (
            editor.dash_idx,
            is_grid,
            editor.cursor_col,
            editor.cursor_row,
            dash.free_grid().unwrap_or(crate::dashboard::Grid { cols: 12, rows: 8 }),
            card_pos,
            editor.grid_focus,
            editor.selected_card,
        )
    };

    crate::screens::dashboard::draw(f, main_area, app, dash_idx, usize::MAX, None);

    if is_grid {
        draw_grid_editor_overlay(f, main_area, app, dash_idx, grid_focus, selected_card);
    } else {
        // Free-canvas: cursor highlight + selected card border.
        let cur_rect = cell_to_rect(
            main_area,
            dash_grid,
            crate::dashboard::Pos { col: cursor_col, row: cursor_row, w: 1, h: 1 },
        );
        if cur_rect.width > 0 && cur_rect.height > 0 {
            f.render_widget(Block::new().style(Style::new().bg(Color::DarkGray)), cur_rect);
        }
        if let Some(pos) = card_pos {
            let r = cell_to_rect(main_area, dash_grid, pos);
            f.render_widget(Block::bordered().border_style(Style::new().fg(Color::Yellow).bold()), r);
        }
    }

    // Help bar.
    draw_editor_help_line(f, help_area, app, is_grid);

    // Re-borrow editor and dash after the &mut dashboard draw.
    let Some(editor) = app.editor.as_ref() else { return; };
    let Some(dash) = app.dashboards.get(editor.dash_idx) else { return; };

    match &editor.mode {
        EditorMode::PickingType { selected } => draw_palette(f, area, *selected),
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
        EditorMode::EditEntityListItemOverride {
            name_buf,
            hide_state,
            entity_id,
            focus_entity_id,
            ..
        } => draw_entity_list_item_override(
            f,
            area,
            entity_id.as_deref(),
            *focus_entity_id,
            name_buf,
            *hide_state,
        ),
        EditorMode::PickEntityListItemToOverride {
            card_idx,
            items,
            selected,
        } => draw_pick_entity_list_item(f, area, dash, *card_idx, items, *selected),
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
        // Image add-flow
        EditorMode::ImagePickSourceKind { selected } => {
            draw_image_pick_source(f, area, *selected);
        }
        EditorMode::ImageEditRefreshSeconds { buf, .. } => {
            draw_text_input(
                f,
                area,
                " New image card (3/4) ",
                "Refresh interval in seconds (Enter to skip)",
                buf,
            );
        }
        EditorMode::ImageEditTitleAdd { buf, .. } => {
            draw_text_input(
                f,
                area,
                " New image card (4/4) ",
                "Title (optional — Enter to use entity name)",
                buf,
            );
        }
        // Weather add-flow
        EditorMode::WxEditShowForecast { selected, .. } => {
            draw_wx_show_forecast(f, area, *selected);
        }
        EditorMode::WxEditForecastDays {
            buf, show_forecast, ..
        } => {
            let hint = if *show_forecast {
                "Days of forecast to show (1–7, default 3)"
            } else {
                "Days of forecast (1–7, default 3 — ignored since forecast is off)"
            };
            draw_text_input(f, area, " New weather card (4/5) ", hint, buf);
        }
        EditorMode::WxEditTitleAdd { buf, .. } => {
            draw_text_input(
                f,
                area,
                " New weather card (5/5) ",
                "Title (optional — Enter to use entity name)",
                buf,
            );
        }
        // Cross-dashboard transfer flows
        EditorMode::PickingTargetDashboard { op, selected, .. } => {
            draw_dashboard_picker(f, area, app, *selected, *op, "Pick target dashboard");
        }
        EditorMode::PickingTargetGridRow { target_dash, selected, .. } => {
            draw_grid_row_picker(f, area, app, *target_dash, *selected);
        }
        EditorMode::PickingTargetGridColumn { target_dash, target_row, selected, .. } => {
            draw_grid_col_picker(f, area, app, *target_dash, *target_row, *selected);
        }
        EditorMode::PickingNewDashboardLayout { selected } => {
            draw_new_dashboard_layout_picker(f, area, *selected);
        }
        EditorMode::ConfirmDeleteDashboard => {
            draw_confirm(f, area, "Delete this dashboard? (y/n)");
        }
        EditorMode::PickingNewRowHeight { buf } => {
            draw_text_input(f, area, "Add row (1/2)", "Enter height: integer or \"auto\"", buf);
        }
        EditorMode::PickingNewRowColumnCount { buf, .. } => {
            draw_text_input(f, area, "Add row (2/2)", "Number of columns", buf);
        }
        EditorMode::EditingRowHeight { buf, row_idx } => {
            draw_text_input(f, area, &format!("Row {} height", row_idx + 1), "integer or \"auto\"", buf);
        }
        EditorMode::ConfirmRemoveRow { row_idx } => {
            draw_confirm(f, area, &format!("Remove row {}? (y/n)", row_idx + 1));
        }
        EditorMode::ConfirmRemoveColumn { row_idx, col_idx } => {
            draw_confirm(f, area, &format!("Remove col {} from row {}? (y/n)", col_idx + 1, row_idx + 1));
        }
        EditorMode::Browse => {}
    }
}

fn draw_new_dashboard_layout_picker(f: &mut Frame, area: Rect, selected: usize) {
    let r = modal_rect(area, 52, 8);
    f.render_widget(Clear, r);
    let block = Block::bordered().title(" New dashboard — pick layout (j/k + Enter, or 1/2) ");
    let inner = block.inner(r);
    f.render_widget(block, r);
    let items: [(&str, &str, &str); 2] = [
        ("1", "Free canvas", "place cards at any (col, row, w, h) position"),
        ("2", "Grid layout", "stacked rows of columns, cards fill column width"),
    ];
    let list_items: Vec<ListItem> = items
        .iter()
        .enumerate()
        .map(|(i, (key, name, desc))| {
            let style = if i == selected {
                Style::new().bold().fg(Color::Yellow)
            } else {
                Style::new()
            };
            ListItem::new(format!("[{key}] {name} — {desc}")).style(style)
        })
        .collect();
    let list = List::new(list_items);
    f.render_widget(list, inner);
}

fn draw_image_pick_source(f: &mut Frame, area: Rect, selected: usize) {
    let r = modal_rect(area, 44, 6);
    f.render_widget(Clear, r);
    let block = Block::bordered().title(" New image card (1/4) — j/k + Enter, or 1/2 ");
    let inner = block.inner(r);
    f.render_widget(block, r);
    let items: [(&str, &str); 2] = [
        ("1", "Image entity  (image.*)"),
        ("2", "Camera stream (camera.*)"),
    ];
    let list_items: Vec<ListItem> = items
        .iter()
        .enumerate()
        .map(|(i, (key, label))| {
            let style = if i == selected {
                Style::new().bold().fg(Color::Yellow)
            } else {
                Style::new()
            };
            ListItem::new(format!("[{key}] {label}")).style(style)
        })
        .collect();
    let list = List::new(list_items);
    f.render_widget(list, inner);
}

fn draw_wx_show_forecast(f: &mut Frame, area: Rect, selected: usize) {
    let r = modal_rect(area, 44, 6);
    f.render_widget(Clear, r);
    let block = Block::bordered().title(" New weather card (3/5) ");
    let inner = block.inner(r);
    f.render_widget(block, r);
    let items: [(&str, &str); 2] = [
        ("y", "Yes — show forecast strip"),
        ("n", "No  — current conditions only"),
    ];
    let list_items: Vec<ListItem> = items
        .iter()
        .enumerate()
        .map(|(i, (key, label))| {
            let style = if i == selected {
                Style::new().bold().fg(Color::Yellow)
            } else {
                Style::new()
            };
            ListItem::new(format!("[{key}] {label}")).style(style)
        })
        .collect();
    let list = List::new(list_items);
    f.render_widget(list, inner);
}

fn draw_grid_editor_overlay(
    f: &mut Frame,
    area: Rect,
    app: &App,
    dash_idx: usize,
    grid_focus: Option<GridFocus>,
    selected_flat: Option<usize>,
) {
    let Some(dash) = app.dashboards.get(dash_idx) else { return; };
    let DashboardLayout::Grid { rows } = &dash.layout else { return; };

    // Build col_scrolls like the dashboard renderer does.
    let col_scrolls: HashMap<(usize, usize), u16> = app
        .column_scroll
        .iter()
        .filter(|((di, _, _), _)| *di == dash_idx)
        .map(|((_, ri, ci), &offset)| ((*ri, *ci), offset))
        .collect();

    // Use zero heights — we only need col_infos for layout positions.
    let card_count = dash.card_count();
    let card_heights: Vec<u16> = vec![4; card_count];
    let (_slots, col_infos) = grid_layout(rows, area, &col_scrolls, &card_heights);

    for info in &col_infos {
        let (border_color, title_color) = match grid_focus {
            Some(GridFocus::Row { row }) if row == info.row_idx => {
                (Color::Yellow, Color::Yellow)
            }
            Some(GridFocus::Column { row, col })
                if row == info.row_idx && col == info.col_idx =>
            {
                (Color::Cyan, Color::Cyan)
            }
            Some(GridFocus::Card { row, col, .. })
                if row == info.row_idx && col == info.col_idx =>
            {
                (Color::Blue, Color::Blue)
            }
            _ => (Color::White, Color::White),
        };

        let row = rows.get(info.row_idx);
        let col = row.and_then(|r| r.columns.get(info.col_idx));
        let card_n = col.map(|c| c.cards.len()).unwrap_or(0);
        let title = format!(
            " R{} C{} ({} card{}) ",
            info.row_idx + 1,
            info.col_idx + 1,
            card_n,
            if card_n == 1 { "" } else { "s" }
        );

        let is_inactive = matches!(grid_focus, None)
            || matches!(
                grid_focus,
                Some(GridFocus::Card { row, col, .. }) | Some(GridFocus::Column { row, col })
                    if row != info.row_idx || col != info.col_idx
            )
            || matches!(
                grid_focus,
                Some(GridFocus::Row { row }) if row != info.row_idx
            );
        let block = Block::new()
            .borders(Borders::ALL)
            .border_style(if is_inactive {
                Style::new().fg(border_color).dim()
            } else {
                Style::new().fg(border_color)
            })
            .title(Span::styled(
                title,
                if is_inactive {
                    Style::new().fg(title_color).dim()
                } else {
                    Style::new().fg(title_color)
                },
            ));
        f.render_widget(block, info.rect);
    }

    // Highlight the selected card with a yellow border on top of everything.
    if let Some(flat) = selected_flat {
        // Find which col_info contains this card, then compute its rect within the slot.
        let loc = dash.locate_grid_flat(flat);
        if let Some((ri, ci, _)) = loc {
            if let Some(info) = col_infos.iter().find(|c| c.row_idx == ri && c.col_idx == ci) {
                // Re-run grid_layout with real heights to get the card slot rect.
                let n_cols = rows.get(ri).map(|r| r.columns.len() as u16).unwrap_or(1);
                let col_w = if n_cols > 0 { area.width / n_cols } else { area.width };
                let real_heights: Vec<u16> = dash
                    .cards_iter()
                    .map(|c| c.preferred_height(col_w, None))
                    .collect();
                let (slots, _) = grid_layout(rows, area, &col_scrolls, &real_heights);
                if let Some(slot) = slots.iter().find(|s| s.flat_idx == flat) {
                    f.render_widget(
                        Block::bordered().border_style(Style::new().fg(Color::Yellow).bold()),
                        slot.rect,
                    );
                }
                // Also highlight the focused column border in a brighter shade.
                let _ = info; // already drawn above
            }
        }
    }
}

fn draw_editor_help_line(f: &mut Frame, area: Rect, app: &App, is_grid: bool) {
    let Some(editor) = app.editor.as_ref() else { return; };

    let focus_str: String = if is_grid {
        match editor.grid_focus {
            Some(GridFocus::Card { row, col, pos_in_col }) => {
                format!("Row {} · Col {} · Card {}  ", row + 1, col + 1, pos_in_col + 1)
            }
            Some(GridFocus::Column { row, col }) => {
                format!("Row {} · Col {} [column focus]  ", row + 1, col + 1)
            }
            Some(GridFocus::Row { row }) => {
                format!("Row {} [row focus]  ", row + 1)
            }
            None => {
                if app.dashboards.get(editor.dash_idx).map(|d| d.card_count()).unwrap_or(0) == 0 {
                    "Empty grid — ".to_string()
                } else {
                    String::new()
                }
            }
        }
    } else {
        String::new()
    };

    let keys = if is_grid {
        "a=add  m=menu  j/k=up/dn  h/l=left/right  R=row focus  C=col focus  u=undo  s=save  Esc=exit"
    } else {
        "a=add  m=menu  hjkl=move cursor  Space=select  Enter=drop  H/L/K/J=resize  u=undo  s=save  Esc=exit"
    };

    let dirty = if editor.dirty { " [unsaved]" } else { "" };
    let line = Line::from(vec![
        Span::styled(&focus_str, Style::new().fg(Color::Cyan).bold()),
        Span::styled(keys, Style::new().fg(Color::DarkGray)),
        Span::styled(dirty, Style::new().fg(Color::Yellow)),
    ]);
    f.render_widget(Paragraph::new(line).style(Style::new().bg(Color::Black)), area);
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

fn draw_palette(f: &mut Frame, area: Rect, selected: usize) {
    let height = (CardTypeStub::ALL.len() as u16) + 2;
    let r = modal_rect(area, 44, height);
    f.render_widget(Clear, r);
    let lines: Vec<Line<'_>> = CardTypeStub::ALL
        .iter()
        .enumerate()
        .map(|(i, k)| {
            let idx_style = if i == selected {
                Style::new().reversed().bold()
            } else {
                Style::new().reversed()
            };
            let label_style = if i == selected {
                Style::new().bold()
            } else {
                Style::new()
            };
            let idx_label = if i < 9 {
                format!(" {} ", i + 1)
            } else {
                "   ".to_string()
            };
            Line::from(vec![
                Span::styled(idx_label, idx_style),
                Span::raw("  "),
                Span::styled(k.label(), label_style),
            ])
        })
        .collect();
    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(" Add card — j/k + Enter, or 1-9 ")),
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

    let domain_prefix = match card_type {
        CardTypeStub::Image => match app.editor.as_ref().and_then(|e| e.image_pending_is_camera) {
            Some(true) => Some("camera."),
            Some(false) => Some("image."),
            None => None,
        },
        other => crate::app::domain_prefix_for_type(other),
    };
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
        crate::dashboard::editor::MenuContext::Card(idx) => format!(" Card #{} settings ", idx + 1),
        crate::dashboard::editor::MenuContext::Dashboard => " Dashboard settings ".to_string(),
        crate::dashboard::editor::MenuContext::Row(r) => format!(" Row {} settings ", r + 1),
        crate::dashboard::editor::MenuContext::GridColumn(r, c) => format!(" Row {} col {} settings ", r + 1, c + 1),
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

// `entity_id`: `Some(buf)` for FilteredEntityList (user must supply entity_id);
// `None` for EntityList (entity known from position).
fn draw_entity_list_item_override(
    f: &mut Frame,
    area: Rect,
    entity_id: Option<&str>,
    focus_entity_id: bool,
    name_buf: &str,
    hide_state: bool,
) {
    let has_eid_field = entity_id.is_some();
    let modal_h = if has_eid_field { 10 } else { 7 };
    let r = modal_rect(area, 50, modal_h);
    f.render_widget(Clear, r);
    let hide_label = if hide_state { "[x] hide state" } else { "[ ] hide state" };

    let mut lines: Vec<Line<'_>> = Vec::new();

    // entity_id field — only shown for FilteredEntityList
    if let Some(eid_buf) = entity_id {
        let eid_style = if focus_entity_id {
            Style::new().bold()
        } else {
            Style::new().dim()
        };
        lines.push(Line::styled("Entity ID (e.g. sensor.kitchen_temp)", eid_style));
        let mut eid_spans = vec![Span::raw("> "), Span::styled(eid_buf.to_string(), eid_style)];
        if focus_entity_id {
            eid_spans.push(Span::styled("_", Style::new().rapid_blink()));
        }
        lines.push(Line::from(eid_spans));
        lines.push(Line::raw(""));
    }

    // name field
    let name_style = if !focus_entity_id {
        Style::new().bold()
    } else {
        Style::new().dim()
    };
    lines.push(Line::styled("Name override (blank = use friendly name)", name_style));
    let mut name_spans = vec![Span::raw("> "), Span::styled(name_buf.to_string(), name_style)];
    if !focus_entity_id {
        name_spans.push(Span::styled("_", Style::new().rapid_blink()));
    }
    lines.push(Line::from(name_spans));
    lines.push(Line::raw(""));
    lines.push(Line::styled(hide_label.to_string(), Style::new().bold()));
    lines.push(Line::raw(""));

    let footer = if has_eid_field && focus_entity_id {
        "Tab / Enter advance · Esc cancel"
    } else {
        "Enter save · Tab toggle hide_state · Esc cancel"
    };
    lines.push(Line::styled(footer.to_string(), Style::new().dim()));

    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(" Override entry ")),
        r,
    );
}

fn draw_pick_entity_list_item(
    f: &mut Frame,
    area: Rect,
    dash: &crate::dashboard::Dashboard,
    card_idx: usize,
    items: &[(usize, String)],
    selected: usize,
) {
    let h = (items.len() as u16 + 4)
        .max(6)
        .min(area.height.saturating_sub(4));
    let r = modal_rect(area, 60, h);
    f.render_widget(Clear, r);

    // Build display labels: "entity_id (Name override)" if a name override is set
    let list_items: Vec<ListItem<'_>> = items
        .iter()
        .map(|(item_idx, eid)| {
            let label = dash
                .card(card_idx)
                .and_then(|c| {
                    if let crate::dashboard::CardKind::EntityList { entities, .. } = &c.kind {
                        entities.get(*item_idx).and_then(|e| {
                            if let crate::dashboard::EntityListItem::Full {
                                name: Some(n), ..
                            } = e
                            {
                                Some(format!("{eid} ({n})"))
                            } else {
                                None
                            }
                        })
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| eid.clone());
            ListItem::new(Line::raw(label))
        })
        .collect();

    let mut state = ListState::default();
    if !list_items.is_empty() {
        state.select(Some(selected.min(list_items.len() - 1)));
    }
    f.render_stateful_widget(
        List::new(list_items)
            .block(Block::bordered().title(" Pick entry to override (j/k · Enter · Esc) "))
            .highlight_style(Style::new().reversed())
            .highlight_symbol("▶ "),
        r,
        &mut state,
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
        .and_then(|d| d.card(card_idx))
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
        .card(card_idx)
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

fn draw_dashboard_picker(
    f: &mut Frame,
    area: Rect,
    app: &App,
    selected: usize,
    op: TransferOp,
    title: &str,
) {
    let _ = op;
    let r = modal_rect(area, 50, 14);
    f.render_widget(Clear, r);
    let items: Vec<ListItem<'_>> = app
        .dashboards
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let label = format!("  {}  {}", i + 1, d.name);
            ListItem::new(label)
        })
        .collect();
    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(Some(selected.min(items.len() - 1)));
    }
    let header = format!(" {} (j/k · Enter · Esc) ", title);
    f.render_stateful_widget(
        List::new(items)
            .block(Block::bordered().title(header.as_str()))
            .highlight_style(Style::new().reversed())
            .highlight_symbol("▶ "),
        r,
        &mut state,
    );
}

fn draw_grid_row_picker(
    f: &mut Frame,
    area: Rect,
    app: &App,
    target_dash: usize,
    selected: usize,
) {
    let r = modal_rect(area, 50, 14);
    f.render_widget(Clear, r);
    let items: Vec<ListItem<'_>> = app
        .dashboards
        .get(target_dash)
        .and_then(|d| if let DashboardLayout::Grid { rows } = &d.layout { Some(rows) } else { None })
        .map(|rows| {
            rows.iter()
                .enumerate()
                .map(|(i, row)| {
                    let h = match row.height {
                        crate::dashboard::RowHeight::Fixed(n) => format!("{}r", n),
                        crate::dashboard::RowHeight::Auto => "auto".into(),
                    };
                    ListItem::new(format!("  Row {}  [{} cols, height {}]", i + 1, row.columns.len(), h))
                })
                .collect()
        })
        .unwrap_or_default();
    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(Some(selected.min(items.len() - 1)));
    }
    f.render_stateful_widget(
        List::new(items)
            .block(Block::bordered().title(" Pick row (j/k · Enter · Esc) "))
            .highlight_style(Style::new().reversed())
            .highlight_symbol("▶ "),
        r,
        &mut state,
    );
}

fn draw_grid_col_picker(
    f: &mut Frame,
    area: Rect,
    app: &App,
    target_dash: usize,
    target_row: usize,
    selected: usize,
) {
    let r = modal_rect(area, 50, 14);
    f.render_widget(Clear, r);
    let items: Vec<ListItem<'_>> = app
        .dashboards
        .get(target_dash)
        .and_then(|d| if let DashboardLayout::Grid { rows } = &d.layout { rows.get(target_row) } else { None })
        .map(|row| {
            row.columns
                .iter()
                .enumerate()
                .map(|(i, col)| {
                    ListItem::new(format!("  Col {}  [{} card(s)]", i + 1, col.cards.len()))
                })
                .collect()
        })
        .unwrap_or_default();
    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(Some(selected.min(items.len() - 1)));
    }
    f.render_stateful_widget(
        List::new(items)
            .block(Block::bordered().title(" Pick column (j/k · Enter · Esc) "))
            .highlight_style(Style::new().reversed())
            .highlight_symbol("▶ "),
        r,
        &mut state,
    );
}
