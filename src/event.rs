use chrono::{DateTime, Utc};

use crate::config::Alias;
use crate::ha::{ConnStatus, EntityId, EntityState};

#[derive(Debug, Clone)]
pub enum AppEvent {
    Tick,
    #[allow(dead_code)]
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
}
