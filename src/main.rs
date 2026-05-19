use clap::Parser;
use color_eyre::Result;

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

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    rt.block_on(async move {
        let terminal = ratatui::init();
        let result = app::run(terminal, args.config, args.dashboards).await;
        ratatui::restore();
        result
    })
}
