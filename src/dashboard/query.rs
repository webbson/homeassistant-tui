//! Entity-list query parser: `glob[filter1][filter2]...`
//!
//! Examples:
//!   `sensor.hbg_*`
//!   `sensor.hbg_*[state=on]`
//!   `sensor.hbg_*[state!=unavailable][attr.location="HBG - Helsingborg"]`
//!
//! Supported operators:
//!   `=`   exact match
//!   `!=`  not equal
//!   `~=`  contains (substring)
//!   `~!`  does not contain

use serde_json::Value;

#[derive(Debug, Clone)]
pub struct EntityQuery {
    pub glob: String,
    pub filters: Vec<Filter>,
}

#[derive(Debug, Clone)]
pub struct Filter {
    pub field: Field,
    pub op: Op,
    pub value: String,
}

#[derive(Debug, Clone)]
pub enum Field {
    State,
    Attribute(String),
}

#[derive(Debug, Clone, Copy)]
pub enum Op {
    Eq,
    Ne,
    Contains,
    NotContains,
}

impl EntityQuery {
    pub fn parse(raw: &str) -> Result<EntityQuery, String> {
        let raw = raw.trim();
        let first_bracket = raw.find('[');
        let glob = match first_bracket {
            Some(i) => raw[..i].trim().to_string(),
            None => raw.to_string(),
        };
        if glob.is_empty() {
            return Err("empty glob".into());
        }
        let mut filters = Vec::new();
        let mut rest = match first_bracket {
            Some(i) => &raw[i..],
            None => "",
        };
        while let Some(open) = rest.find('[') {
            let after = &rest[open + 1..];
            let close = after
                .find(']')
                .ok_or_else(|| "unterminated [...] block".to_string())?;
            let inner = &after[..close];
            filters.push(parse_filter(inner)?);
            rest = &after[close + 1..];
        }
        Ok(EntityQuery { glob, filters })
    }

    pub fn matches(&self, entity_id: &str, state: &str, attributes: &Value) -> bool {
        if !glob_match(&self.glob, entity_id) {
            return false;
        }
        for f in &self.filters {
            let actual: String = match &f.field {
                Field::State => state.to_string(),
                Field::Attribute(path) => attr_get(attributes, path)
                    .map(value_to_string)
                    .unwrap_or_default(),
            };
            if !f.op.eval(&actual, &f.value) {
                return false;
            }
        }
        true
    }
}

impl Op {
    fn eval(self, actual: &str, expected: &str) -> bool {
        match self {
            Op::Eq => actual == expected,
            Op::Ne => actual != expected,
            Op::Contains => actual.contains(expected),
            Op::NotContains => !actual.contains(expected),
        }
    }
}

fn parse_filter(s: &str) -> Result<Filter, String> {
    let s = s.trim();
    // Try operators longest-first
    for (token, op) in &[
        ("~!", Op::NotContains),
        ("~=", Op::Contains),
        ("!=", Op::Ne),
        ("=", Op::Eq),
    ] {
        if let Some(i) = s.find(token) {
            let key = s[..i].trim();
            let raw_val = s[i + token.len()..].trim();
            let value = strip_quotes(raw_val);
            let field = parse_field(key)?;
            return Ok(Filter {
                field,
                op: *op,
                value,
            });
        }
    }
    Err(format!("filter `{s}` has no operator"))
}

fn parse_field(key: &str) -> Result<Field, String> {
    match key {
        "state" => Ok(Field::State),
        other if other.starts_with("attr.") => Ok(Field::Attribute(other[5..].to_string())),
        other => Err(format!("unknown filter field `{other}`")),
    }
}

fn strip_quotes(s: &str) -> String {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"') && s.len() >= 2)
        || (s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2)
    {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

fn attr_get<'a>(attrs: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = attrs;
    for part in path.split('.') {
        cur = cur.get(part)?;
    }
    Some(cur)
}

fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Resolve a raw query against an instance runtime, returning sorted entity ids.
pub fn resolve(runtime: Option<&crate::ha::InstanceRuntime>, raw: &str) -> Vec<String> {
    let Some(rt) = runtime else { return Vec::new() };
    let Ok(q) = EntityQuery::parse(raw) else {
        return Vec::new();
    };
    let mut ids: Vec<String> = rt
        .states
        .values()
        .filter(|s| q.matches(&s.entity_id, &s.state, &s.attributes))
        .map(|s| s.entity_id.clone())
        .collect();
    ids.sort();
    ids
}

/// Minimal glob: supports `*` (any sequence) and `?` (single char). Anchored.
pub fn glob_match(pattern: &str, text: &str) -> bool {
    fn go(p: &[u8], t: &[u8]) -> bool {
        match (p.first(), t.first()) {
            (None, None) => true,
            (Some(b'*'), _) => {
                // empty match
                if go(&p[1..], t) {
                    return true;
                }
                if t.is_empty() {
                    return false;
                }
                go(p, &t[1..])
            }
            (Some(b'?'), Some(_)) => go(&p[1..], &t[1..]),
            (Some(a), Some(b)) if a == b => go(&p[1..], &t[1..]),
            _ => false,
        }
    }
    go(pattern.as_bytes(), text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn glob_basic() {
        assert!(glob_match("sensor.hbg_*", "sensor.hbg_temp"));
        assert!(!glob_match("sensor.hbg_*", "switch.kitchen"));
        assert!(glob_match("light.?ed", "light.bed"));
        assert!(!glob_match("light.?ed", "light.bed1"));
    }

    #[test]
    fn parse_full_query() {
        let q = EntityQuery::parse(r#"sensor.hbg_*[state=on][attr.location="HBG - Helsingborg"]"#)
            .unwrap();
        assert_eq!(q.glob, "sensor.hbg_*");
        assert_eq!(q.filters.len(), 2);
    }

    #[test]
    fn matches_state() {
        let q = EntityQuery::parse("light.*[state=on]").unwrap();
        let attrs = json!({});
        assert!(q.matches("light.kitchen", "on", &attrs));
        assert!(!q.matches("light.kitchen", "off", &attrs));
        assert!(!q.matches("switch.x", "on", &attrs));
    }

    #[test]
    fn matches_attr() {
        let q = EntityQuery::parse(r#"sensor.*[attr.location="HBG - Helsingborg"]"#).unwrap();
        let attrs = json!({ "location": "HBG - Helsingborg" });
        assert!(q.matches("sensor.x", "anything", &attrs));
        let attrs2 = json!({ "location": "Other" });
        assert!(!q.matches("sensor.x", "anything", &attrs2));
    }

    #[test]
    fn contains_op() {
        let q = EntityQuery::parse(r#"sensor.*[attr.location~="HBG"]"#).unwrap();
        let attrs = json!({ "location": "HBG - Helsingborg" });
        assert!(q.matches("sensor.x", "v", &attrs));
        let attrs2 = json!({ "location": "Other" });
        assert!(!q.matches("sensor.x", "v", &attrs2));
    }
}
