use serde_json::{json, Value};

use crate::config::Alias;
use crate::ha::{EntityId, EntityState, HaCommand};

/// Domain-aware default action when Enter pressed on an entity.
pub fn default_action(entity_id: &EntityId, state: Option<&EntityState>) -> Option<HaCommand> {
    let domain = entity_id.split_once('.').map(|(d, _)| d)?;
    let (service, target) = match domain {
        "light" | "switch" | "input_boolean" | "fan" | "siren" | "cover" => {
            ("toggle".to_string(), json!({ "entity_id": entity_id }))
        }
        "script" | "automation" | "scene" => {
            ("turn_on".to_string(), json!({ "entity_id": entity_id }))
        }
        "button" | "input_button" => {
            ("press".to_string(), json!({ "entity_id": entity_id }))
        }
        "lock" => {
            // If locked → unlock; otherwise (unlocked, jammed, unknown) → lock.
            // Exception: if state is completely absent (None), preserve legacy unlock behavior.
            let svc = match state.map(|s| s.state.as_str()) {
                Some("locked") => "unlock",
                Some(_) => "lock",
                None => "unlock",
            };
            (svc.to_string(), json!({ "entity_id": entity_id }))
        }
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
    use crate::ha::EntityState;

    fn make_state(state_str: &str) -> EntityState {
        EntityState {
            entity_id: "test.entity".to_string(),
            state: state_str.to_string(),
            attributes: serde_json::Value::Object(Default::default()),
            last_changed: None,
            last_updated: None,
        }
    }

    #[test]
    fn light_toggles() {
        let cmd = default_action(&"light.kitchen".to_string(), None).unwrap();
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
        let cmd = default_action(&"script.morning".to_string(), None).unwrap();
        let HaCommand::CallService { service, .. } = cmd else {
            panic!("expected CallService");
        };
        assert_eq!(service, "turn_on");
    }

    #[test]
    fn unknown_domain_no_action() {
        assert!(default_action(&"sensor.temp".to_string(), None).is_none());
    }

    #[test]
    fn button_presses() {
        let cmd = default_action(&"button.doorbell".to_string(), None).unwrap();
        let HaCommand::CallService { domain, service, .. } = cmd else {
            panic!("expected CallService");
        };
        assert_eq!(domain, "button");
        assert_eq!(service, "press");
    }

    #[test]
    fn input_button_presses() {
        let cmd = default_action(&"input_button.foo".to_string(), None).unwrap();
        let HaCommand::CallService { service, .. } = cmd else {
            panic!("expected CallService");
        };
        assert_eq!(service, "press");
    }

    #[test]
    fn lock_when_locked_unlocks() {
        let s = make_state("locked");
        let cmd = default_action(&"lock.front_door".to_string(), Some(&s)).unwrap();
        let HaCommand::CallService { service, .. } = cmd else {
            panic!("expected CallService");
        };
        assert_eq!(service, "unlock");
    }

    #[test]
    fn lock_when_unlocked_locks() {
        let s = make_state("unlocked");
        let cmd = default_action(&"lock.front_door".to_string(), Some(&s)).unwrap();
        let HaCommand::CallService { service, .. } = cmd else {
            panic!("expected CallService");
        };
        assert_eq!(service, "lock");
    }

    #[test]
    fn lock_when_state_none_unlocks() {
        let cmd = default_action(&"lock.front_door".to_string(), None).unwrap();
        let HaCommand::CallService { service, .. } = cmd else {
            panic!("expected CallService");
        };
        assert_eq!(service, "unlock");
    }

    #[test]
    fn existing_light_toggle_still_works() {
        let cmd = default_action(&"light.kitchen".to_string(), None).unwrap();
        let HaCommand::CallService { service, .. } = cmd else {
            panic!("expected CallService");
        };
        assert_eq!(service, "toggle");
    }
}
