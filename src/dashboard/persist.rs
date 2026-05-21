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
        .flat_map(|d| d.cards_iter())
        .map(|c| c.id.0)
        .max()
        .unwrap_or(0);
    let mut next_id = max_existing + 1;

    for (d_idx, d) in file.dashboards.iter_mut().enumerate() {
        for (c_idx, c) in d.cards_iter_mut().enumerate() {
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
    use crate::dashboard::{
        Card, CardId, CardKind, CardSize, Dashboard, DashboardLayout, Grid, Pos,
    };

    #[test]
    fn round_trip_yaml() {
        let f = DashboardFile {
            dashboards: vec![Dashboard {
                name: "Home".into(),
                layout: DashboardLayout::Free {
                    grid: Grid { cols: 12, rows: 8 },
                    cards: vec![Card {
                        id: CardId::ZERO,
                        pos: Some(Pos {
                            col: 0,
                            row: 0,
                            w: 3,
                            h: 2,
                        }),
                        height: None,
                        kind: CardKind::Entity {
                            instance: "home".into(),
                            entity: "light.kitchen".into(),
                            title: Some("Kitchen".into()),
                            ticker: false,
                        },
                        color: None,
                        size: CardSize::Normal,
                    }],
                },
            }],
        };
        let yaml = serde_yaml::to_string(&f).unwrap();
        let back: DashboardFile = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(back.dashboards.len(), 1);
        assert_eq!(back.dashboards[0].card_count(), 1);
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
        assert_eq!(f.dashboards[0].card_count(), 2);
    }

    #[test]
    fn free_dashboard_serialize_omits_type_key() {
        // Free dashboards must serialize without a `type:` key so existing
        // user YAML files round-trip cleanly with no migration required.
        let f = DashboardFile {
            dashboards: vec![Dashboard {
                name: "Test".into(),
                layout: DashboardLayout::Free {
                    grid: Grid { cols: 12, rows: 8 },
                    cards: vec![],
                },
            }],
        };
        let yaml = serde_yaml::to_string(&f).unwrap();
        assert!(
            !yaml.contains("type:"),
            "free dashboard must not emit type: key"
        );
        assert!(yaml.contains("grid:"), "free dashboard must emit grid:");
        assert!(yaml.contains("cards:"), "free dashboard must emit cards:");
        // Re-parse and verify structure preserved
        let back: DashboardFile = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(back.dashboards[0].name, "Test");
        assert!(back.dashboards[0].is_free());
    }

    #[test]
    fn grid_dashboard_roundtrip() {
        use crate::dashboard::{GridColumn, GridRow, RowHeight};
        let yaml = r#"
dashboards:
  - name: "Stacked"
    type: grid
    rows:
      - height: 4
        columns:
          - cards: []
          - cards: []
      - height: auto
        fill_height: true
        columns:
          - cards: []
"#;
        let f: DashboardFile = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(f.dashboards[0].name, "Stacked");
        assert!(!f.dashboards[0].is_free());
        let DashboardLayout::Grid { rows } = &f.dashboards[0].layout else {
            panic!("expected grid layout");
        };
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].height, RowHeight::Fixed(4));
        assert_eq!(rows[1].height, RowHeight::Auto);
        assert_eq!(rows[1].fill_height, Some(true));
        // Serialize and re-parse
        let back_yaml = serde_yaml::to_string(&f).unwrap();
        assert!(back_yaml.contains("type: grid"));
        let back: DashboardFile = serde_yaml::from_str(&back_yaml).unwrap();
        let DashboardLayout::Grid { rows: back_rows } = &back.dashboards[0].layout else {
            panic!("expected grid layout after roundtrip");
        };
        assert_eq!(back_rows.len(), 2);
    }
}
