use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ha::ForecastKind;

/// Outgoing client messages. HA WebSocket protocol.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg<'a> {
    Auth {
        access_token: &'a str,
    },
    SubscribeEvents {
        id: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        event_type: Option<&'a str>,
    },
    GetStates {
        id: u64,
    },
    CallService {
        id: u64,
        domain: &'a str,
        service: &'a str,
        #[serde(skip_serializing_if = "Value::is_null")]
        service_data: Value,
        #[serde(skip_serializing_if = "Value::is_null")]
        target: Value,
    },
    /// history/history_during_period — backfill state samples for one or more entities.
    #[serde(rename = "history/history_during_period")]
    HistoryDuringPeriod {
        id: u64,
        start_time: String,
        end_time: String,
        entity_ids: Vec<String>,
        minimal_response: bool,
        no_attributes: bool,
    },
}

/// Inbound server messages — discriminated by `type`.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ServerMsg {
    #[serde(rename = "auth_required")]
    AuthRequired {
        #[allow(dead_code)]
        ha_version: Option<String>,
    },
    #[serde(rename = "auth_ok")]
    AuthOk { ha_version: Option<String> },
    #[serde(rename = "auth_invalid")]
    AuthInvalid { message: Option<String> },
    #[serde(rename = "result")]
    Result {
        id: u64,
        success: bool,
        #[serde(default)]
        result: Value,
        #[serde(default)]
        error: Value,
    },
    #[serde(rename = "event")]
    Event {
        #[allow(dead_code)]
        id: u64,
        event: HaEvent,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
pub struct HaEvent {
    pub event_type: String,
    pub data: Value,
}

#[derive(Debug, Deserialize)]
pub struct StateChangedData {
    #[allow(dead_code)]
    pub entity_id: String,
    pub new_state: Option<RawState>,
    #[allow(dead_code)]
    pub old_state: Option<RawState>,
}

#[derive(Debug, Deserialize)]
pub struct RawState {
    pub entity_id: String,
    pub state: String,
    #[serde(default)]
    pub attributes: Value,
    #[serde(default)]
    pub last_changed: Option<String>,
    #[serde(default)]
    pub last_updated: Option<String>,
}

impl From<RawState> for crate::ha::EntityState {
    fn from(r: RawState) -> Self {
        Self {
            entity_id: r.entity_id,
            state: r.state,
            attributes: r.attributes,
            last_changed: r.last_changed,
            last_updated: r.last_updated,
        }
    }
}

/// Build a `call_service` payload for `weather.get_forecasts` with `return_response: true`.
pub fn weather_get_forecasts_payload(id: u64, entity_id: &str, kind: ForecastKind) -> Value {
    let kind_str = match kind {
        ForecastKind::Daily => "daily",
        ForecastKind::Hourly => "hourly",
    };
    serde_json::json!({
        "id": id,
        "type": "call_service",
        "domain": "weather",
        "service": "get_forecasts",
        "service_data": { "type": kind_str },
        "target": { "entity_id": entity_id },
        "return_response": true,
    })
}
