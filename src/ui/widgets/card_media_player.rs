use ratatui::layout::{Constraint, Layout, Rect};
#[allow(unused_imports)]
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Gauge, Paragraph};
use ratatui::Frame;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{Resize, StatefulImage};

use crate::dashboard::CardSize;
use crate::ui::theme::{resolve_card_color, Theme};

/// Unified view model consumed by the render fn.
/// Built by `screens/dashboard.rs` from either HA entity state or local media snapshot.
pub struct MediaPlayerView {
    pub player_state: String,
    pub media_title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub position_secs: Option<f64>,
    pub duration_secs: Option<f64>,
    pub volume_0_1: Option<f64>,
    pub is_muted: bool,
    pub show_cover: bool,
    pub show_volume: bool,
    pub show_progress: bool,
}

#[allow(clippy::too_many_arguments)]
pub fn render(
    f: &mut Frame,
    area: Rect,
    title: &str,
    instance_or_source: &str,
    view: &mut MediaPlayerView,
    cover: Option<&mut StatefulProtocol>,
    card_color: Option<&str>,
    size: CardSize,
    theme: &Theme,
    selected: bool,
) {
    let color = resolve_card_color(card_color, instance_or_source, theme);
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

    if inner.height < 4 {
        render_small(f, inner, view, color);
        return;
    }

    // Horizontal split: main content | optional volume bar
    let [content_area, vol_area] = Layout::horizontal([
        Constraint::Min(0),
        Constraint::Length(if view.show_volume && inner.width > 4 {
            1
        } else {
            0
        }),
    ])
    .areas(inner);

    render_content(f, content_area, view, cover, color, selected, size);

    if view.show_volume && vol_area.width > 0 {
        render_volume_bar(f, vol_area, view.volume_0_1, view.is_muted, color);
    }
}

fn render_small(f: &mut Frame, area: Rect, view: &MediaPlayerView, color: ratatui::style::Color) {
    let track = view.media_title.as_deref().unwrap_or("—");
    let state = view.player_state.as_str();
    if area.height < 2 {
        f.render_widget(Paragraph::new(track).style(Style::new().fg(color)), area);
        return;
    }
    let [title_row, state_row] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);
    f.render_widget(
        Paragraph::new(track).style(Style::new().fg(color)),
        title_row,
    );
    f.render_widget(
        Paragraph::new(format!("[{state}]")).style(Style::new().dim()),
        state_row,
    );
}

fn render_content(
    f: &mut Frame,
    area: Rect,
    view: &mut MediaPlayerView,
    cover: Option<&mut StatefulProtocol>,
    color: ratatui::style::Color,
    selected: bool,
    size: CardSize,
) {
    // Reserve rows for the fixed-height info section
    let info_rows: u16 = 1  // track title
        + 1                  // artist / album
        + 1                  // controls
        + if view.show_progress { 1 } else { 0 };

    let cover_rows = if view.show_cover && cover.is_some() {
        area.height.saturating_sub(info_rows)
    } else {
        0
    };

    let constraints = if cover_rows > 0 {
        vec![
            Constraint::Length(cover_rows),
            Constraint::Length(info_rows),
        ]
    } else {
        vec![Constraint::Length(0), Constraint::Min(0)]
    };

    let [cover_area, info_area] = {
        let areas = Layout::vertical(constraints).split(area);
        [areas[0], areas[1]]
    };

    if cover_rows > 0 {
        if let Some(protocol) = cover {
            // size controls what fraction of card width the cover occupies:
            //   Large = 100%, Normal = 50%, Small = 25%
            let frac_num: u16 = match size {
                CardSize::Large => 4,
                CardSize::Normal => 2,
                CardSize::Small => 1,
            };
            let max_cover_w = (cover_area.width.saturating_mul(frac_num) / 4).max(1);
            // cap height to aspect ratio so we don't get a tall narrow strip
            let art_h = (max_cover_w / 2).min(cover_area.height).max(1);
            let art_w = (art_h.saturating_mul(2)).min(max_cover_w).max(1);
            // centre within cover_area
            let cx = cover_area.x + (cover_area.width.saturating_sub(art_w)) / 2;
            let cy = cover_area.y + (cover_area.height.saturating_sub(art_h)) / 2;
            let art_area = Rect {
                x: cx,
                y: cy,
                width: art_w,
                height: art_h,
            };
            f.render_stateful_widget(
                StatefulImage::new().resize(Resize::Scale(None)),
                art_area,
                protocol,
            );
        }
    }

    // Info rows
    let progress_constraint = if view.show_progress {
        Constraint::Length(1)
    } else {
        Constraint::Length(0)
    };
    let [title_area, artist_area, controls_area, progress_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        progress_constraint,
    ])
    .areas(info_area);

    // Track title
    let track = view.media_title.as_deref().unwrap_or("—").to_string();
    f.render_widget(
        Paragraph::new(track)
            .centered()
            .style(Style::new().fg(color).bold()),
        title_area,
    );

    // Artist (Album)
    let artist_album = format_artist_album(view.artist.as_deref(), view.album.as_deref());
    f.render_widget(
        Paragraph::new(artist_album)
            .centered()
            .style(Style::new().dim()),
        artist_area,
    );

    // Controls row: glyphs + key hints when selected
    let play_glyph = if view.player_state == "playing" {
        "⏸"
    } else {
        "⏵"
    };
    let controls = if selected {
        format!("[p]⏮  {play_glyph}[⎵]  ⏭[n]")
    } else {
        format!("⏮  {play_glyph}  ⏭")
    };
    let ctrl_style = if selected {
        Style::new().fg(color).bold()
    } else {
        Style::new().fg(color).dim()
    };
    f.render_widget(
        Paragraph::new(controls).centered().style(ctrl_style),
        controls_area,
    );

    // Progress bar
    if view.show_progress {
        if let (Some(pos), Some(dur)) = (view.position_secs, view.duration_secs) {
            if dur > 0.0 {
                let ratio = (pos / dur).clamp(0.0, 1.0);
                let label = format!("{} / {}", fmt_duration(pos), fmt_duration(dur));
                f.render_widget(
                    Gauge::default()
                        .ratio(ratio)
                        .label(label)
                        .gauge_style(Style::new().fg(color)),
                    progress_area,
                );
                return;
            }
        }
        // No duration data: show state
        f.render_widget(
            Paragraph::new(format!("[{}]", view.player_state)).style(Style::new().dim()),
            progress_area,
        );
    }
}

fn render_volume_bar(
    f: &mut Frame,
    area: Rect,
    volume: Option<f64>,
    muted: bool,
    color: ratatui::style::Color,
) {
    let vol = volume.unwrap_or(0.0).clamp(0.0, 1.0);
    let height = area.height;
    if height == 0 {
        return;
    }

    if muted {
        let mid = height / 2;
        let lines: Vec<Line> = (0..height)
            .map(|row| {
                if row == mid {
                    Line::styled("M", Style::new().fg(color).dim())
                } else {
                    Line::styled("░", Style::new().dim())
                }
            })
            .collect();
        f.render_widget(Paragraph::new(Text::from(lines)), area);
        return;
    }

    let filled = (vol * height as f64).round() as u16;
    let lines: Vec<Line> = (0..height)
        .map(|row| {
            // row 0 = top; filled rows are at the bottom
            if row >= height.saturating_sub(filled) {
                Line::styled("█", Style::new().fg(color))
            } else {
                Line::styled("░", Style::new().dim())
            }
        })
        .collect();
    f.render_widget(Paragraph::new(Text::from(lines)), area);
}

fn format_artist_album(artist: Option<&str>, album: Option<&str>) -> String {
    match (artist, album) {
        (Some(a), Some(al)) => format!("{a} ({al})"),
        (Some(a), None) => a.to_string(),
        (None, Some(al)) => format!("({al})"),
        (None, None) => String::new(),
    }
}

fn fmt_duration(secs: f64) -> String {
    let s = secs as u64;
    let m = s / 60;
    let s = s % 60;
    format!("{m}:{s:02}")
}
