pub mod editor;
pub mod layout;
pub mod persist;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
    #[serde(flatten)]
    pub kind: CardKind,
    pub pos: Pos,
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
            CardKind::Text { .. } => None,
        }
    }
}
