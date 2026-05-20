use ratatui::layout::Rect;
#[allow(unused_imports)]
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::dashboard::{CardSize, Severity};
use crate::ui::theme::Theme;

#[allow(clippy::too_many_arguments)]
pub fn render(
    f: &mut Frame,
    area: Rect,
    title: &str,
    instance: &str,
    value: Option<f64>,
    min: f64,
    max: f64,
    unit: Option<&str>,
    severity: Option<&Severity>,
    needle: bool,
    card_color: Option<&str>,
    size: CardSize,
    theme: &Theme,
    selected: bool,
) {
    let base_color = crate::ui::theme::resolve_card_color(card_color, instance, theme);
    let mut block = Block::bordered()
        .title(format!(" {title} "))
        .border_style(Style::new().fg(base_color));
    if selected {
        block = block.border_style(Style::new().fg(base_color).bold());
    }

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height == 0 {
        return;
    }

    // Determine the needle position (cell index within the arc).
    let arc_width = inner.width as usize;
    let needle_cell: Option<usize> = if needle && value.is_some() && max > min {
        let v = value.unwrap().clamp(min, max);
        let pos = ((v - min) / (max - min) * (arc_width.saturating_sub(1) as f64)).round() as usize;
        Some(pos.min(arc_width.saturating_sub(1)))
    } else {
        None
    };

    // Build the arc row as styled spans.
    let arc_spans: Vec<Span<'static>> = (0..arc_width)
        .map(|i| {
            // Map this cell back to a value in [min, max].
            let cell_val = if arc_width <= 1 {
                min
            } else {
                min + (i as f64 / (arc_width - 1) as f64) * (max - min)
            };

            let cell_color = match severity {
                Some(s) => {
                    if cell_val >= s.red {
                        Color::Red
                    } else if cell_val >= s.yellow {
                        Color::Yellow
                    } else {
                        // green threshold reserved; only yellow/red discriminate
                        Color::Green
                    }
                }
                None => base_color,
            };

            if needle_cell == Some(i) {
                // Needle: inverted — black glyph on the cell colour background.
                Span::styled("█", Style::new().fg(Color::Black).bg(cell_color))
            } else {
                Span::styled("█", Style::new().fg(cell_color))
            }
        })
        .collect();

    // Arc occupies the first row of inner.
    let arc_rect = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    };
    f.render_widget(Paragraph::new(Line::from(arc_spans)), arc_rect);

    // Value label below the arc.
    let label_area = Rect {
        x: inner.x,
        y: inner.y + 1,
        width: inner.width,
        height: inner.height.saturating_sub(1),
    };
    if label_area.height == 0 {
        return;
    }

    let label_str: String = match value {
        Some(v) => match unit {
            Some(u) if !u.is_empty() => format!("{v:.1} {u}"),
            _ => format!("{v:.1}"),
        },
        None => "—".into(),
    };

    if size == CardSize::Large && crate::ui::widgets::big_text::fits(label_area) {
        crate::ui::widgets::big_text::render_big(f, label_area, &label_str, base_color);
    } else {
        f.render_widget(
            Paragraph::new(label_str).style(Style::new().fg(base_color)),
            label_area,
        );
    }
}
