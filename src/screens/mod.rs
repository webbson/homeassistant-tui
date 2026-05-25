pub mod dashboard;
pub mod editor;
pub mod entities;

use tui_input::Input as TInput;

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
    InstanceForm(Box<InstanceFormState>),
    InstanceDeleteConfirm {
        alias: Alias,
        affected_cards: usize,
        affected_dashboards: usize,
    },
    InputValue(InputModalState),
    EntityDetails {
        alias: Alias,
        entity_id: EntityId,
        scroll: u16,
    },
}

#[derive(Debug, Clone)]
pub struct InstanceFormState {
    pub mode: InstanceFormMode,
    pub alias_buf: TInput,
    pub url_buf: TInput,
    pub token_buf: TInput,
    pub color_buf: TInput,
    pub focus: InstanceFormField,
    pub error: Option<String>,
    /// When true, Esc quits the app (first-run — nothing to show without an instance).
    pub first_run: bool,
}

impl InstanceFormState {
    pub fn new_add() -> Self {
        Self {
            mode: InstanceFormMode::New,
            alias_buf: TInput::default(),
            url_buf: TInput::default(),
            token_buf: TInput::default(),
            color_buf: TInput::default(),
            focus: InstanceFormField::Alias,
            error: None,
            first_run: false,
        }
    }

    pub fn new_first_run() -> Self {
        Self {
            first_run: true,
            ..Self::new_add()
        }
    }

    pub fn new_edit(alias: &str, url: &str, color: Option<&str>) -> Self {
        Self {
            mode: InstanceFormMode::Edit {
                original_alias: alias.to_string(),
            },
            alias_buf: TInput::new(alias.to_string()),
            url_buf: TInput::new(url.to_string()),
            token_buf: TInput::default(),
            color_buf: TInput::new(color.unwrap_or("").to_string()),
            focus: InstanceFormField::Alias,
            error: None,
            first_run: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum InstanceFormMode {
    New,
    Edit { original_alias: Alias },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceFormField {
    Alias,
    Url,
    Token,
    Color,
}

impl InstanceFormField {
    pub fn next(self) -> Self {
        match self {
            Self::Alias => Self::Url,
            Self::Url => Self::Token,
            Self::Token => Self::Color,
            Self::Color => Self::Alias,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Alias => Self::Color,
            Self::Url => Self::Alias,
            Self::Token => Self::Url,
            Self::Color => Self::Token,
        }
    }
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
