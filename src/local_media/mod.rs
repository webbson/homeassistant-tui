use std::path::PathBuf;

use tokio::sync::mpsc;

use crate::event::AppEvent;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;

#[derive(Debug, Clone, Default)]
pub struct LocalMediaSnapshot {
    /// Name of the source app (e.g. "Music", "Spotify", MPRIS bus suffix).
    /// Reserved for future display; not yet rendered.
    #[allow(dead_code)]
    pub player_name: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub position_secs: Option<f64>,
    pub duration_secs: Option<f64>,
    pub volume_0_1: Option<f64>,
    pub is_muted: bool,
    pub is_playing: bool,
    pub art_path: Option<PathBuf>,
}

#[derive(Debug)]
pub enum LocalCommand {
    PlayPause,
    Next,
    Prev,
    VolumeUp,
    VolumeDown,
    Mute,
}

/// Spawn a background task that polls the local media player every second.
/// Returns a sender for sending commands to the player.
/// The task emits `AppEvent::LocalMediaUpdate` on every poll cycle.
pub fn spawn(tx: mpsc::UnboundedSender<AppEvent>) -> mpsc::UnboundedSender<LocalCommand> {
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<LocalCommand>();

    tokio::spawn(poll_loop(tx, cmd_rx));

    cmd_tx
}

async fn poll_loop(
    tx: mpsc::UnboundedSender<AppEvent>,
    mut cmd_rx: mpsc::UnboundedReceiver<LocalCommand>,
) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
    loop {
        tokio::select! {
            _ = interval.tick() => {
                let snapshot = fetch_snapshot().await;
                let _ = tx.send(AppEvent::LocalMediaUpdate(snapshot));
            }
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(c) => { send_command(c).await; }
                    None => break,
                }
            }
        }
    }
}

#[cfg(target_os = "macos")]
async fn fetch_snapshot() -> LocalMediaSnapshot {
    macos::fetch_snapshot().await
}

#[cfg(target_os = "linux")]
async fn fetch_snapshot() -> LocalMediaSnapshot {
    linux::fetch_snapshot().await
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
async fn fetch_snapshot() -> LocalMediaSnapshot {
    LocalMediaSnapshot::default()
}

#[cfg(target_os = "macos")]
async fn send_command(cmd: LocalCommand) {
    macos::send_command(cmd).await;
}

#[cfg(target_os = "linux")]
async fn send_command(cmd: LocalCommand) {
    linux::send_command(cmd).await;
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
async fn send_command(_cmd: LocalCommand) {}
