use std::io::{stdout, Stdout};

use clap::Parser;
use color_eyre::Result;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

mod actions;
mod app;
mod config;
mod dashboard;
mod event;
mod ha;
mod screens;
mod ui;
mod util;

#[derive(Parser, Debug)]
#[command(name = "ha-tui", version, about = "Terminal UI for Home Assistant")]
struct Args {
    /// Path to config.yaml (default: XDG config dir / ha-tui / config.yaml)
    #[arg(long, value_name = "PATH")]
    config: Option<std::path::PathBuf>,
    /// Path to dashboards.yaml (default: XDG config dir / ha-tui / dashboards.yaml)
    #[arg(long, value_name = "PATH")]
    dashboards: Option<std::path::PathBuf>,
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let args = Args::parse();
    let _guard = util::logging::init()?;

    // rustls 0.23 needs an explicit CryptoProvider. Install ring once.
    let _ = rustls::crypto::ring::default_provider().install_default();

    // Restore the terminal on panic so a crash doesn't leave it broken.
    let panic_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore_terminal();
        panic_hook(info);
    }));

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    // Query the terminal for image protocol + font size BEFORE switching to
    // alt screen + raw mode — DCS responses don't round-trip reliably otherwise,
    // which forces ratatui-image to fall back to pixelated halfblocks.
    // Set `HA_TUI_IMAGE_PROTO=halfblocks|sixel|kitty|iterm2` to override the
    // auto-detected protocol (useful when a terminal advertises kitty but
    // doesn't actually render it).
    let picker = match ratatui_image::picker::Picker::from_query_stdio() {
        Ok(mut p) => {
            use ratatui_image::picker::ProtocolType;
            // iTerm2 advertises partial Kitty support but its native protocol
            // renders correctly while Kitty does not. Auto-prefer iterm2 when
            // running inside iTerm2.app.
            if std::env::var("TERM_PROGRAM")
                .map(|v| v == "iTerm.app")
                .unwrap_or(false)
                && p.protocol_type() != ProtocolType::Iterm2
            {
                tracing::info!(
                    "TERM_PROGRAM=iTerm.app detected — overriding {:?} → Iterm2",
                    p.protocol_type()
                );
                p.set_protocol_type(ProtocolType::Iterm2);
            }
            if let Ok(forced) = std::env::var("HA_TUI_IMAGE_PROTO") {
                let proto = match forced.to_ascii_lowercase().as_str() {
                    "halfblocks" => Some(ProtocolType::Halfblocks),
                    "sixel" => Some(ProtocolType::Sixel),
                    "kitty" => Some(ProtocolType::Kitty),
                    "iterm2" => Some(ProtocolType::Iterm2),
                    other => {
                        tracing::warn!(value = %other, "unknown HA_TUI_IMAGE_PROTO — ignored");
                        None
                    }
                };
                if let Some(proto) = proto {
                    p.set_protocol_type(proto);
                }
            }
            tracing::info!(
                protocol = ?p.protocol_type(),
                font_size = ?p.font_size(),
                "image picker initialised"
            );
            Some(p)
        }
        Err(e) => {
            tracing::warn!(error = %e, "image picker query failed — falling back to halfblocks");
            None
        }
    };

    let terminal = setup_terminal()?;
    let result = rt.block_on(app::run(terminal, picker, args.config, args.dashboards));
    restore_terminal()?;
    result
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen, EnableMouseCapture)?;
    let term = Terminal::new(CrosstermBackend::new(out))?;
    Ok(term)
}

fn restore_terminal() -> Result<()> {
    let mut out = stdout();
    execute!(out, DisableMouseCapture, LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}
