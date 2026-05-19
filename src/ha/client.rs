use std::time::Duration;

use color_eyre::eyre::eyre;
use color_eyre::Result;
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, info, warn};

use crate::config::{Alias, InstanceConfig};
use crate::event::AppEvent;
use crate::ha::protocol::{ClientMsg, RawState, ServerMsg, StateChangedData};
use crate::ha::{ConnStatus, EntityState, HaCommand};

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

    // 1. Expect auth_required
    let first = read_msg(&mut stream).await?;
    if !matches!(first, ServerMsg::AuthRequired { .. }) {
        return Err(eyre!("expected auth_required, got {:?}", first));
    }

    // 2. Send auth
    let token = inst
        .token
        .as_deref()
        .ok_or_else(|| eyre!("token missing"))?;
    send(
        &mut sink,
        &ClientMsg::Auth {
            access_token: token,
        },
    )
    .await?;

    // 3. Expect auth_ok / auth_invalid
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

    // 4. Initial get_states + subscribe
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

    // 5. Main loop: forward events + handle cmds
    loop {
        tokio::select! {
            msg = read_msg(&mut stream) => {
                let msg = msg?;
                handle_server_msg(msg, get_id, &inst.alias, tx_app);
            }
            cmd = rx_cmd.recv() => {
                let Some(cmd) = cmd else { return Ok(()); };
                handle_cmd(cmd, &mut id_counter, &mut sink).await?;
            }
        }
    }
}

fn handle_server_msg(
    msg: ServerMsg,
    initial_get_id: u64,
    alias: &Alias,
    tx_app: &mpsc::UnboundedSender<AppEvent>,
) {
    match msg {
        ServerMsg::Result {
            id,
            success,
            result,
            ..
        } if id == initial_get_id && success => {
            // result is an array of RawState
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
            warn!(alias = %alias, id, ?error, "ha service/result error");
            let _ = tx_app.send(AppEvent::HaServiceError {
                instance: alias.clone(),
                error: error.to_string(),
            });
        }
        _ => debug!(alias = %alias, "ignored msg"),
    }
}

async fn handle_cmd(
    cmd: HaCommand,
    id_counter: &mut u64,
    sink: &mut (impl SinkExt<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin),
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
                // tokio-tungstenite auto-replies; ignore
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
    sink.send(Message::Text(json.into())).await?;
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

// Suppress unused warnings on json! during dev
const _: fn() = || {
    let _ = json!({});
    let _: Value = Value::Null;
};
