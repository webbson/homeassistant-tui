pub mod dashboard;
pub mod editor;
pub mod entities;

use crate::config::Alias;

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
}
