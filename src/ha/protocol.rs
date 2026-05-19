use serde::{Deserialize, Serialize};
use serde_json::Value;

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
}

/// Inbound server messages — discriminated by `type`.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ServerMsg {
    #[serde(rename = "auth_required")]
    AuthRequired { ha_version: Option<String> },
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
    Event { id: u64, event: HaEvent },
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
