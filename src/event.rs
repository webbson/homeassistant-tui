use chrono::{DateTime, Utc};

use crate::config::Alias;
use crate::ha::{ConnStatus, EntityId, EntityState, ForecastDay};
use crate::local_media::LocalMediaSnapshot;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum AppEvent {
    Tick,
    Quit,
    HaInitialStates {
        instance: Alias,
        states: Vec<EntityState>,
    },
    HaEntityUpdated {
        instance: Alias,
        state: EntityState,
    },
    HaConnStatus {
        instance: Alias,
        status: ConnStatus,
        error: Option<String>,
    },
    HaServiceError {
        instance: Alias,
        error: String,
    },
    HaHistory {
        instance: Alias,
        entity_id: EntityId,
        samples: Vec<(DateTime<Utc>, f64)>,
    },
    HaImageBytes {
        instance: Alias,
        entity: EntityId,
        result: Result<Vec<u8>, String>,
    },
    RefreshImageCard {
        instance: Alias,
        entity: EntityId,
    },
    HaWeatherForecast {
        instance: Alias,
        entity: EntityId,
        forecast: Vec<ForecastDay>,
    },
    RefreshWeatherForecast {
        instance: Alias,
        entity: EntityId,
    },
    UpdateAvailable {
        version: String,
    },
    RemoveInstance {
        alias: Alias,
    },
    LocalMediaUpdate(LocalMediaSnapshot),
    /// Cover art bytes loaded from a local file path; decoded into a protocol in the handler.
    LocalArtLoaded {
        path: std::path::PathBuf,
        bytes: Vec<u8>,
    },
}
