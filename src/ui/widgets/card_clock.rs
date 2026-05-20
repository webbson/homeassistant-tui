use chrono::Local;
use ratatui::layout::{Alignment, Rect};
#[allow(unused_imports)]
use ratatui::style::Stylize;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::dashboard::CardSize;

#[allow(clippy::too_many_arguments)]
pub fn render(
    f: &mut Frame,
    area: Rect,
    title: &str,
    format: &str,
    timezone: Option<&str>,
    card_color: Option<&str>,
    size: CardSize,
    selected: bool,
) {
    let color = card_color
        .and_then(crate::ui::theme::parse_color)
        .unwrap_or(Color::Cyan);

    let mut block = Block::bordered()
        .title(format!(" {title} "))
        .border_style(Style::new().fg(color));
    if selected {
        block = block.border_style(Style::new().fg(color).bold());
    }

    let time_str = format_time(format, timezone);

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Large size + fits: use big-text renderer inside inner area.
    if size == CardSize::Large && crate::ui::widgets::big_text::fits(inner) {
        crate::ui::widgets::big_text::render_big(f, inner, &time_str, color);
        return;
    }

    f.render_widget(
        Paragraph::new(time_str)
            .style(Style::new().fg(color).bold())
            .alignment(Alignment::Center),
        inner,
    );
}

fn format_time(fmt: &str, timezone: Option<&str>) -> String {
    if let Some(tz_name) = timezone {
        if let Ok(tz) = tz_name.parse::<chrono_tz::Tz>() {
            let now = chrono::Utc::now().with_timezone(&tz);
            return now.format(fmt).to_string();
        }
    }
    Local::now().format(fmt).to_string()
}
