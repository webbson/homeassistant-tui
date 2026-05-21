use ratatui::layout::Rect;
#[allow(unused_imports)]
use ratatui::style::{Color, Style, Stylize};
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Axis, Bar, BarChart, Block, Chart, Dataset, GraphType, Paragraph};
use ratatui::Frame;

use crate::dashboard::{BarOrientation, GraphSeries};
use crate::ha::{EntityId, EntityState};
use crate::ui::theme::Theme;
use crate::util::history::RingBuf;

pub struct GraphRender<'a> {
    pub area: Rect,
    pub title: &'a str,
    pub instance: &'a str,
    pub series: &'a [GraphSeries],
    /// (entity, history) tuples in the same order as `series`.
    pub histories: &'a [(EntityId, Option<&'a RingBuf>)],
    /// Current state for each series (same order as `series`). `None` if unavailable.
    pub current_states: &'a [Option<&'a EntityState>],
    pub window: &'a str,
    pub card_color: Option<&'a str>,
    pub theme: &'a Theme,
    pub selected: bool,
}

fn make_block<'a>(title: &'a str, color: Color, selected: bool) -> Block<'a> {
    let border_style = if selected {
        Style::new().fg(color).bold()
    } else {
        Style::new().fg(color)
    };
    Block::bordered()
        .title(format!(" {title} "))
        .border_style(border_style)
}

fn series_color(
    series: &GraphSeries,
    card_color: Option<&str>,
    instance: &str,
    theme: &Theme,
) -> Color {
    crate::ui::theme::resolve_card_color(series.color.as_deref().or(card_color), instance, theme)
}

fn short_name(entity: &EntityId) -> &str {
    let s = entity.as_str();
    s.rfind('.').map(|i| &s[i + 1..]).unwrap_or(s)
}

fn series_label(s: &GraphSeries) -> &str {
    s.label.as_deref().unwrap_or_else(|| short_name(&s.entity))
}

fn legend_value_str(state: Option<&EntityState>) -> String {
    match state {
        None => "\u{2014}".into(), // em dash
        Some(s) => {
            let val = crate::ui::format::format_state(s, 1);
            let unit = crate::ui::format::unit_of(s);
            if unit.is_empty() {
                val
            } else {
                format!("{val} {unit}")
            }
        }
    }
}

pub fn render_line(f: &mut Frame, args: GraphRender<'_>) {
    let card_color =
        crate::ui::theme::resolve_card_color(args.card_color, args.instance, args.theme);
    // Compute inner area before any block is moved.
    let inner = {
        let tmp = make_block(args.title, card_color, args.selected);
        tmp.inner(args.area)
    };

    struct SeriesData {
        color: Color,
        label: String,
        points: Vec<(f64, f64)>,
        orig_idx: usize,
    }

    let inner_w = inner.width.saturating_sub(4).max(4) as usize;
    let mut all_series: Vec<SeriesData> = Vec::new();
    for (i, s) in args.series.iter().enumerate() {
        let history_opt = args.histories.get(i).and_then(|(_, h)| *h);
        let values: Vec<f64> = history_opt
            .map(|h| h.values().collect())
            .unwrap_or_default();
        if values.len() < 2 {
            continue;
        }
        let downsampled = downsample(&values, inner_w);
        let points: Vec<(f64, f64)> = downsampled
            .iter()
            .enumerate()
            .map(|(idx, v)| (idx as f64, *v))
            .collect();
        all_series.push(SeriesData {
            color: series_color(s, args.card_color, args.instance, args.theme),
            label: series_label(s).to_string(),
            points,
            orig_idx: i,
        });
    }

    if all_series.is_empty() {
        let block = make_block(args.title, card_color, args.selected);
        f.render_widget(
            Paragraph::new("(no history yet)")
                .style(Style::new().dim())
                .block(block),
            args.area,
        );
        return;
    }

    // Y range across all series.
    let (lo, hi) = all_series
        .iter()
        .flat_map(|s| s.points.iter())
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), (_, v)| {
            (lo.min(*v), hi.max(*v))
        });
    let pad = ((hi - lo).abs() * 0.05).max(0.5);
    let y_lo = lo - pad;
    let y_hi = hi + pad;

    let max_n = all_series.iter().map(|s| s.points.len()).max().unwrap_or(1);
    let x_max = (max_n as f64 - 1.0).max(1.0);

    // Show legend for any non-empty series (including single), but only when
    // the card is tall enough that the chart still gets at least 3 rows for axis labels.
    let potential_legend_rows = all_series.len() as u16;
    let show_legend = !all_series.is_empty()
        && inner.height > potential_legend_rows + 2;
    let legend_rows = if show_legend { potential_legend_rows } else { 0 };
    let chart_height = inner.height.saturating_sub(legend_rows).max(1);

    let window = args.window;

    if show_legend {
        // Render block separately, then chart (no block) in upper sub-area.
        let block = make_block(args.title, card_color, args.selected);
        let chart_area = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: chart_height,
        };
        let legend_area = Rect {
            x: inner.x,
            y: inner.y + chart_height,
            width: inner.width,
            height: legend_rows,
        };
        f.render_widget(block, args.area);

        let datasets: Vec<Dataset> = all_series
            .iter()
            .map(|s| {
                Dataset::default()
                    .marker(symbols::Marker::Braille)
                    .graph_type(GraphType::Line)
                    .style(Style::new().fg(s.color))
                    .data(&s.points)
            })
            .collect();
        let chart = Chart::new(datasets)
            .x_axis(
                Axis::default()
                    .style(Style::new().dim())
                    .bounds([0.0, x_max])
                    .labels(vec![
                        Span::styled(format!("-{window}"), Style::new().dim()),
                        Span::styled("now", Style::new().dim()),
                    ]),
            )
            .y_axis(
                Axis::default()
                    .style(Style::new().dim())
                    .bounds([y_lo, y_hi])
                    .labels(vec![
                        Span::styled(format!("{lo:.1}"), Style::new().dim()),
                        Span::styled(format!("{hi:.1}"), Style::new().dim()),
                    ]),
            );
        f.render_widget(chart, chart_area);

        for (i, s) in all_series.iter().enumerate() {
            if i as u16 >= legend_rows {
                break;
            }
            let row = Rect {
                x: legend_area.x,
                y: legend_area.y + i as u16,
                width: legend_area.width,
                height: 1,
            };
            let swatch = Span::styled("█ ", Style::new().fg(s.color));
            let label = Span::styled(s.label.as_str(), Style::new());
            let val_str = legend_value_str(args.current_states.get(s.orig_idx).and_then(|o| *o));
            // Only append the value if there is enough room (swatch=2, space=2, label, space, value).
            let min_width = 2 + 2 + s.label.len() + 1 + val_str.len();
            let spans = if (row.width as usize) >= min_width {
                let val_span = Span::styled(
                    format!("  {val_str}"),
                    Style::new().dim(),
                );
                vec![swatch, label, val_span]
            } else {
                vec![swatch, label]
            };
            f.render_widget(Paragraph::new(Line::from(spans)), row);
        }
    } else {
        let block = make_block(args.title, card_color, args.selected);
        let datasets: Vec<Dataset> = all_series
            .iter()
            .map(|s| {
                Dataset::default()
                    .marker(symbols::Marker::Braille)
                    .graph_type(GraphType::Line)
                    .style(Style::new().fg(s.color))
                    .data(&s.points)
            })
            .collect();
        let chart = Chart::new(datasets)
            .block(block)
            .x_axis(
                Axis::default()
                    .style(Style::new().dim())
                    .bounds([0.0, x_max])
                    .labels(vec![
                        Span::styled(format!("-{window}"), Style::new().dim()),
                        Span::styled("now", Style::new().dim()),
                    ]),
            )
            .y_axis(
                Axis::default()
                    .style(Style::new().dim())
                    .bounds([y_lo, y_hi])
                    .labels(vec![
                        Span::styled(format!("{lo:.1}"), Style::new().dim()),
                        Span::styled(format!("{hi:.1}"), Style::new().dim()),
                    ]),
            );
        f.render_widget(chart, args.area);
    }
}

pub fn render_bar(
    f: &mut Frame,
    args: GraphRender<'_>,
    orientation: BarOrientation,
    current: &[(EntityId, Option<f64>)],
) {
    let card_color =
        crate::ui::theme::resolve_card_color(args.card_color, args.instance, args.theme);
    let block = make_block(args.title, card_color, args.selected);
    let inner = block.inner(args.area);
    let n = args.series.len().max(1) as u16;
    let gap = 1u16;

    let bars: Vec<Bar> = args
        .series
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let color = series_color(s, args.card_color, args.instance, args.theme);
            let raw_val = current.get(i).and_then(|(_, v)| *v);
            // Multiply by 1000 so decimal values (e.g. 0.5) don't truncate to 0.
            let u_val = (raw_val.unwrap_or(0.0).max(0.0) * 1000.0).round() as u64;
            let val_str = legend_value_str(args.current_states.get(i).and_then(|o| *o));
            Bar::with_label(series_label(s).to_string(), u_val)
                .text_value(val_str)
                .style(Style::new().fg(color))
        })
        .collect();

    match orientation {
        BarOrientation::Vertical => {
            let bar_w = ((inner.width.saturating_sub((n - 1) * gap)) / n).clamp(3, 20);
            let chart = BarChart::vertical(bars).block(block).bar_gap(gap).bar_width(bar_w);
            f.render_widget(chart, args.area);
        }
        BarOrientation::Horizontal => {
            let bar_h = ((inner.height.saturating_sub((n - 1) * gap)) / n).clamp(1, 3);
            let chart = BarChart::horizontal(bars).block(block).bar_gap(gap).bar_width(bar_h);
            f.render_widget(chart, args.area);
        }
    }
}

pub fn render_pie(f: &mut Frame, args: GraphRender<'_>, current: &[(EntityId, Option<f64>)]) {
    use tui_piechart::{PieChart, PieSlice};

    let card_color =
        crate::ui::theme::resolve_card_color(args.card_color, args.instance, args.theme);
    let block = make_block(args.title, card_color, args.selected);
    let inner = block.inner(args.area);

    struct SliceData {
        label: String,
        value: f64,
        color: ratatui::style::Color,
    }
    let slice_data: Vec<SliceData> = args
        .series
        .iter()
        .enumerate()
        .filter_map(|(i, s)| {
            let v = current.get(i).and_then(|(_, v)| *v)?;
            if v <= 0.0 {
                return None;
            }
            let color = series_color(s, args.card_color, args.instance, args.theme);
            let val_str = legend_value_str(args.current_states.get(i).and_then(|o| *o));
            let label = format!("{} ({})", series_label(s), val_str);
            Some(SliceData { label, value: v, color })
        })
        .collect();
    let pie_slices: Vec<PieSlice> = slice_data
        .iter()
        .map(|d| PieSlice::new(&d.label, d.value, d.color))
        .collect();

    if pie_slices.is_empty() {
        f.render_widget(
            Paragraph::new("(no positive values)")
                .style(Style::new().dim())
                .block(block),
            args.area,
        );
        return;
    }

    f.render_widget(block, args.area);
    f.render_widget(
        PieChart::new(pie_slices).show_legend(true).show_percentages(true),
        inner,
    );
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

#[cfg(test)]
mod tests {
    use super::legend_value_str;
    use crate::ha::EntityState;
    use serde_json::json;

    fn make_state(state: &str, attrs: serde_json::Value) -> EntityState {
        EntityState {
            entity_id: "test.entity".to_string(),
            state: state.to_string(),
            attributes: attrs,
            last_changed: None,
            last_updated: None,
        }
    }

    #[test]
    fn none_gives_em_dash() {
        assert_eq!(legend_value_str(None), "\u{2014}");
    }

    #[test]
    fn numeric_with_unit() {
        let s = make_state("21.0", json!({"unit_of_measurement": "°C"}));
        assert_eq!(legend_value_str(Some(&s)), "21 °C");
    }

    #[test]
    fn numeric_no_unit() {
        let s = make_state("21.0", json!({}));
        assert_eq!(legend_value_str(Some(&s)), "21");
    }

    #[test]
    fn non_numeric_state() {
        let s = make_state("on", json!({}));
        assert_eq!(legend_value_str(Some(&s)), "on");
    }

    #[test]
    fn whole_number_no_trailing_decimals() {
        let s = make_state("42.0", json!({}));
        assert_eq!(legend_value_str(Some(&s)), "42");
    }

    #[test]
    fn fractional_rounded_to_1dp() {
        let s = make_state("21.567", json!({"unit_of_measurement": "%"}));
        assert_eq!(legend_value_str(Some(&s)), "21.6 %");
    }
}
