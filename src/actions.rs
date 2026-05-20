use serde_json::{json, Value};

use crate::config::Alias;
use crate::ha::{EntityId, HaCommand};

/// Domain-aware default action when Enter pressed on an entity.
pub fn default_action(entity_id: &EntityId) -> Option<HaCommand> {
    let domain = entity_id.split_once('.').map(|(d, _)| d)?;
    let (service, target) = match domain {
        "light" | "switch" | "input_boolean" | "fan" | "siren" => {
            ("toggle".to_string(), json!({ "entity_id": entity_id }))
        }
        "script" | "automation" | "scene" => {
            ("turn_on".to_string(), json!({ "entity_id": entity_id }))
        }
        "cover" => ("toggle".to_string(), json!({ "entity_id": entity_id })),
        "lock" => ("unlock".to_string(), json!({ "entity_id": entity_id })),
        _ => return None,
    };
    Some(HaCommand::CallService {
        domain: domain.to_string(),
        service,
        service_data: Value::Null,
        target,
    })
}

pub fn media_service_for_key(k: char) -> Option<&'static str> {
    match k {
        ' ' => Some("media_play_pause"),
        'n' => Some("media_next_track"),
        'p' => Some("media_previous_track"),
        '+' => Some("volume_up"),
        '-' => Some("volume_down"),
        'm' => Some("volume_mute"),
        _ => None,
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PendingAction {
    pub instance: Alias,
    pub entity_id: EntityId,
    pub command: HaCommand,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_toggles() {
        let cmd = default_action(&"light.kitchen".to_string()).unwrap();
        let HaCommand::CallService {
            domain, service, ..
        } = cmd
        else {
            panic!("expected CallService");
        };
        assert_eq!(domain, "light");
        assert_eq!(service, "toggle");
    }

    #[test]
    fn script_turns_on() {
        let cmd = default_action(&"script.morning".to_string()).unwrap();
        let HaCommand::CallService { service, .. } = cmd else {
            panic!("expected CallService");
        };
        assert_eq!(service, "turn_on");
    }

    #[test]
    fn unknown_domain_no_action() {
        assert!(default_action(&"sensor.temp".to_string()).is_none());
    }
}
