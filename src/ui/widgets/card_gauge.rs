use ratatui::layout::Rect;
use ratatui::style::{Style, Stylize};
use ratatui::widgets::{Block, Gauge};
use ratatui::Frame;

use crate::ha::EntityState;
use crate::ui::theme::Theme;

pub fn render(
    f: &mut Frame,
    area: Rect,
    title: &str,
    instance: &str,
    state: Option<&EntityState>,
    min: f64,
    max: f64,
    unit: Option<&str>,
    theme: &Theme,
    selected: bool,
) {
    let color = theme.instance_color(instance);
    let value = state.and_then(|s| s.state.parse::<f64>().ok());
    let ratio = match value {
        Some(v) if max > min => ((v - min) / (max - min)).clamp(0.0, 1.0),
        _ => 0.0,
    };
    let label = match (value, unit) {
        (Some(v), Some(u)) => format!("{v:.1} {u}"),
        (Some(v), None) => format!("{v:.1}"),
        (None, _) => "—".into(),
    };
    let mut block = Block::bordered()
        .title(format!(" {title} "))
        .border_style(Style::new().fg(color));
    if selected {
        block = block.border_style(Style::new().fg(color).bold());
    }
    let gauge = Gauge::default()
        .block(block)
        .gauge_style(Style::new().fg(color))
        .ratio(ratio)
        .label(label);
    f.render_widget(gauge, area);
}
