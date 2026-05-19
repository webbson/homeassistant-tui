use ratatui::layout::Rect;
#[allow(unused_imports)]
use ratatui::style::{Color, Style, Stylize};
use ratatui::symbols;
use ratatui::text::Span;
use ratatui::widgets::{Axis, Block, Chart, Dataset, GraphType, Paragraph};
use ratatui::Frame;

use crate::ui::theme::Theme;
use crate::util::history::RingBuf;

#[allow(clippy::too_many_arguments)]
pub fn render(
    f: &mut Frame,
    area: Rect,
    title: &str,
    instance: &str,
    history: Option<&RingBuf>,
    window: &str,
    theme: &Theme,
    selected: bool,
) {
    let color = theme.instance_color(instance);
    let mut block = Block::bordered()
        .title(format!(" {title} "))
        .border_style(Style::new().fg(color));
    if selected {
        block = block.border_style(Style::new().fg(color).bold());
    }

    let values: Vec<f64> = history.map(|h| h.values().collect()).unwrap_or_default();
    if values.len() < 2 {
        f.render_widget(
            Paragraph::new("(no history yet)")
                .style(Style::new().fg(Color::DarkGray))
                .block(block),
            area,
        );
        return;
    }

    // Downsample to inner width so the line always fills the card.
    let inner_w = area.width.saturating_sub(8).max(4) as usize;
    let downsampled = downsample(&values, inner_w);
    let points: Vec<(f64, f64)> = downsampled
        .iter()
        .enumerate()
        .map(|(i, v)| (i as f64, *v))
        .collect();
    let n = (points.len() as f64 - 1.0).max(1.0);
    let (lo, hi) = points
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), (_, v)| {
            (lo.min(*v), hi.max(*v))
        });
    let pad = ((hi - lo).abs() * 0.05).max(0.5);
    let y_lo = lo - pad;
    let y_hi = hi + pad;

    let dataset = Dataset::default()
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::new().fg(color))
        .data(&points);

    let chart = Chart::new(vec![dataset])
        .block(block)
        .x_axis(
            Axis::default()
                .style(Style::new().fg(Color::DarkGray))
                .bounds([0.0, n])
                .labels(vec![
                    Span::styled(format!("-{}", window), Style::new().fg(Color::DarkGray)),
                    Span::styled("now", Style::new().fg(Color::DarkGray)),
                ]),
        )
        .y_axis(
            Axis::default()
                .style(Style::new().fg(Color::DarkGray))
                .bounds([y_lo, y_hi])
                .labels(vec![
                    Span::styled(format!("{lo:.1}"), Style::new().fg(Color::DarkGray)),
                    Span::styled(format!("{hi:.1}"), Style::new().fg(Color::DarkGray)),
                ]),
        );

    f.render_widget(chart, area);
}

fn downsample(values: &[f64], target: usize) -> Vec<f64> {
    if values.len() <= target || target == 0 {
        return values.to_vec();
    }
    let mut out = Vec::with_capacity(target);
    let step = values.len() as f64 / target as f64;
    for i in 0..target {
        let start = (i as f64 * step) as usize;
        let end = ((i + 1) as f64 * step) as usize;
        let end = end.min(values.len()).max(start + 1);
        let slice = &values[start..end];
        let avg = slice.iter().sum::<f64>() / slice.len() as f64;
        out.push(avg);
    }
    out
}
