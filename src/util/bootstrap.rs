use std::path::Path;

use color_eyre::eyre::Context;
use color_eyre::Result;

use crate::dashboard::{Card, CardKind, CardSize, Dashboard, DashboardFile, Grid, Pos};

const EXAMPLE_CONFIG: &str = include_str!("../../config/config.example.yaml");

const WELCOME_MARKDOWN: &str = "# Welcome to ha-tui\n\n\
1. Edit `~/.config/ha-tui/config.yaml` and add your Home Assistant URL + long-lived token.\n\
2. Restart ha-tui.\n\
3. Press `e` to enter the editor and build your own dashboards.\n\n\
Press `?` for keybindings. Docs: https://github.com/webbson/homeassistant-tui";

#[derive(Debug, Default)]
pub struct BootstrapReport {
    pub config_created: bool,
    pub dashboards_created: bool,
}

pub fn ensure_files(config_path: &Path, dashboards_path: &Path) -> Result<BootstrapReport> {
    let mut report = BootstrapReport::default();

    if !config_path.exists() {
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("mkdir -p {}", parent.display()))?;
        }
        std::fs::write(config_path, EXAMPLE_CONFIG)
            .with_context(|| format!("write {}", config_path.display()))?;
        report.config_created = true;
    }

    if !dashboards_path.exists() {
        let file = welcome_dashboard_file();
        crate::dashboard::persist::save(&file, dashboards_path)?;
        report.dashboards_created = true;
    }

    Ok(report)
}

fn welcome_dashboard_file() -> DashboardFile {
    DashboardFile {
        dashboards: vec![Dashboard {
            name: "Welcome".into(),
            grid: Grid { cols: 12, rows: 8 },
            cards: vec![Card {
                kind: CardKind::Text {
                    markdown: WELCOME_MARKDOWN.into(),
                    title: Some("Setup".into()),
                },
                pos: Pos {
                    col: 1,
                    row: 1,
                    w: 10,
                    h: 5,
                },
                color: None,
                size: CardSize::Normal,
            }],
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn writes_both_files_when_missing() {
        let dir = TempDir::new().unwrap();
        let cfg = dir.path().join("nested/config.yaml");
        let dash = dir.path().join("nested/dashboards.yaml");
        let r = ensure_files(&cfg, &dash).unwrap();
        assert!(r.config_created);
        assert!(r.dashboards_created);
        assert!(cfg.exists());
        assert!(dash.exists());
    }

    #[test]
    fn leaves_existing_files_alone() {
        let dir = TempDir::new().unwrap();
        let cfg = dir.path().join("config.yaml");
        let dash = dir.path().join("dashboards.yaml");
        std::fs::write(&cfg, "preserve me").unwrap();
        std::fs::write(&dash, "dashboards: []\n").unwrap();
        let r = ensure_files(&cfg, &dash).unwrap();
        assert!(!r.config_created);
        assert!(!r.dashboards_created);
        assert_eq!(std::fs::read_to_string(&cfg).unwrap(), "preserve me");
    }

    #[test]
    fn welcome_dashboard_parses_back() {
        let f = welcome_dashboard_file();
        let yaml = serde_yaml::to_string(&f).unwrap();
        let back: DashboardFile = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(back.dashboards.len(), 1);
        assert_eq!(back.dashboards[0].name, "Welcome");
        assert_eq!(back.dashboards[0].cards.len(), 1);
    }
}
