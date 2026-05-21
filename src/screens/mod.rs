pub mod dashboard;
pub mod editor;
pub mod entities;

use crate::config::Alias;
use crate::ha::EntityId;

#[derive(Debug, Clone)]
pub enum Screen {
    Dashboard {
        idx: usize,
        selected_card: usize,
        sub_index: usize,
    },
    Editor,
}

impl Default for Screen {
    fn default() -> Self {
        Self::Dashboard {
            idx: 0,
            selected_card: 0,
            sub_index: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Overlay {
    EntitySearch {
        query: String,
        selected: usize,
        instance_filter: Option<Alias>,
    },
    InstanceList {
        selected: usize,
    },
    InputValue(InputModalState),
    EntityDetails {
        alias: Alias,
        entity_id: EntityId,
        scroll: u16,
    },
}

#[derive(Debug, Clone)]
pub struct InputModalState {
    pub alias: Alias,
    pub entity_id: EntityId,
    pub friendly_name: String,
    pub kind: InputModalKind,
    pub buffer: String,
    /// Byte position in buffer (kept at end for simplicity).
    pub cursor: usize,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum InputModalKind {
    Number {
        min: f64,
        max: f64,
        step: f64,
    },
    Text {
        min_length: usize,
        max_length: usize,
        password: bool,
    },
    Select {
        options: Vec<String>,
        selected: usize,
    },
    DateTime {
        has_date: bool,
        has_time: bool,
    },
}
