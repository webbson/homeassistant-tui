pub mod dashboard;
pub mod editor;
pub mod entities;
pub mod instances;

use crate::config::Alias;

#[derive(Debug, Clone)]
pub enum Screen {
    Entities {
        instance_filter: Option<Alias>,
        search: String,
        selected: usize,
    },
    Instances {
        selected: usize,
    },
    Dashboard {
        idx: usize,
        selected_card: usize,
    },
    Editor,
}

impl Default for Screen {
    fn default() -> Self {
        Self::Entities {
            instance_filter: None,
            search: String::new(),
            selected: 0,
        }
    }
}
