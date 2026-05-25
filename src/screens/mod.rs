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
    /// Host only: `192.168.1.1:8123` or `homeassistant.local`. No scheme/path.
    pub host_buf: TInput,
    pub ssl: bool,
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
            host_buf: TInput::default(),
            ssl: false,
            token_buf: TInput::default(),
            color_buf: TInput::default(),
            focus: InstanceFormField::Host,
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
        let (host, ssl) = parse_url_to_host(url);
        Self {
            mode: InstanceFormMode::Edit {
                original_alias: alias.to_string(),
            },
            alias_buf: TInput::new(alias.to_string()),
            host_buf: TInput::new(host),
            ssl,
            token_buf: TInput::default(),
            color_buf: TInput::new(color.unwrap_or("").to_string()),
            focus: InstanceFormField::Host,
            error: None,
            first_run: false,
        }
    }

    /// Build the full WebSocket URL from host + ssl flag.
    pub fn build_url(&self) -> String {
        let host = self.host_buf.value().trim();
        let scheme = if self.ssl { "wss" } else { "ws" };
        format!("{scheme}://{host}/api/websocket")
    }

    /// Derive an alias from the host if the alias field is empty.
    pub fn effective_alias(&self) -> String {
        let alias = self.alias_buf.value().trim();
        if alias.is_empty() {
            // Use first label of hostname, stripping port.
            let host = self.host_buf.value().trim();
            let host_no_port = host.split(':').next().unwrap_or(host);
            let label = host_no_port.split('.').next().unwrap_or(host_no_port);
            if label.is_empty() {
                "ha".to_string()
            } else {
                label.to_string()
            }
        } else {
            alias.to_string()
        }
    }
}

/// Parse `ws(s)://host(:port)/api/websocket` back to `(host, ssl)`.
fn parse_url_to_host(url: &str) -> (String, bool) {
    let (scheme, rest) = if let Some(s) = url.strip_prefix("wss://") {
        (true, s)
    } else if let Some(s) = url.strip_prefix("ws://") {
        (false, s)
    } else {
        (false, url)
    };
    let host = rest.split('/').next().unwrap_or(rest).to_string();
    (host, scheme)
}

#[derive(Debug, Clone)]
pub enum InstanceFormMode {
    New,
    Edit { original_alias: Alias },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceFormField {
    Host,
    Ssl,
    Alias,
    Token,
    Color,
}

impl InstanceFormField {
    pub fn next(self) -> Self {
        match self {
            Self::Host => Self::Ssl,
            Self::Ssl => Self::Token,
            Self::Token => Self::Alias,
            Self::Alias => Self::Color,
            Self::Color => Self::Host,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Host => Self::Color,
            Self::Ssl => Self::Host,
            Self::Token => Self::Ssl,
            Self::Alias => Self::Token,
            Self::Color => Self::Alias,
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
