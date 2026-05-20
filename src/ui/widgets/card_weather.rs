use ratatui::layout::{Constraint, Layout, Rect};
#[allow(unused_imports)]
use ratatui::style::{Style, Stylize};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::dashboard::CardSize;
use crate::ha::{EntityState, ForecastDay};
use crate::ui::theme::{resolve_card_color, Theme};
use crate::ui::widgets::big_text;

#[allow(clippy::too_many_arguments)]
pub fn render(
    f: &mut Frame,
    area: Rect,
    title: &str,
    instance: &str,
    state: Option<&EntityState>,
    forecast: Option<&Vec<ForecastDay>>,
    show_forecast: bool,
    forecast_days: u8,
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

    // Extract current condition and temperature from entity state.
    let condition = state.map(|s| s.state.as_str()).unwrap_or("unavailable");
    let temp = attr_f64(state, "temperature");
    let humidity = attr_f64(state, "humidity");
    let wind_speed = attr_f64(state, "wind_speed");

    let glyph = condition_glyph(condition);

    match size {
        CardSize::Small => {
            // Small: single header line — glyph + temp + condition
            let text = if let Some(t) = temp {
                format!("{glyph}  {t:.1}°  {condition}")
            } else {
                format!("{glyph}  {condition}")
            };
            f.render_widget(Paragraph::new(text).style(Style::new().fg(color)), inner);
        }
        CardSize::Large => {
            // Large: oversized temperature via big-text + glyph + condition line.
            let temp_str = temp
                .map(|t| format!("{t:.0}°"))
                .unwrap_or_else(|| "—".into());

            let [glyph_area, temp_area] =
                Layout::vertical([Constraint::Length(2), Constraint::Fill(1)]).areas(inner);

            f.render_widget(
                Paragraph::new(format!("{glyph}  {condition}"))
                    .style(Style::new().fg(color))
                    .alignment(ratatui::layout::Alignment::Center),
                glyph_area,
            );

            if big_text::fits(temp_area) {
                big_text::render_big(f, temp_area, &temp_str, color);
            } else {
                f.render_widget(
                    Paragraph::new(temp_str)
                        .style(Style::new().fg(color))
                        .alignment(ratatui::layout::Alignment::Center),
                    temp_area,
                );
            }
        }
        CardSize::Normal => {
            // Normal: header line + attributes + forecast strip
            let header = if let Some(t) = temp {
                format!("{glyph}  {t:.1}°  {condition}")
            } else {
                format!("{glyph}  {condition}")
            };

            // Build attribute lines
            let mut attr_lines: Vec<String> = Vec::new();
            if let Some(h) = humidity {
                attr_lines.push(format!("  humidity  {h:.0}%"));
            }
            if let Some(w) = wind_speed {
                attr_lines.push(format!("  wind      {w:.1} km/h"));
            }

            // Forecast rows
            let forecast_rows: Vec<String> = if show_forecast {
                let days = forecast.map(|f| f.as_slice()).unwrap_or(&[]);
                let count = (forecast_days as usize).min(days.len());
                days[..count]
                    .iter()
                    .map(|d| {
                        let weekday = d.datetime.format("%a").to_string();
                        let g = condition_glyph(&d.condition);
                        let hi = format!("{:.0}°", d.temperature);
                        let lo = d
                            .templow
                            .map(|t| format!("{t:.0}°"))
                            .unwrap_or_else(|| "—".into());
                        format!("  {weekday:<3}  {g}  {hi} / {lo}")
                    })
                    .collect()
            } else {
                Vec::new()
            };

            // Layout: 1 header + attr_lines + forecast_rows, all Length(1) except header Fill(1)
            let attr_count = attr_lines.len() as u16;
            let fc_count = forecast_rows.len() as u16;
            let total_fixed = attr_count + fc_count;

            let constraints: Vec<Constraint> = std::iter::once(Constraint::Fill(1))
                .chain(std::iter::repeat_n(
                    Constraint::Length(1),
                    total_fixed as usize,
                ))
                .collect();

            let areas = Layout::vertical(constraints).split(inner);

            f.render_widget(
                Paragraph::new(header).style(Style::new().fg(color)),
                areas[0],
            );

            let mut idx = 1usize;
            for line in &attr_lines {
                if idx < areas.len() {
                    f.render_widget(
                        Paragraph::new(line.as_str()).style(Style::new().dim()),
                        areas[idx],
                    );
                }
                idx += 1;
            }
            for line in &forecast_rows {
                if idx < areas.len() {
                    f.render_widget(
                        Paragraph::new(line.as_str()).style(Style::new().fg(color).dim()),
                        areas[idx],
                    );
                }
                idx += 1;
            }
        }
    }
}

fn condition_glyph(condition: &str) -> &'static str {
    let c = condition.to_ascii_lowercase();
    if c.contains("sunny") || c.contains("clear") {
        "☀"
    } else if c.contains("partlycloudy") || c.contains("partly") {
        "⛅"
    } else if c.contains("cloudy") || c.contains("overcast") {
        "☁"
    } else if c.contains("rainy") || c.contains("rain") || c.contains("drizzle") {
        "🌧"
    } else if c.contains("snowy") || c.contains("snow") {
        "❄"
    } else if c.contains("fog") || c.contains("haze") || c.contains("mist") {
        "🌫"
    } else if c.contains("lightning") || c.contains("thunder") || c.contains("storm") {
        "⛈"
    } else {
        "~"
    }
}

fn attr_f64(state: Option<&EntityState>, key: &str) -> Option<f64> {
    state?.attributes.get(key).and_then(|v| v.as_f64())
}
