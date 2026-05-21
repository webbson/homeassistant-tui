use ratatui::layout::Rect;
#[allow(unused_imports)]
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;
use serde_json::json;

use crate::ha::{EntityState, HaCommand};
use crate::screens::{InputModalKind, InputModalState};

// ── Render entry point ────────────────────────────────────────────────────────

pub fn render(f: &mut Frame, area: Rect, state: &InputModalState) {
    let (w, h) = match &state.kind {
        InputModalKind::Select { options, .. } => (52u16, (options.len() as u16 + 6).min(20)),
        _ => (52, 9),
    };
    let popup = centered_rect(w, h, area);
    f.render_widget(Clear, popup);

    let title = format!(" {} ", state.friendly_name);
    let block = Block::bordered().title(title);
    f.render_widget(block, popup);

    let inner = Rect {
        x: popup.x + 1,
        y: popup.y + 1,
        width: popup.width.saturating_sub(2),
        height: popup.height.saturating_sub(2),
    };

    match &state.kind {
        InputModalKind::Select { options, selected } => {
            render_select(f, inner, options, *selected, &state.error);
        }
        _ => {
            render_text_input(f, inner, state);
        }
    }
}

fn render_text_input(f: &mut Frame, area: Rect, state: &InputModalState) {
    let hint: String = match &state.kind {
        InputModalKind::Number { min, max, .. } => format!("↑↓ adjust · range [{min}, {max}]"),
        InputModalKind::Text { .. } => "Type value · Enter confirm".into(),
        InputModalKind::DateTime { has_date, has_time } => {
            let fmt = match (has_date, has_time) {
                (true, true) => "YYYY-MM-DD HH:MM:SS",
                (true, false) => "YYYY-MM-DD",
                (false, true) => "HH:MM:SS",
                _ => "",
            };
            format!("Format: {fmt}")
        }
        InputModalKind::Select { .. } => String::new(),
    };

    let display = match &state.kind {
        InputModalKind::Text { password: true, .. } => "•".repeat(state.buffer.len()),
        _ => state.buffer.clone(),
    };

    let input_line = Line::from(vec![
        Span::raw(display),
        Span::styled("█", Style::new().dim()),
    ]);
    let input = Paragraph::new(input_line).block(Block::bordered());
    let input_area = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: 3,
    };
    if input_area.y + 3 <= area.y + area.height {
        f.render_widget(input, input_area);
    }

    let mut y = area.y + 3;
    if let Some(err) = &state.error {
        let err_line =
            Paragraph::new(Line::from(Span::styled(err.as_str(), Style::new().red())));
        f.render_widget(
            err_line,
            Rect {
                x: area.x,
                y,
                width: area.width,
                height: 1,
            },
        );
        y += 1;
    }
    let footer = Paragraph::new(Line::from(Span::styled(
        format!("{hint} · Esc cancel"),
        Style::new().dim(),
    )));
    f.render_widget(
        footer,
        Rect {
            x: area.x,
            y,
            width: area.width,
            height: 1,
        },
    );
}

fn render_select(
    f: &mut Frame,
    area: Rect,
    options: &[String],
    selected: usize,
    error: &Option<String>,
) {
    let items: Vec<ListItem> = options.iter().map(|o| ListItem::new(o.as_str())).collect();
    let list = List::new(items)
        .highlight_style(Style::new().reversed())
        .highlight_symbol("▶ ");
    let mut list_state = ListState::default();
    list_state.select(Some(selected));

    let list_height = area.height.saturating_sub(2);
    let list_area = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: list_height,
    };
    f.render_stateful_widget(list, list_area, &mut list_state);

    let footer_y = area.y + list_height;
    if footer_y < area.y + area.height {
        let footer = Paragraph::new(Line::from(Span::styled(
            "↑↓ select · Enter confirm · Esc cancel",
            Style::new().dim(),
        )));
        f.render_widget(
            footer,
            Rect {
                x: area.x,
                y: footer_y,
                width: area.width,
                height: 1,
            },
        );
    }
    if let Some(err) = error {
        let ey = footer_y + 1;
        if ey < area.y + area.height {
            let err_line =
                Paragraph::new(Line::from(Span::styled(err.as_str(), Style::new().red())));
            f.render_widget(
                err_line,
                Rect {
                    x: area.x,
                    y: ey,
                    width: area.width,
                    height: 1,
                },
            );
        }
    }
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x.saturating_add(area.width.saturating_sub(width) / 2);
    let y = area.y.saturating_add(area.height.saturating_sub(height) / 2);
    Rect {
        x,
        y,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}

// ── Logic helpers (public so app.rs can call them) ────────────────────────────

/// Build an `InputModalKind` from a domain string + entity state attributes.
pub fn build_input_modal_kind(domain: &str, state: &EntityState) -> InputModalKind {
    let attrs = &state.attributes;
    match domain {
        "input_number" => {
            let min = attrs.get("min").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let max = attrs.get("max").and_then(|v| v.as_f64()).unwrap_or(100.0);
            let step = attrs.get("step").and_then(|v| v.as_f64()).unwrap_or(1.0);
            InputModalKind::Number { min, max, step }
        }
        "input_text" => {
            let min_length = attrs
                .get("min")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            let max_length = attrs
                .get("max")
                .and_then(|v| v.as_u64())
                .unwrap_or(255) as usize;
            let password = attrs.get("mode").and_then(|v| v.as_str()) == Some("password");
            InputModalKind::Text {
                min_length,
                max_length,
                password,
            }
        }
        "input_select" => {
            let options: Vec<String> = attrs
                .get("options")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            let selected = options.iter().position(|o| o == &state.state).unwrap_or(0);
            InputModalKind::Select { options, selected }
        }
        "input_datetime" => {
            let has_date = attrs
                .get("has_date")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let has_time = attrs
                .get("has_time")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            InputModalKind::DateTime { has_date, has_time }
        }
        _ => unreachable!("build_input_modal_kind called with unexpected domain: {domain}"),
    }
}

/// Validate current buffer contents before submitting.
pub fn validate_input_modal(s: &InputModalState) -> Result<(), String> {
    match &s.kind {
        InputModalKind::Number { min, max, .. } => {
            let v = s
                .buffer
                .parse::<f64>()
                .map_err(|_| "Enter a valid number".to_string())?;
            if v < *min || v > *max {
                return Err(format!("Must be between {} and {}", min, max));
            }
            Ok(())
        }
        InputModalKind::Text {
            min_length,
            max_length,
            ..
        } => {
            let len = s.buffer.len();
            if len < *min_length {
                return Err(format!("Minimum {} characters", min_length));
            }
            if len > *max_length {
                return Err(format!("Maximum {} characters", max_length));
            }
            Ok(())
        }
        InputModalKind::Select { options, selected } => {
            if *selected < options.len() {
                Ok(())
            } else {
                Err("Invalid selection".to_string())
            }
        }
        InputModalKind::DateTime { has_date, has_time } => {
            use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
            let fmt = match (has_date, has_time) {
                (true, true) => "%Y-%m-%d %H:%M:%S",
                (true, false) => "%Y-%m-%d",
                (false, true) => "%H:%M:%S",
                (false, false) => return Ok(()),
            };
            match (has_date, has_time) {
                (true, true) => NaiveDateTime::parse_from_str(&s.buffer, fmt)
                    .map(|_| ())
                    .map_err(|_| datetime_format_hint(fmt)),
                (true, false) => NaiveDate::parse_from_str(&s.buffer, fmt)
                    .map(|_| ())
                    .map_err(|_| datetime_format_hint(fmt)),
                (false, true) => NaiveTime::parse_from_str(&s.buffer, fmt)
                    .map(|_| ())
                    .map_err(|_| datetime_format_hint(fmt)),
                (false, false) => Ok(()),
            }
        }
    }
}

fn datetime_format_hint(fmt: &str) -> String {
    format!(
        "Use format: {}",
        fmt.replace("%Y", "YYYY")
            .replace("%m", "MM")
            .replace("%d", "DD")
            .replace("%H", "HH")
            .replace("%M", "MM")
            .replace("%S", "SS")
    )
}

/// Build the `HaCommand` to submit the modal value.
pub fn build_input_submit_cmd(s: &InputModalState) -> Option<HaCommand> {
    let target = json!({ "entity_id": s.entity_id });
    match &s.kind {
        InputModalKind::Number { .. } => {
            let v: f64 = s.buffer.parse().ok()?;
            Some(HaCommand::CallService {
                domain: "input_number".into(),
                service: "set_value".into(),
                service_data: json!({ "value": v }),
                target,
            })
        }
        InputModalKind::Text { .. } => Some(HaCommand::CallService {
            domain: "input_text".into(),
            service: "set_value".into(),
            service_data: json!({ "value": s.buffer.clone() }),
            target,
        }),
        InputModalKind::Select { options, selected } => {
            let option = options.get(*selected)?.clone();
            Some(HaCommand::CallService {
                domain: "input_select".into(),
                service: "select_option".into(),
                service_data: json!({ "option": option }),
                target,
            })
        }
        InputModalKind::DateTime { has_date, has_time } => {
            use chrono::NaiveDateTime;
            let fmt = match (has_date, has_time) {
                (true, true) => "%Y-%m-%d %H:%M:%S",
                (true, false) => "%Y-%m-%d",
                (false, true) => "%H:%M:%S",
                (false, false) => return None,
            };
            let (date_str, time_str) = if *has_date && *has_time {
                let dt = NaiveDateTime::parse_from_str(&s.buffer, fmt).ok()?;
                (
                    Some(dt.date().format("%Y-%m-%d").to_string()),
                    Some(dt.time().format("%H:%M:%S").to_string()),
                )
            } else if *has_date {
                (Some(s.buffer.clone()), None)
            } else {
                (None, Some(s.buffer.clone()))
            };
            let mut data = serde_json::Map::new();
            if let Some(d) = date_str {
                data.insert("date".into(), serde_json::Value::String(d));
            }
            if let Some(t) = time_str {
                data.insert("time".into(), serde_json::Value::String(t));
            }
            Some(HaCommand::CallService {
                domain: "input_datetime".into(),
                service: "set_datetime".into(),
                service_data: serde_json::Value::Object(data),
                target,
            })
        }
    }
}

/// Format a floating-point number for display in the input buffer.
pub fn format_number_for_input(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v:.6}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::ha::EntityState;

    fn make_number_state(buf: &str, min: f64, max: f64) -> InputModalState {
        InputModalState {
            alias: "home".to_string(),
            entity_id: "input_number.temp".to_string(),
            friendly_name: "Test".to_string(),
            kind: InputModalKind::Number {
                min,
                max,
                step: 1.0,
            },
            buffer: buf.to_string(),
            cursor: buf.len(),
            error: None,
        }
    }

    fn make_text_state(buf: &str, min: usize, max: usize) -> InputModalState {
        InputModalState {
            alias: "home".to_string(),
            entity_id: "input_text.name".to_string(),
            friendly_name: "Test".to_string(),
            kind: InputModalKind::Text {
                min_length: min,
                max_length: max,
                password: false,
            },
            buffer: buf.to_string(),
            cursor: buf.len(),
            error: None,
        }
    }

    fn make_select_state(current: &str, options: &[&str]) -> InputModalState {
        let options: Vec<String> = options.iter().map(|s| s.to_string()).collect();
        let selected = options.iter().position(|o| o == current).unwrap_or(0);
        InputModalState {
            alias: "home".to_string(),
            entity_id: "input_select.mode".to_string(),
            friendly_name: "Test".to_string(),
            kind: InputModalKind::Select { options, selected },
            buffer: current.to_string(),
            cursor: current.len(),
            error: None,
        }
    }

    fn make_datetime_state(buf: &str, has_date: bool, has_time: bool) -> InputModalState {
        InputModalState {
            alias: "home".to_string(),
            entity_id: "input_datetime.alarm".to_string(),
            friendly_name: "Test".to_string(),
            kind: InputModalKind::DateTime { has_date, has_time },
            buffer: buf.to_string(),
            cursor: buf.len(),
            error: None,
        }
    }

    // ── validate_input_modal ──────────────────────────────────────────────────

    #[test]
    fn validate_number_in_range() {
        let s = make_number_state("50", 0.0, 100.0);
        assert!(validate_input_modal(&s).is_ok());
    }

    #[test]
    fn validate_number_out_of_range() {
        let s = make_number_state("200", 0.0, 100.0);
        assert!(validate_input_modal(&s).is_err());
    }

    #[test]
    fn validate_number_not_numeric() {
        let s = make_number_state("abc", 0.0, 100.0);
        assert!(validate_input_modal(&s).is_err());
    }

    #[test]
    fn validate_number_at_boundary() {
        assert!(validate_input_modal(&make_number_state("0", 0.0, 100.0)).is_ok());
        assert!(validate_input_modal(&make_number_state("100", 0.0, 100.0)).is_ok());
    }

    #[test]
    fn validate_text_ok() {
        let s = make_text_state("hello", 1, 10);
        assert!(validate_input_modal(&s).is_ok());
    }

    #[test]
    fn validate_text_too_short() {
        let s = make_text_state("", 1, 10);
        assert!(validate_input_modal(&s).is_err());
    }

    #[test]
    fn validate_text_too_long() {
        let s = make_text_state("hello world!", 1, 5);
        assert!(validate_input_modal(&s).is_err());
    }

    #[test]
    fn validate_select_valid() {
        let s = make_select_state("b", &["a", "b", "c"]);
        assert!(validate_input_modal(&s).is_ok());
    }

    #[test]
    fn validate_datetime_full() {
        let s = make_datetime_state("2024-03-15 14:30:00", true, true);
        assert!(validate_input_modal(&s).is_ok());
    }

    #[test]
    fn validate_datetime_date_only() {
        let s = make_datetime_state("2024-03-15", true, false);
        assert!(validate_input_modal(&s).is_ok());
    }

    #[test]
    fn validate_datetime_time_only() {
        let s = make_datetime_state("14:30:00", false, true);
        assert!(validate_input_modal(&s).is_ok());
    }

    #[test]
    fn validate_datetime_bad_format() {
        let s = make_datetime_state("not-a-date", true, true);
        assert!(validate_input_modal(&s).is_err());
    }

    // ── build_input_submit_cmd ────────────────────────────────────────────────

    #[test]
    fn build_number_cmd() {
        let s = make_number_state("42", 0.0, 100.0);
        let cmd = build_input_submit_cmd(&s).unwrap();
        let HaCommand::CallService { domain, service, service_data, .. } = cmd else {
            panic!("expected CallService");
        };
        assert_eq!(domain, "input_number");
        assert_eq!(service, "set_value");
        assert_eq!(service_data["value"], json!(42.0));
    }

    #[test]
    fn build_text_cmd() {
        let s = make_text_state("hello", 0, 255);
        let cmd = build_input_submit_cmd(&s).unwrap();
        let HaCommand::CallService { domain, service, service_data, .. } = cmd else {
            panic!("expected CallService");
        };
        assert_eq!(domain, "input_text");
        assert_eq!(service, "set_value");
        assert_eq!(service_data["value"], json!("hello"));
    }

    #[test]
    fn build_select_cmd() {
        let s = make_select_state("b", &["a", "b", "c"]);
        let cmd = build_input_submit_cmd(&s).unwrap();
        let HaCommand::CallService { domain, service, service_data, .. } = cmd else {
            panic!("expected CallService");
        };
        assert_eq!(domain, "input_select");
        assert_eq!(service, "select_option");
        assert_eq!(service_data["option"], json!("b"));
    }

    #[test]
    fn build_datetime_cmd_full() {
        let s = make_datetime_state("2024-03-15 14:30:00", true, true);
        let cmd = build_input_submit_cmd(&s).unwrap();
        let HaCommand::CallService { domain, service, service_data, .. } = cmd else {
            panic!("expected CallService");
        };
        assert_eq!(domain, "input_datetime");
        assert_eq!(service, "set_datetime");
        assert_eq!(service_data["date"], json!("2024-03-15"));
        assert_eq!(service_data["time"], json!("14:30:00"));
    }

    // ── build_input_modal_kind ────────────────────────────────────────────────

    #[test]
    fn kind_from_input_number_attrs() {
        let state = EntityState {
            entity_id: "input_number.vol".to_string(),
            state: "50".to_string(),
            attributes: json!({ "min": 0.0, "max": 100.0, "step": 5.0 }),
            last_changed: None,
            last_updated: None,
        };
        let kind = build_input_modal_kind("input_number", &state);
        let InputModalKind::Number { min, max, step } = kind else {
            panic!("expected Number");
        };
        assert_eq!(min, 0.0);
        assert_eq!(max, 100.0);
        assert_eq!(step, 5.0);
    }

    #[test]
    fn kind_from_input_select_attrs() {
        let state = EntityState {
            entity_id: "input_select.mode".to_string(),
            state: "b".to_string(),
            attributes: json!({ "options": ["a", "b", "c"] }),
            last_changed: None,
            last_updated: None,
        };
        let kind = build_input_modal_kind("input_select", &state);
        let InputModalKind::Select { options, selected } = kind else {
            panic!("expected Select");
        };
        assert_eq!(options, vec!["a", "b", "c"]);
        assert_eq!(selected, 1); // "b" is index 1
    }

    // ── format_number_for_input ───────────────────────────────────────────────

    #[test]
    fn format_integer_value() {
        assert_eq!(format_number_for_input(42.0), "42");
    }

    #[test]
    fn format_float_value() {
        assert_eq!(format_number_for_input(3.5), "3.5");
    }
}
