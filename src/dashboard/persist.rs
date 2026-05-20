use std::path::{Path, PathBuf};

use color_eyre::eyre::Context;
use color_eyre::Result;

use crate::dashboard::{CardId, CardKind, DashboardFile};

pub fn default_path() -> Option<PathBuf> {
    crate::util::paths::config_dir().map(|d| d.join("dashboards.yaml"))
}

pub fn load(explicit: Option<&Path>) -> Result<DashboardFile> {
    let path = match explicit {
        Some(p) => p.to_path_buf(),
        None => {
            default_path().ok_or_else(|| color_eyre::eyre::eyre!("cannot resolve config dir"))?
        }
    };
    if !path.exists() {
        return Ok(DashboardFile { dashboards: vec![] });
    }
    let raw = std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let mut file: DashboardFile =
        serde_yaml::from_str(&raw).with_context(|| format!("parse {}", path.display()))?;

    // Assign stable CardIds to any card that doesn't have one yet.
    // Track the global high-water mark across all dashboards so IDs are
    // unique file-wide (makes cross-dashboard moves unambiguous).
    let max_existing = file
        .dashboards
        .iter()
        .flat_map(|d| d.cards.iter())
        .map(|c| c.id.0)
        .max()
        .unwrap_or(0);
    let mut next_id = max_existing + 1;

    for (d_idx, d) in file.dashboards.iter_mut().enumerate() {
        for (c_idx, c) in d.cards.iter_mut().enumerate() {
            if c.id.is_zero() {
                c.id = CardId(next_id);
                next_id += 1;
            }
            c.normalize();
            if let CardKind::Graph { entities, .. } = &c.kind {
                if entities.is_empty() {
                    return Err(color_eyre::eyre::eyre!(
                        "dashboard {} card {} graph has no entities",
                        d_idx,
                        c_idx
                    ));
                }
            }
        }
    }
    Ok(file)
}

pub fn save(file: &DashboardFile, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("mkdir -p {}", parent.display()))?;
    }
    let raw = serde_yaml::to_string(file)?;
    std::fs::write(path, raw).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dashboard::{Card, CardId, CardKind, CardSize, Dashboard, Grid, Pos};

    #[test]
    fn round_trip_yaml() {
        let f = DashboardFile {
            dashboards: vec![Dashboard {
                name: "Home".into(),
                grid: Grid { cols: 12, rows: 8 },
                cards: vec![Card {
                    id: CardId::ZERO,
                    pos: Pos {
                        col: 0,
                        row: 0,
                        w: 3,
                        h: 2,
                    },
                    kind: CardKind::Entity {
                        instance: "home".into(),
                        entity: "light.kitchen".into(),
                        title: Some("Kitchen".into()),
                        ticker: false,
                    },
                    color: None,
                    size: CardSize::Normal,
                }],
            }],
        };
        let yaml = serde_yaml::to_string(&f).unwrap();
        let back: DashboardFile = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(back.dashboards.len(), 1);
        assert_eq!(back.dashboards[0].cards.len(), 1);
    }

    #[test]
    fn parses_example_format() {
        let yaml = r#"
dashboards:
  - name: "Home"
    grid: { cols: 12, rows: 8 }
    cards:
      - type: entity
        instance: home
        entity: light.kitchen
        pos: { col: 0, row: 0, w: 3, h: 2 }
      - type: gauge
        instance: cabin
        entity: sensor.temp
        min: -20
        max: 30
        unit: "°C"
        pos: { col: 3, row: 0, w: 3, h: 3 }
"#;
        let f: DashboardFile = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(f.dashboards[0].cards.len(), 2);
    }
}
