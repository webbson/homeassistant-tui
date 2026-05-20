use ratatui::layout::{Alignment, Rect};
#[allow(unused_imports)]
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::dashboard::CardSize;
use crate::ha::EntityState;
use crate::ui::theme::Theme;

#[allow(clippy::too_many_arguments)]
pub fn render(
    f: &mut Frame,
    area: Rect,
    title: &str,
    instance: &str,
    state: Option<&EntityState>,
    card_color: Option<&str>,
    theme: &Theme,
    selected: bool,
    ticker: bool,
    ticker_offset: usize,
    size: CardSize,
) {
    let color = crate::ui::theme::resolve_card_color(card_color, instance, theme);
    let mut block = Block::bordered()
        .title(format!(" {title} "))
        .border_style(Style::new().fg(color));
    if selected {
        block = block.border_style(Style::new().fg(color).bold());
    }

    let unit = state.map(crate::ui::format::unit_of).unwrap_or("");
    let value = state
        .map(|s| crate::ui::format::format_state(s, 1))
        .unwrap_or_default();
    let raw = if unit.is_empty() {
        value
    } else {
        format!("{value} {unit}")
    };

    // Inner area (inside the 1-char border on each side).
    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    if size == CardSize::Large && state.is_some() && super::big_text::fits(inner) {
        f.render_widget(block, area);
        super::big_text::render_big(f, inner, &raw, color);
        return;
    }

    let body = match state {
        None => Line::from(Span::styled("(unknown)", Style::new().fg(Color::DarkGray))),
        Some(_) => {
            // Reserve 2 chars padding on each side
            let usable = area.width.saturating_sub(4) as usize;
            let display = if ticker && raw.chars().count() > usable.max(1) {
                ticker_slice(&raw, ticker_offset, usable.max(1))
            } else {
                raw
            };
            Line::from(vec![Span::styled(display, Style::new().bold())])
        }
    };
    let p = Paragraph::new(body)
        .alignment(Alignment::Center)
        .block(block);
    f.render_widget(p, area);
}

/// Produce a window of `width` characters that scrolls across `text`,
/// with a 3-space gap separating loops so the marquee reads cleanly.
fn ticker_slice(text: &str, offset: usize, width: usize) -> String {
    let padded: String = format!("{text}   ");
    let chars: Vec<char> = padded.chars().collect();
    let len = chars.len();
    if len == 0 {
        return String::new();
    }
    let start = offset % len;
    let mut out = String::with_capacity(width);
    for i in 0..width {
        out.push(chars[(start + i) % len]);
    }
    out
}
