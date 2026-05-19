use crate::ha::EntityState;

/// Format the numeric state of `s` using HA's `display_precision` attribute when present.
/// Falls back to `default_precision` decimals for f64 values, or to the raw string for
/// non-numeric states.
pub fn format_state(s: &EntityState, default_precision: usize) -> String {
    let Some(v) = s.state.parse::<f64>().ok() else {
        return s.state.clone();
    };
    let precision = s
        .attributes
        .get("display_precision")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .unwrap_or(default_precision);
    format!("{v:.precision$}")
}

pub fn unit_of(s: &EntityState) -> &str {
    s.attributes
        .get("unit_of_measurement")
        .and_then(|v| v.as_str())
        .unwrap_or("")
}
