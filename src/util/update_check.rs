use std::time::Duration;

const RELEASES_URL: &str = "https://api.github.com/repos/webbson/homeassistant-tui/releases/latest";

pub async fn check_latest(current_version: &str) -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .user_agent(concat!("ha-tui/", env!("CARGO_PKG_VERSION")))
        .build()
        .ok()?;

    let resp = client
        .get(RELEASES_URL)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        tracing::debug!(status = %resp.status(), "update check non-2xx");
        return None;
    }

    let text = resp.text().await.ok()?;
    let body: serde_json::Value = serde_json::from_str(&text).ok()?;
    let tag = body.get("tag_name")?.as_str()?.to_string();
    let stripped = tag.strip_prefix('v').unwrap_or(&tag).to_string();
    if newer(&stripped, current_version) {
        Some(stripped)
    } else {
        None
    }
}

fn newer(candidate: &str, current: &str) -> bool {
    let a = parse_parts(candidate);
    let b = parse_parts(current);
    a > b
}

fn parse_parts(v: &str) -> Vec<u64> {
    v.split('.')
        .map(|p| {
            p.chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
        })
        .map(|p| p.parse::<u64>().unwrap_or(0))
        .collect()
}

/// Heuristically detect how ha-tui was installed and return the upgrade command.
///
/// Checks current_exe() path:
///   - contains `/Cellar/` or `/opt/homebrew/` → Homebrew
///   - starts with `$HOME/.cargo/bin/`         → cargo install
///   - otherwise                                → releases page URL
pub fn detect_upgrade_cmd() -> &'static str {
    let path = std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let home = std::env::var("HOME").unwrap_or_default();
    classify_path(&path, &home)
}

fn classify_path(path: &str, home: &str) -> &'static str {
    if path.contains("/Cellar/") || path.contains("/opt/homebrew/") {
        return "brew upgrade ha-tui";
    }
    if !home.is_empty() && path.starts_with(&format!("{}/.cargo/bin/", home)) {
        return "cargo install ha-tui";
    }
    "https://github.com/webbson/homeassistant-tui/releases/latest"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions() {
        assert!(newer("0.2.0", "0.1.0"));
        assert!(newer("0.1.10", "0.1.2"));
        assert!(newer("1.0.0", "0.99.99"));
        assert!(!newer("0.1.0", "0.1.0"));
        assert!(!newer("0.1.0", "0.2.0"));
    }

    #[test]
    fn tolerates_pre_release_suffix() {
        // "0.2.0-rc1" parses to [0, 2, 0]; equal to 0.2.0, so not strictly newer.
        assert!(!newer("0.2.0-rc1", "0.2.0"));
        assert!(newer("0.2.0-rc1", "0.1.9"));
    }

    #[test]
    fn detect_homebrew_cellar() {
        assert_eq!(
            classify_path("/opt/homebrew/Cellar/ha-tui/0.6.0/bin/ha-tui", ""),
            "brew upgrade ha-tui"
        );
    }

    #[test]
    fn detect_homebrew_opt() {
        assert_eq!(
            classify_path("/opt/homebrew/bin/ha-tui", ""),
            "brew upgrade ha-tui"
        );
    }

    #[test]
    fn detect_cargo() {
        assert_eq!(
            classify_path("/home/user/.cargo/bin/ha-tui", "/home/user"),
            "cargo install ha-tui"
        );
    }

    #[test]
    fn detect_unknown() {
        assert_eq!(
            classify_path("/usr/local/bin/ha-tui", "/home/user"),
            "https://github.com/webbson/homeassistant-tui/releases/latest"
        );
    }

    #[test]
    fn detect_homebrew_not_triggered_by_home_dir() {
        // A user with "homebrew" in their home path should not trigger Homebrew detection
        // unless it contains /Cellar/ or /opt/homebrew/.
        assert_eq!(
            classify_path(
                "/home/homebrew_user/.cargo/bin/ha-tui",
                "/home/homebrew_user"
            ),
            "cargo install ha-tui"
        );
    }
}
