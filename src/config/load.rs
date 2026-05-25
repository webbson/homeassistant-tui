use std::path::{Path, PathBuf};

use color_eyre::eyre::{eyre, Context};
use color_eyre::Result;

use super::{Config, InstanceConfig};

pub fn default_config_path() -> Option<PathBuf> {
    crate::util::paths::config_dir().map(|d| d.join("config.yaml"))
}

pub fn load(explicit: Option<&Path>) -> Result<Config> {
    let path = match explicit {
        Some(p) => p.to_path_buf(),
        None => default_config_path().ok_or_else(|| eyre!("cannot resolve default config dir"))?,
    };

    if !path.exists() {
        return Err(eyre!("config not found at {}", path.display()));
    }

    let raw = std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let mut cfg: Config =
        serde_yaml::from_str(&raw).with_context(|| format!("parse {}", path.display()))?;

    for inst in &mut cfg.instances {
        resolve_token(inst)
            .with_context(|| format!("resolve token for instance {}", inst.alias))?;
    }
    validate(&cfg)?;
    Ok(cfg)
}

pub fn resolve_token(inst: &mut InstanceConfig) -> Result<()> {
    if let Some(t) = &inst.token {
        inst.resolved_token = Some(expand_env(t)?);
        return Ok(());
    }
    if let Some(file) = &inst.token_file {
        let path = expand_tilde(file);
        let val = std::fs::read_to_string(&path)
            .with_context(|| format!("read token_file {}", path.display()))?;
        inst.resolved_token = Some(val.trim().to_string());
        return Ok(());
    }
    Err(eyre!(
        "instance {} has neither token nor token_file",
        inst.alias
    ))
}

fn expand_env(s: &str) -> Result<String> {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let end = after
            .find('}')
            .ok_or_else(|| eyre!("unterminated ${{...}} in token"))?;
        let var = &after[..end];
        let val = std::env::var(var).with_context(|| format!("env var {} not set", var))?;
        out.push_str(&val);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

fn expand_tilde(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(stripped) = s.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(stripped);
        }
    }
    p.to_path_buf()
}

fn validate(cfg: &Config) -> Result<()> {
    let mut seen = std::collections::HashSet::new();
    for i in &cfg.instances {
        if !seen.insert(&i.alias) {
            return Err(eyre!("duplicate instance alias: {}", i.alias));
        }
        if !i.url.starts_with("ws://") && !i.url.starts_with("wss://") {
            return Err(eyre!(
                "instance {} url must start with ws:// or wss://",
                i.alias
            ));
        }
    }
    Ok(())
}

/// Validate a single `InstanceConfig` against an existing list.
/// Pass `exclude_alias` when editing an existing instance so its old alias is
/// not treated as a collision.
pub fn validate_one(
    inst: &InstanceConfig,
    others: &[InstanceConfig],
    exclude_alias: Option<&str>,
) -> Result<()> {
    if inst.alias.trim().is_empty() {
        return Err(eyre!("name cannot be empty"));
    }
    if inst.url.trim().is_empty() {
        return Err(eyre!("url cannot be empty"));
    }
    if !inst.url.starts_with("ws://") && !inst.url.starts_with("wss://") {
        return Err(eyre!("url must start with ws:// or wss://"));
    }
    for o in others {
        if exclude_alias.map_or(false, |ex| ex == o.alias.as_str()) {
            continue;
        }
        if o.alias == inst.alias {
            return Err(eyre!("duplicate instance alias: {}", inst.alias));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn yaml(s: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(s.as_bytes()).unwrap();
        f
    }

    #[test]
    fn loads_minimal() {
        let f = yaml(
            r#"
instances:
  - alias: home
    url: "ws://localhost:8123/api/websocket"
    token: "abc"
"#,
        );
        let cfg = load(Some(f.path())).unwrap();
        assert_eq!(cfg.instances.len(), 1);
        assert_eq!(cfg.instances[0].token.as_deref(), Some("abc"));
        assert_eq!(cfg.instances[0].resolved_token.as_deref(), Some("abc"));
    }

    #[test]
    fn expands_env_in_token() {
        unsafe { std::env::set_var("HA_TEST_TOK", "secret123") };
        let f = yaml(
            r#"
instances:
  - alias: home
    url: "ws://localhost:8123/api/websocket"
    token: "${HA_TEST_TOK}"
"#,
        );
        let cfg = load(Some(f.path())).unwrap();
        // raw token field preserved
        assert_eq!(cfg.instances[0].token.as_deref(), Some("${HA_TEST_TOK}"));
        // resolved_token holds the expanded value
        assert_eq!(
            cfg.instances[0].resolved_token.as_deref(),
            Some("secret123")
        );
    }

    #[test]
    fn rejects_duplicate_aliases() {
        let f = yaml(
            r#"
instances:
  - alias: home
    url: "ws://a/api/websocket"
    token: "a"
  - alias: home
    url: "ws://b/api/websocket"
    token: "b"
"#,
        );
        assert!(load(Some(f.path())).is_err());
    }
}
