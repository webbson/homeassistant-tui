use crate::ha::EntityState;

/// Format the numeric state of `s` using HA's `display_precision` attribute when present.
/// Falls back to `default_precision` decimals for f64 values, or to the raw string for
/// non-numeric states. Whole numbers (no fractional part) drop trailing decimals when
/// `display_precision` is not set.
pub fn format_state(s: &EntityState, default_precision: usize) -> String {
    let Some(v) = s.state.parse::<f64>().ok() else {
        return s.state.clone();
    };
    let empty = serde_json::Map::new();
    let attrs = s.attributes.as_object().unwrap_or(&empty);
    format_f64_smart(v, attrs, default_precision)
}

/// Format an f64 value with smart precision:
/// - If `display_precision` is present in `attrs`, use that exact precision.
/// - If the value has no fractional part, format with 0 decimals.
/// - Otherwise, use `default_precision`.
pub fn format_f64_smart(
    v: f64,
    attrs: &serde_json::Map<String, serde_json::Value>,
    default_precision: usize,
) -> String {
    if let Some(p) = attrs.get("display_precision").and_then(|x| x.as_u64()) {
        return format!("{v:.prec$}", prec = p as usize);
    }
    if v.fract().abs() < f64::EPSILON {
        format!("{v:.0}")
    } else {
        format!("{v:.default_precision$}")
    }
}

pub fn unit_of(s: &EntityState) -> &str {
    s.attributes
        .get("unit_of_measurement")
        .and_then(|v| v.as_str())
        .unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn whole_number_drops_decimals() {
        let s = make_state("21.0", json!({}));
        assert_eq!(format_state(&s, 1), "21");
    }

    #[test]
    fn fractional_uses_default_precision() {
        let s = make_state("21.567", json!({}));
        assert_eq!(format_state(&s, 1), "21.6");
    }

    #[test]
    fn display_precision_wins() {
        let s = make_state("21.0", json!({"display_precision": 2}));
        assert_eq!(format_state(&s, 1), "21.00");
    }

    #[test]
    fn display_precision_zero() {
        let s = make_state("21.7", json!({"display_precision": 0}));
        assert_eq!(format_state(&s, 1), "22");
    }

    #[test]
    fn non_numeric_passthrough() {
        let s = make_state("on", json!({}));
        assert_eq!(format_state(&s, 1), "on");
    }

    #[test]
    fn negative_whole_number() {
        let s = make_state("-3.0", json!({}));
        assert_eq!(format_state(&s, 1), "-3");
    }
}
