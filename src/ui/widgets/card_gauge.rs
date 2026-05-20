use ratatui::layout::Rect;
#[allow(unused_imports)]
use ratatui::style::{Style, Stylize};
use ratatui::widgets::{Block, Gauge};
use ratatui::Frame;

use crate::ha::EntityState;
use crate::ui::theme::Theme;

#[allow(clippy::too_many_arguments)]
pub fn render(
    f: &mut Frame,
    area: Rect,
    title: &str,
    instance: &str,
    state: Option<&EntityState>,
    min: f64,
    max: f64,
    unit: Option<&str>,
    card_color: Option<&str>,
    theme: &Theme,
    selected: bool,
) {
    let color = crate::ui::theme::resolve_card_color(card_color, instance, theme);
    let value = state.and_then(|s| s.state.parse::<f64>().ok());
    let ratio = match value {
        Some(v) if max > min => ((v - min) / (max - min)).clamp(0.0, 1.0),
        _ => 0.0,
    };
    let formatted = state.map(|s| crate::ui::format::format_state(s, 1));
    let entity_unit = state.map(crate::ui::format::unit_of).unwrap_or("");
    let unit_str = unit.unwrap_or(entity_unit);
    let label = match formatted {
        Some(v) if !unit_str.is_empty() => format!("{v} {unit_str}"),
        Some(v) => v,
        None => "—".into(),
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
