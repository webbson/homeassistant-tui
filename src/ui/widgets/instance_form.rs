use ratatui::layout::{Constraint, Layout, Rect};
#[allow(unused_imports)]
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};
use ratatui::Frame;

use crate::screens::{InstanceFormField, InstanceFormMode, InstanceFormState};

fn modal_rect(area: Rect, width: u16, height: u16) -> Rect {
    let col = area.x + area.width.saturating_sub(width) / 2;
    let row = area.y + area.height.saturating_sub(height) / 2;
    Rect {
        x: col.min(area.right().saturating_sub(width)),
        y: row.min(area.bottom().saturating_sub(height)),
        width: width.min(area.width),
        height: height.min(area.height),
    }
}

pub fn render_instance_form(f: &mut Frame, area: Rect, state: &InstanceFormState) {
    let title = match &state.mode {
        InstanceFormMode::New => " Add Instance ",
        InstanceFormMode::Edit { .. } => " Edit Instance ",
    };

    let modal_h = 16u16;
    let modal_w = 60u16;
    let r = modal_rect(area, modal_w, modal_h);
    f.render_widget(Clear, r);

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(title);
    let inner = block.inner(r);
    f.render_widget(block, r);

    let [alias_label, alias_field, url_label, url_field, token_label, token_field, color_label, color_field, _gap, hint_row, error_row] =
        Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner)[..]
    else {
        return;
    };

    render_field(
        f,
        alias_label,
        alias_field,
        "Name",
        state.alias_buf.value(),
        state.focus == InstanceFormField::Alias,
        false,
    );
    render_field(
        f,
        url_label,
        url_field,
        "URL",
        state.url_buf.value(),
        state.focus == InstanceFormField::Url,
        false,
    );

    let token_display = if state.token_buf.value().is_empty() {
        match &state.mode {
            InstanceFormMode::Edit { .. } => "(unchanged)".to_string(),
            InstanceFormMode::New => String::new(),
        }
    } else {
        "•".repeat(state.token_buf.value().len())
    };
    render_field(
        f,
        token_label,
        token_field,
        "Token",
        &token_display,
        state.focus == InstanceFormField::Token,
        state.token_buf.value().is_empty() && matches!(&state.mode, InstanceFormMode::Edit { .. }),
    );

    render_field(
        f,
        color_label,
        color_field,
        "Color (opt)",
        state.color_buf.value(),
        state.focus == InstanceFormField::Color,
        false,
    );

    let hint = match &state.mode {
        InstanceFormMode::New => "Tab next field · Enter add · Esc cancel",
        InstanceFormMode::Edit { .. } => "Tab next field · Enter save · Esc cancel",
    };
    f.render_widget(Paragraph::new(hint).style(Style::new().dim()), hint_row);

    if let Some(err) = &state.error {
        f.render_widget(
            Paragraph::new(err.as_str()).style(Style::new().fg(Color::Red)),
            error_row,
        );
    }
}

fn render_field(
    f: &mut Frame,
    label_area: Rect,
    field_area: Rect,
    label: &str,
    value: &str,
    focused: bool,
    dimmed: bool,
) {
    let label_style = if focused {
        Style::new().bold()
    } else {
        Style::new().dim()
    };
    f.render_widget(Paragraph::new(label).style(label_style), label_area);

    let prefix = if focused { "▶ " } else { "  " };
    let value_style = if dimmed {
        Style::new().dim().italic()
    } else if focused {
        Style::new()
    } else {
        Style::new().dim()
    };
    let line = Line::from(vec![
        Span::raw(prefix),
        Span::styled(value, value_style),
        if focused {
            Span::styled("█", Style::new().dim())
        } else {
            Span::raw("")
        },
    ]);
    f.render_widget(Paragraph::new(line), field_area);
}

pub fn render_instance_delete_confirm(
    f: &mut Frame,
    area: Rect,
    alias: &str,
    affected_cards: usize,
    affected_dashboards: usize,
) {
    let modal_h = 8u16;
    let modal_w = 60u16;
    let r = modal_rect(area, modal_w, modal_h);
    f.render_widget(Clear, r);

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(" Confirm Delete ");
    let inner = block.inner(r);
    f.render_widget(block, r);

    let [msg1, msg2, _gap, hint] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(inner)[..] else {
        return;
    };

    f.render_widget(Paragraph::new(format!("Delete instance '{alias}'?")), msg1);

    let detail = if affected_cards == 0 {
        "No cards reference this instance.".to_string()
    } else {
        format!("Removes {affected_cards} card(s) from {affected_dashboards} dashboard(s).")
    };
    f.render_widget(
        Paragraph::new(detail).style(Style::new().fg(Color::Yellow)),
        msg2,
    );

    f.render_widget(
        Paragraph::new("y confirm · n/Esc cancel").style(Style::new().dim()),
        hint,
    );
}
