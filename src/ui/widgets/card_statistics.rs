use ratatui::layout::{Alignment, Constraint, Layout, Rect};
#[allow(unused_imports)]
use ratatui::style::{Style, Stylize};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::dashboard::{CardSize, StatsMetric};
use crate::ui::theme::resolve_card_color;
use crate::ui::widgets::big_text;
use crate::util::history::RingBuf;

#[allow(clippy::too_many_arguments)]
pub fn render(
    f: &mut Frame,
    area: Rect,
    title: &str,
    instance: &str,
    history: Option<&RingBuf>,
    metric: StatsMetric,
    window: &str,
    unit: Option<&str>,
    card_color: Option<&str>,
    size: CardSize,
    theme: &crate::ui::theme::Theme,
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

    let value = history.and_then(|h| h.compute(metric));
    let text = match (value, unit) {
        (Some(v), Some(u)) => format!("{v:.2} {u}"),
        (Some(v), None) => format!("{v:.2}"),
        (None, _) => "\u{2014}".into(), // em dash
    };
    let subtitle = format!("{} \u{00b7} {}", metric_label(metric), window);

    let [value_area, label_area] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(inner);

    if matches!(size, CardSize::Large) && big_text::fits(value_area) {
        big_text::render_big(f, value_area, &text, Style::new().fg(color));
    } else {
        f.render_widget(
            Paragraph::new(text)
                .style(Style::new().fg(color))
                .alignment(Alignment::Center),
            value_area,
        );
    }
    f.render_widget(
        Paragraph::new(subtitle)
            .style(Style::new().dim())
            .alignment(Alignment::Center),
        label_area,
    );
}

fn metric_label(m: StatsMetric) -> &'static str {
    match m {
        StatsMetric::Avg => "avg",
        StatsMetric::Min => "min",
        StatsMetric::Max => "max",
        StatsMetric::Sum => "sum",
        StatsMetric::Count => "count",
    }
}
