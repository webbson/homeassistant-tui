use std::collections::HashMap;
use std::time::Duration;

use chrono::{Duration as ChronoDuration, Utc};
use color_eyre::eyre::eyre;
use color_eyre::Result;
use futures::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, info, warn};

use crate::config::{Alias, InstanceConfig};
use crate::event::AppEvent;
use crate::ha::protocol::{
    weather_get_forecasts_payload, ClientMsg, RawState, ServerMsg, StateChangedData,
};
use crate::ha::{
    ConnStatus, EntityId, EntityState, ForecastDay, ForecastKind, HaCommand, ImageFetchKind,
};

pub fn spawn(
    inst: InstanceConfig,
    tx_app: mpsc::UnboundedSender<AppEvent>,
) -> mpsc::UnboundedSender<HaCommand> {
    let (tx_cmd, rx_cmd) = mpsc::unbounded_channel::<HaCommand>();
    tokio::spawn(async move {
        run_loop(inst, tx_app, rx_cmd).await;
    });
    tx_cmd
}

async fn run_loop(
    inst: InstanceConfig,
    tx_app: mpsc::UnboundedSender<AppEvent>,
    mut rx_cmd: mpsc::UnboundedReceiver<HaCommand>,
) {
    let mut backoff = Duration::from_secs(1);
    loop {
        emit_status(&tx_app, &inst.alias, ConnStatus::Connecting, None);
        match connect_once(&inst, &tx_app, &mut rx_cmd).await {
            Ok(()) => {
                info!(alias = %inst.alias, "ha client exited cleanly");
                emit_status(&tx_app, &inst.alias, ConnStatus::Disconnected, None);
                return;
            }
            Err(e) => {
                warn!(alias = %inst.alias, error = %e, "ha client error — reconnecting");
                emit_status(
                    &tx_app,
                    &inst.alias,
                    ConnStatus::Failed,
                    Some(e.to_string()),
                );
            }
        }
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(Duration::from_secs(30));
    }
}

async fn connect_once(
    inst: &InstanceConfig,
    tx_app: &mpsc::UnboundedSender<AppEvent>,
    rx_cmd: &mut mpsc::UnboundedReceiver<HaCommand>,
) -> Result<()> {
    let (ws_stream, _) = tokio_tungstenite::connect_async(&inst.url).await?;
    let (mut sink, mut stream) = ws_stream.split();
    emit_status(tx_app, &inst.alias, ConnStatus::Authenticating, None);

    let first = read_msg(&mut stream).await?;
    if !matches!(first, ServerMsg::AuthRequired { .. }) {
        return Err(eyre!("expected auth_required, got {:?}", first));
    }

    let token = inst
        .token
        .as_deref()
        .ok_or_else(|| eyre!("token missing"))?
        .to_string();
    let ws_url = inst.url.clone();
    send(
        &mut sink,
        &ClientMsg::Auth {
            access_token: &token,
        },
    )
    .await?;

    match read_msg(&mut stream).await? {
        ServerMsg::AuthOk { ha_version } => {
            info!(alias = %inst.alias, ?ha_version, "authenticated");
        }
        ServerMsg::AuthInvalid { message } => {
            return Err(eyre!(
                "auth invalid: {}",
                message.unwrap_or_else(|| "no detail".into())
            ));
        }
        other => return Err(eyre!("unexpected msg after auth: {:?}", other)),
    }

    emit_status(tx_app, &inst.alias, ConnStatus::Connected, None);

    let mut id_counter: u64 = 1;
    let get_id = next_id(&mut id_counter);
    send(&mut sink, &ClientMsg::GetStates { id: get_id }).await?;
    let sub_id = next_id(&mut id_counter);
    send(
        &mut sink,
        &ClientMsg::SubscribeEvents {
            id: sub_id,
            event_type: Some("state_changed"),
        },
    )
    .await?;

    // Track id -> entity_id for pending history requests
    let mut pending_history: HashMap<u64, EntityId> = HashMap::new();
    // Track id -> (entity_id, kind) for pending weather forecast requests
    let mut pending_weather: HashMap<u64, (EntityId, ForecastKind)> = HashMap::new();

    loop {
        tokio::select! {
            msg = read_msg(&mut stream) => {
                let msg = msg?;
                handle_server_msg(msg, get_id, &inst.alias, tx_app, &mut pending_history, &mut pending_weather);
            }
            cmd = rx_cmd.recv() => {
                let Some(cmd) = cmd else { return Ok(()); };
                handle_cmd(
                    cmd,
                    &mut id_counter,
                    &mut sink,
                    &mut pending_history,
                    &mut pending_weather,
                    &ws_url,
                    &token,
                    &inst.alias,
                    tx_app,
                )
                .await?;
            }
        }
    }
}

fn handle_server_msg(
    msg: ServerMsg,
    initial_get_id: u64,
    alias: &Alias,
    tx_app: &mpsc::UnboundedSender<AppEvent>,
    pending_history: &mut HashMap<u64, EntityId>,
    pending_weather: &mut HashMap<u64, (EntityId, ForecastKind)>,
) {
    match msg {
        ServerMsg::Result {
            id,
            success,
            result,
            ..
        } if id == initial_get_id && success => {
            if let Some(arr) = result.as_array() {
                let mut states = Vec::with_capacity(arr.len());
                for v in arr {
                    if let Ok(rs) = serde_json::from_value::<RawState>(v.clone()) {
                        states.push(EntityState::from(rs));
                    }
                }
                let _ = tx_app.send(AppEvent::HaInitialStates {
                    instance: alias.clone(),
                    states,
                });
            }
        }
        ServerMsg::Result {
            id,
            success: true,
            result,
            ..
        } if pending_history.contains_key(&id) => {
            let entity_id = pending_history.remove(&id).unwrap();
            let samples = parse_history(&result, &entity_id);
            let _ = tx_app.send(AppEvent::HaHistory {
                instance: alias.clone(),
                entity_id,
                samples,
            });
        }
        ServerMsg::Result {
            id,
            success: true,
            result,
            ..
        } if pending_weather.contains_key(&id) => {
            let (entity_id, _kind) = pending_weather.remove(&id).unwrap();
            let forecast = parse_weather_forecast(&result, &entity_id);
            let _ = tx_app.send(AppEvent::HaWeatherForecast {
                instance: alias.clone(),
                entity: entity_id,
                forecast,
            });
        }
        ServerMsg::Event { event, .. } if event.event_type == "state_changed" => {
            if let Ok(data) = serde_json::from_value::<StateChangedData>(event.data) {
                if let Some(new) = data.new_state {
                    let _ = tx_app.send(AppEvent::HaEntityUpdated {
                        instance: alias.clone(),
                        state: EntityState::from(new),
                    });
                }
            }
        }
        ServerMsg::Result {
            success: false,
            error,
            id,
            ..
        } => {
            pending_history.remove(&id);
            pending_weather.remove(&id);
            warn!(alias = %alias, id, ?error, "ha service/result error");
            let _ = tx_app.send(AppEvent::HaServiceError {
                instance: alias.clone(),
                error: error.to_string(),
            });
        }
        _ => debug!(alias = %alias, "ignored msg"),
    }
}

/// Parse HA `weather.get_forecasts` service_response for one entity.
/// Result shape: { "response": { "<entity_id>": { "forecast": [...] } } }
/// Treats missing/null/malformed response as empty — never panics.
fn parse_weather_forecast(result: &Value, entity_id: &str) -> Vec<ForecastDay> {
    #[derive(serde::Deserialize)]
    struct RawForecastItem {
        datetime: String,
        condition: String,
        temperature: f64,
        templow: Option<f64>,
        humidity: Option<f64>,
        wind_speed: Option<f64>,
    }

    let arr = result
        .get("response")
        .and_then(|r| r.get(entity_id))
        .and_then(|e| e.get("forecast"))
        .and_then(|f| f.as_array());

    let arr = match arr {
        Some(a) => a,
        None => return Vec::new(),
    };

    arr.iter()
        .filter_map(|v| {
            let item: RawForecastItem = serde_json::from_value(v.clone()).ok()?;
            let dt = chrono::DateTime::parse_from_rfc3339(&item.datetime)
                .map(|d| d.with_timezone(&Utc))
                .ok()?;
            Some(ForecastDay {
                datetime: dt,
                condition: item.condition,
                temperature: item.temperature,
                templow: item.templow,
                humidity: item.humidity,
                wind_speed: item.wind_speed,
            })
        })
        .collect()
}

/// Parse HA history/history_during_period result for a single entity.
/// Result shape: { "<entity_id>": [ { "s": "<state>", "lu": <ts_seconds_f64> }, ... ] }
fn parse_history(result: &Value, entity_id: &str) -> Vec<(chrono::DateTime<Utc>, f64)> {
    let arr = match result.get(entity_id).and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return Vec::new(),
    };
    arr.iter()
        .filter_map(|v| {
            let state = v.get("s")?.as_str()?;
            let val: f64 = state.parse().ok()?;
            let ts_secs = v.get("lu")?.as_f64()?;
            let dt = chrono::DateTime::<Utc>::from_timestamp(
                ts_secs as i64,
                ((ts_secs.fract()) * 1e9) as u32,
            )?;
            Some((dt, val))
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
async fn handle_cmd(
    cmd: HaCommand,
    id_counter: &mut u64,
    sink: &mut (impl SinkExt<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin),
    pending_history: &mut HashMap<u64, EntityId>,
    pending_weather: &mut HashMap<u64, (EntityId, ForecastKind)>,
    ws_url: &str,
    token: &str,
    alias: &Alias,
    tx_app: &mpsc::UnboundedSender<AppEvent>,
) -> Result<()> {
    match cmd {
        HaCommand::CallService {
            domain,
            service,
            service_data,
            target,
        } => {
            let id = next_id(id_counter);
            send(
                sink,
                &ClientMsg::CallService {
                    id,
                    domain: &domain,
                    service: &service,
                    service_data,
                    target,
                },
            )
            .await?;
        }
        HaCommand::FetchHistory { entity_id, hours } => {
            let id = next_id(id_counter);
            let end = Utc::now();
            let start = end - ChronoDuration::hours(i64::from(hours));
            pending_history.insert(id, entity_id.clone());
            send(
                sink,
                &ClientMsg::HistoryDuringPeriod {
                    id,
                    start_time: start.to_rfc3339(),
                    end_time: end.to_rfc3339(),
                    entity_ids: vec![entity_id],
                    minimal_response: true,
                    no_attributes: true,
                },
            )
            .await?;
        }
        HaCommand::FetchImageBytes { entity, kind } => {
            // Spawn a separate task so the WS loop is not blocked by HTTP.
            let base_url = ws_url.to_string();
            let tok = token.to_string();
            let alias_clone = alias.clone();
            let entity_clone = entity.clone();
            let tx_clone = tx_app.clone();
            tokio::spawn(async move {
                let bytes_res = match kind {
                    ImageFetchKind::Image => {
                        crate::ha::rest::fetch_image_proxy(&base_url, entity_clone.as_str(), &tok)
                            .await
                    }
                    ImageFetchKind::Camera => {
                        crate::ha::rest::fetch_camera_proxy(&base_url, entity_clone.as_str(), &tok)
                            .await
                    }
                };
                let _ = tx_clone.send(crate::event::AppEvent::HaImageBytes {
                    instance: alias_clone,
                    entity: entity_clone,
                    result: bytes_res,
                });
            });
        }
        HaCommand::GetWeatherForecast { entity, kind } => {
            let id = next_id(id_counter);
            pending_weather.insert(id, (entity.clone(), kind));
            let payload = weather_get_forecasts_payload(id, &entity, kind);
            let json = serde_json::to_string(&payload)?;
            sink.send(Message::Text(json)).await?;
        }
    }
    Ok(())
}

async fn read_msg<S>(stream: &mut S) -> Result<ServerMsg>
where
    S: StreamExt<Item = std::result::Result<Message, tokio_tungstenite::tungstenite::Error>>
        + Unpin,
{
    loop {
        let m = stream
            .next()
            .await
            .ok_or_else(|| eyre!("ws stream ended"))??;
        match m {
            Message::Text(t) => {
                let parsed: ServerMsg = serde_json::from_str(&t)?;
                return Ok(parsed);
            }
            Message::Binary(b) => {
                let parsed: ServerMsg = serde_json::from_slice(&b)?;
                return Ok(parsed);
            }
            Message::Ping(p) => {
                debug!(?p, "ping");
            }
            Message::Pong(_) | Message::Frame(_) => {}
            Message::Close(_) => return Err(eyre!("ws closed by server")),
        }
    }
}

async fn send<S>(sink: &mut S, msg: &ClientMsg<'_>) -> Result<()>
where
    S: SinkExt<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    let json = serde_json::to_string(msg)?;
    sink.send(Message::Text(json)).await?;
    Ok(())
}

fn next_id(c: &mut u64) -> u64 {
    *c += 1;
    *c
}

fn emit_status(
    tx_app: &mpsc::UnboundedSender<AppEvent>,
    alias: &Alias,
    status: ConnStatus,
    err: Option<String>,
) {
    let _ = tx_app.send(AppEvent::HaConnStatus {
        instance: alias.clone(),
        status,
        error: err,
    });
}
