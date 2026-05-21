use ratatui::layout::{Constraint, Layout, Rect};
#[allow(unused_imports)]
use ratatui::style::{Color, Style, Stylize};
use ratatui::widgets::{Block, Gauge, Paragraph};
use ratatui::Frame;

use crate::dashboard::CardSize;
use crate::ha::EntityState;
use crate::ui::theme::{resolve_card_color, Theme};
use crate::ui::widgets::big_text;

#[allow(clippy::too_many_arguments)]
pub fn render(
    f: &mut Frame,
    area: Rect,
    title: &str,
    instance: &str,
    state: Option<&EntityState>,
    card_color: Option<&str>,
    size: CardSize,
    theme: &Theme,
    selected: bool,
) {
    let color = resolve_card_color(card_color, instance, theme);
    let border_style = if selected {
        Style::new().fg(color).bold()
    } else {
        Style::new().fg(color)
    };
    let block = Block::bordered()
        .title(format!(" {title} "))
        .border_style(border_style);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let player_state = state.map(|s| s.state.as_str()).unwrap_or("unavailable");

    if matches!(size, CardSize::Small) {
        let media_title = attr_str(state, "media_title").unwrap_or_else(|| "—".into());
        let [title_row, state_row] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(inner);
        f.render_widget(
            Paragraph::new(media_title).style(Style::new().fg(color)),
            title_row,
        );
        f.render_widget(
            Paragraph::new(format!("[{player_state}]")).style(Style::new().dim()),
            state_row,
        );
        return;
    }

    let media_title = attr_str(state, "media_title").unwrap_or_else(|| "—".into());
    let media_artist = attr_str(state, "media_artist").unwrap_or_default();
    let app_name = attr_str(state, "app_name").unwrap_or_default();
    let volume = attr_f64(state, "volume_level").unwrap_or(0.0);
    let muted = attr_bool(state, "is_volume_muted").unwrap_or(false);

    let [title_row, artist_row, app_row, gauge_row, state_row] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(inner);

    if matches!(size, CardSize::Large) && big_text::fits(title_row) {
        big_text::render_big(f, title_row, &media_title, Style::new().fg(color));
    } else {
        f.render_widget(
            Paragraph::new(media_title).style(Style::new().fg(color)),
            title_row,
        );
    }
    f.render_widget(
        Paragraph::new(media_artist).style(Style::new().dim()),
        artist_row,
    );
    f.render_widget(Paragraph::new(app_name).style(Style::new().dim()), app_row);
    let vol_label = if muted {
        "muted".to_string()
    } else {
        format!("vol {:.0}%", volume * 100.0)
    };
    f.render_widget(
        Gauge::default()
            .ratio(if muted { 0.0 } else { volume.clamp(0.0, 1.0) })
            .label(vol_label)
            .gauge_style(Style::new().fg(color)),
        gauge_row,
    );
    f.render_widget(
        Paragraph::new(format!("[{player_state}]")).style(Style::new().dim()),
        state_row,
    );
}

fn attr_str(s: Option<&EntityState>, k: &str) -> Option<String> {
    s?.attributes.get(k)?.as_str().map(|v| v.to_string())
}

fn attr_f64(s: Option<&EntityState>, k: &str) -> Option<f64> {
    s?.attributes.get(k)?.as_f64()
}

fn attr_bool(s: Option<&EntityState>, k: &str) -> Option<bool> {
    s?.attributes.get(k)?.as_bool()
}
