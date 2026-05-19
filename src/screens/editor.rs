use ratatui::layout::Rect;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};
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

    // Top title strip
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

    // Render existing cards (re-use dashboard render path).
    crate::screens::dashboard::draw(f, body_rect, app, editor.dash_idx, usize::MAX);

    // Cursor cell
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

    // Selection ring
    if let Some(i) = editor.selected_card {
        if let Some(card) = dash.cards.get(i) {
            let r = cell_to_rect(body_rect, dash.grid, card.pos);
            f.render_widget(
                Block::bordered().border_style(Style::new().fg(Color::Yellow).bold()),
                r,
            );
        }
    }

    // Modal overlays
    match &editor.mode {
        EditorMode::PickingType => draw_palette(f, area),
        EditorMode::Entering { card_type, buffer } => {
            draw_input(f, area, *card_type, buffer);
        }
        EditorMode::ConfirmExit => draw_confirm(f, area, "Unsaved changes. Discard? (y/n)"),
        EditorMode::ConfirmDelete => draw_confirm(f, area, "Delete selected card? (y/n)"),
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
        Paragraph::new(lines).block(Block::bordered().title(" Add card ")),
        r,
    );
}

fn draw_input(f: &mut Frame, area: Rect, kind: CardTypeStub, buffer: &str) {
    let r = modal_rect(area, 60, 5);
    f.render_widget(Clear, r);
    let prompt = match kind {
        CardTypeStub::Text => "Text body (Enter=accept, Esc=cancel):",
        _ => {
            "Entity ID (e.g. light.kitchen) [optionally prefix with instance: home/light.kitchen]:"
        }
    };
    let body = format!("{prompt}\n> {buffer}_");
    let title = format!(" New {} card ", kind.label());
    f.render_widget(
        Paragraph::new(body).block(Block::bordered().title(title)),
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
