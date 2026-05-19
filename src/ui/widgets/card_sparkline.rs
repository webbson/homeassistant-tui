use ratatui::layout::Rect;
use ratatui::style::{Style, Stylize};
use ratatui::widgets::{Block, Sparkline};
use ratatui::Frame;

use crate::ui::theme::Theme;
use crate::util::history::RingBuf;

pub fn render(
    f: &mut Frame,
    area: Rect,
    title: &str,
    instance: &str,
    history: Option<&RingBuf>,
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
    let data: Vec<u64> = match history {
        Some(h) if !h.is_empty() => {
            let vs: Vec<f64> = h.values().collect();
            let lo = vs.iter().cloned().fold(f64::INFINITY, f64::min);
            let hi = vs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let span = (hi - lo).max(1e-9);
            vs.iter()
                .map(|v| (((v - lo) / span) * 100.0) as u64)
                .collect()
        }
        _ => vec![],
    };
    let sparkline = Sparkline::default()
        .block(block)
        .data(data)
        .style(Style::new().fg(color));
    f.render_widget(sparkline, area);
}
