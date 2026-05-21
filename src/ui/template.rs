/// Render a per-row template string against a JSON object item.
///
/// Syntax: `{key}` or `{key|filter}`.
/// Special synthetic keys: `{rank}` (1-based) and `{index}` (0-based).
///
/// Filters: `round`, `int`, `thousands`.
/// Unknown keys pass through as literal `{key}` so typos are visible.
pub fn render_row(template: &str, item: &serde_json::Value, rank: usize) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}') {
            None => {
                out.push_str(&rest[open..]);
                return out;
            }
            Some(close) => {
                let token = &after[..close];
                out.push_str(&eval_token(token, item, rank));
                rest = &after[close + 1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn eval_token(token: &str, item: &serde_json::Value, rank: usize) -> String {
    let (key, filter) = match token.split_once('|') {
        Some((k, f)) => (k.trim(), f.trim()),
        None => (token.trim(), ""),
    };

    // Synthetic keys
    match key {
        "rank" => return format!("{}", rank + 1),
        "index" => return format!("{rank}"),
        _ => {}
    }

    let val = match item.get(key) {
        Some(v) => v,
        None => return format!("{{{token}}}"),
    };

    match filter {
        "" => stringify(val),
        "round" => apply_numeric(val, |n| format!("{}", n.round() as i64)),
        "int" => apply_numeric(val, |n| format!("{}", n.trunc() as i64)),
        "thousands" => apply_numeric(val, |n| format_thousands(n.round() as i64)),
        _ => stringify(val),
    }
}

fn stringify(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn parse_f64(v: &serde_json::Value) -> Option<f64> {
    match v {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.parse::<f64>().ok(),
        _ => None,
    }
}

fn apply_numeric(v: &serde_json::Value, f: impl Fn(f64) -> String) -> String {
    parse_f64(v).map(f).unwrap_or_else(|| stringify(v))
}

fn format_thousands(n: i64) -> String {
    let neg = n < 0;
    let abs = n.unsigned_abs();
    let s = abs.to_string();
    let with_commas: String = s
        .as_bytes()
        .rchunks(3)
        .rev()
        .map(|c| std::str::from_utf8(c).unwrap())
        .collect::<Vec<_>>()
        .join(",");
    if neg {
        format!("-{with_commas}")
    } else {
        with_commas
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn item() -> serde_json::Value {
        json!({"name": "Alice", "dollars": "123.45", "count": 42, "neg": "-9876.5"})
    }

    #[test]
    fn simple_key() {
        assert_eq!(render_row("{name}", &item(), 0), "Alice");
    }

    #[test]
    fn multiple_tokens() {
        assert_eq!(
            render_row("{name}: ${dollars}", &item(), 0),
            "Alice: $123.45"
        );
    }

    #[test]
    fn round_filter_string_number() {
        assert_eq!(render_row("{dollars|round}", &item(), 0), "123");
    }

    #[test]
    fn round_filter_half_up() {
        let v = json!({"x": "123.5"});
        assert_eq!(render_row("{x|round}", &v, 0), "124");
    }

    #[test]
    fn round_filter_integer_value() {
        assert_eq!(render_row("{count|round}", &item(), 0), "42");
    }

    #[test]
    fn int_filter_truncates() {
        let v = json!({"x": "9.9"});
        assert_eq!(render_row("{x|int}", &v, 0), "9");
    }

    #[test]
    fn thousands_filter() {
        let v = json!({"n": "4757280"});
        assert_eq!(render_row("{n|thousands}", &v, 0), "4,757,280");
    }

    #[test]
    fn thousands_negative() {
        assert_eq!(render_row("{neg|thousands}", &item(), 0), "-9,877");
    }

    #[test]
    fn thousands_small() {
        let v = json!({"n": "42"});
        assert_eq!(render_row("{n|thousands}", &v, 0), "42");
    }

    #[test]
    fn unknown_key_passthrough() {
        assert_eq!(render_row("{ghost}", &item(), 0), "{ghost}");
    }

    #[test]
    fn unknown_key_with_filter_passthrough() {
        assert_eq!(render_row("{ghost|round}", &item(), 0), "{ghost|round}");
    }

    #[test]
    fn rank_token() {
        assert_eq!(render_row("{rank}. {name}", &item(), 0), "1. Alice");
        assert_eq!(render_row("{rank}", &item(), 4), "5");
    }

    #[test]
    fn index_token() {
        assert_eq!(render_row("{index}", &item(), 3), "3");
    }

    #[test]
    fn literal_text_no_tokens() {
        assert_eq!(render_row("hello world", &item(), 0), "hello world");
    }

    #[test]
    fn unclosed_brace_preserved() {
        assert_eq!(
            render_row("before {unclosed", &item(), 0),
            "before {unclosed"
        );
    }

    #[test]
    fn empty_template() {
        assert_eq!(render_row("", &item(), 0), "");
    }
}
