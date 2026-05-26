pub mod client;
pub mod protocol;
pub mod registry;
pub mod rest;

pub use registry::{InstanceHandle, InstanceRegistry};

use indexmap::IndexMap;
use serde_json::Value;
use std::time::Instant;

use chrono::{DateTime, Utc};

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

#[derive(Debug, Clone, Copy)]
pub enum ImageFetchKind {
    Image,
    Camera,
    MediaPlayerThumbnail,
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // Hourly included for completeness; only Daily is wired up in UI
pub enum ForecastKind {
    Daily,
    Hourly,
}

#[derive(Debug, Clone)]
pub struct ForecastDay {
    pub datetime: DateTime<Utc>,
    pub condition: String,
    pub temperature: f64,
    pub templow: Option<f64>,
    #[allow(dead_code)]
    pub humidity: Option<f64>,
    #[allow(dead_code)]
    pub wind_speed: Option<f64>,
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
    /// Fetch image bytes from the HA REST API (image_proxy or camera_proxy).
    FetchImageBytes {
        entity: EntityId,
        kind: ImageFetchKind,
    },
    /// Request a weather forecast via `weather.get_forecasts` service call.
    GetWeatherForecast {
        entity: EntityId,
        kind: ForecastKind,
    },
}
