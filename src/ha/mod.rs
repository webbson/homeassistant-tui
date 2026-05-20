pub mod client;
pub mod protocol;
pub mod registry;
pub mod rest;

pub use registry::InstanceRegistry;

use indexmap::IndexMap;
use serde_json::Value;
use std::time::Instant;

use crate::config::Alias;

pub type EntityId = String;

#[derive(Debug, Clone)]
pub struct EntityState {
    pub entity_id: EntityId,
    pub state: String,
    pub attributes: Value,
    pub last_changed: Option<String>,
    pub last_updated: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnStatus {
    Disconnected,
    Connecting,
    Authenticating,
    Connected,
    Failed,
}

#[derive(Debug, Clone)]
pub struct InstanceRuntime {
    pub alias: Alias,
    #[allow(dead_code)]
    pub url: String,
    pub status: ConnStatus,
    pub last_error: Option<String>,
    pub states: IndexMap<EntityId, EntityState>,
    pub last_update: Option<Instant>,
}

impl InstanceRuntime {
    pub fn new(alias: Alias, url: String) -> Self {
        Self {
            alias,
            url,
            status: ConnStatus::Disconnected,
            last_error: None,
            states: IndexMap::new(),
            last_update: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum HaCommand {
    CallService {
        domain: String,
        service: String,
        service_data: Value,
        target: Value,
    },
    /// Backfill history for one entity over the past `hours` hours.
    FetchHistory { entity_id: EntityId, hours: u32 },
}
