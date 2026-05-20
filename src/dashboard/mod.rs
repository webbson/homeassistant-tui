pub mod editor;
pub mod layout;
pub mod persist;
pub mod query;

use serde::{Deserialize, Serialize};

use crate::config::Alias;
use crate::ha::EntityId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardFile {
    pub dashboards: Vec<Dashboard>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dashboard {
    pub name: String,
    pub grid: Grid,
    pub cards: Vec<Card>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Grid {
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CardSize {
    Small,
    #[default]
    Normal,
    Large,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
    #[serde(flatten)]
    pub kind: CardKind,
    pub pos: Pos,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "is_default_size")]
    pub size: CardSize,
}

fn is_default_size(s: &CardSize) -> bool {
    *s == CardSize::Normal
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Pos {
    pub col: u16,
    pub row: u16,
    pub w: u16,
    pub h: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CardKind {
    Entity {
        instance: Alias,
        entity: EntityId,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        ticker: bool,
    },
    Toggle {
        instance: Alias,
        entity: EntityId,
        #[serde(default)]
        title: Option<String>,
    },
    Gauge {
        instance: Alias,
        entity: EntityId,
        min: f64,
        max: f64,
        #[serde(default)]
        unit: Option<String>,
        #[serde(default)]
        title: Option<String>,
    },
    Sparkline {
        instance: Alias,
        entity: EntityId,
        #[serde(default = "default_window")]
        window: String,
        #[serde(default)]
        title: Option<String>,
    },
    Text {
        markdown: String,
        #[serde(default)]
        title: Option<String>,
    },
    EntityList {
        instance: Alias,
        entities: Vec<EntityId>,
        #[serde(default)]
        title: Option<String>,
    },
    FilteredEntityList {
        instance: Alias,
        /// Query string: `glob[filter1][filter2]...`
        query: String,
        #[serde(default)]
        hide_state: bool,
        #[serde(default)]
        title: Option<String>,
    },
}

fn default_window() -> String {
    "1h".into()
}

impl Card {
    pub fn title(&self) -> &str {
        match &self.kind {
            CardKind::Entity { title, entity, .. }
            | CardKind::Toggle { title, entity, .. }
            | CardKind::Gauge { title, entity, .. }
            | CardKind::Sparkline { title, entity, .. } => title.as_deref().unwrap_or(entity),
            CardKind::Text { title, .. } => title.as_deref().unwrap_or("Text"),
            CardKind::EntityList { title, .. } => title.as_deref().unwrap_or("Entities"),
            CardKind::FilteredEntityList { title, .. } => title.as_deref().unwrap_or("Filtered"),
        }
    }

    pub fn entity_ref(&self) -> Option<(&Alias, &EntityId)> {
        match &self.kind {
            CardKind::Entity {
                instance, entity, ..
            }
            | CardKind::Toggle {
                instance, entity, ..
            }
            | CardKind::Gauge {
                instance, entity, ..
            }
            | CardKind::Sparkline {
                instance, entity, ..
            } => Some((instance, entity)),
            CardKind::Text { .. }
            | CardKind::EntityList { .. }
            | CardKind::FilteredEntityList { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_serde_round_trip_with_size_and_color() {
        let yaml = r##"
type: entity
instance: home
entity: light.kitchen
pos: { col: 0, row: 0, w: 4, h: 2 }
color: "#ff8800"
size: large
"##;
        let card: Card = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(card.color.as_deref(), Some("#ff8800"));
        assert_eq!(card.size, CardSize::Large);
        let back = serde_yaml::to_string(&card).unwrap();
        assert!(back.contains("size: large"));
        assert!(back.contains("color: \"#ff8800\"") || back.contains("color: '#ff8800'"));
    }

    #[test]
    fn card_serde_omits_defaults() {
        let yaml = r#"
type: entity
instance: home
entity: light.kitchen
pos: { col: 0, row: 0, w: 4, h: 2 }
"#;
        let card: Card = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(card.color, None);
        assert_eq!(card.size, CardSize::Normal);
        let back = serde_yaml::to_string(&card).unwrap();
        assert!(!back.contains("size:"));
        assert!(!back.contains("color:"));
    }
}
