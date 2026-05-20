# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Common commands

```bash
cargo build              # debug build
cargo build --release    # optimized binary at target/release/ha-tui
cargo test               # unit tests (no real HA required)
cargo test <name>        # single test by substring, e.g. `cargo test glob_basic`
cargo clippy --all-targets   # lint
cargo fmt                # rustfmt; CI-style check: `cargo fmt -- --check`
cargo run -- --config <path> --dashboards <path>
```

Toolchain is pinned via `rust-toolchain.toml` (stable + rustfmt + clippy). No vendored deps; `cargo build` fetches from crates.io.

Run logs land in the XDG data dir (`~/Library/Application Support/ha-tui/log/` on macOS, `~/.local/share/ha-tui/log/` on Linux). Config and dashboards live at `~/.config/ha-tui/{config,dashboards}.yaml` on both platforms — `util::paths` deliberately ignores macOS's Apple-style config dir so the path is portable.

## High-level architecture

**Async event-loop owning all UI state.** `app::run` builds a single `App` struct, then loops on `tokio::select!` over three sources:
1. `crossterm::EventStream` — keyboard + mouse from terminal
2. `mpsc::UnboundedReceiver<AppEvent>` — fan-in from one tokio task per HA instance
3. `tokio::time::interval` — 250 ms ticks (drives the ticker animation + future periodic work)

After each handled event, `terminal.draw(|f| ui::draw(f, &app))?`. `App` is mutated synchronously between awaits, so there are no locks anywhere.

**One tokio task per HA instance.** `ha::client::spawn` connects to an instance over `tokio-tungstenite`, runs the auth handshake, sends `get_states` + `subscribe_events`, then loops forwarding state changes upstream as `AppEvent::Ha*` variants and accepting `HaCommand`s (service calls, history backfills) from a per-instance `mpsc::UnboundedSender<HaCommand>` stored in `InstanceRegistry`. Reconnect uses exponential backoff. The HA WebSocket protocol layer is hand-rolled in `ha::protocol` — `hass-rs` was rejected because it's single-instance oriented.

**Multi-instance design**: entities are never merged across instances. The internal key is `(InstanceAlias, EntityId)`. Per-instance color (auto-cycled from an 8-color palette, or `color:` override in config) is applied everywhere an entity is rendered. `ui::theme::Theme` is the single source of truth.

**Screens** (in `screens/mod.rs::Screen`): `Entities`, `Instances`, `Dashboard { idx, selected_card, sub_index }`, `Editor`. The Dashboard screen carries `sub_index` because EntityList/FilteredEntityList cards have internal row selection — j/k inside such a card moves the sub-cursor, h/l moves between cards. `trigger_default_action` (Enter) routes to the right level based on card kind.

**Editor is mode-driven**: `EditorState` holds `mode: EditorMode` plus an undo stack of full `Dashboard` snapshots. Every modal interaction (picking type/instance/entity, renaming, editing window or query, confirming destructive ops) is its own variant. `m` opens a context-sensitive menu (`card_menu_items` / `dashboard_menu_items`); the previous direct shortcuts (T/C/W/R/G) were removed in favor of menu-driven discovery. Editor commits go through `EditorState::add_card`, which checks `edit_target: Option<usize>` — if `Some`, the in-progress flow replaces an existing card in place; if `None`, it appends.

**Cards** (in `dashboard::CardKind`): `Entity` (with `ticker: bool`), `Toggle`, `Gauge` (uses HA's `display_precision` attribute), `Sparkline` (Chart widget with axes; backfilled from `history/history_during_period`; downsampled to card width), `Text`, `EntityList` (static list), `FilteredEntityList` (dynamic, evaluates a `glob[filter][filter]` query at render time — parser in `dashboard::query`).

**YAML round-trip caveat**: `serde_yaml` does not preserve comments on save. Editor saves overwrite the dashboards file. The parent directory is created automatically (`persist::save` mkdir-p's it) so first-run users don't need to pre-create `~/.config/ha-tui/`.

**Hit testing**: ratatui has no built-in mouse hit-testing. Mouse events convert to grid cells via `app::mouse_to_cell`, which is enough for the editor (the only screen using the mouse so far). Mouse capture is enabled with `EnableMouseCapture` on startup and disabled on exit.

## Critical implementation details

- **`Stylize` trait must be imported explicitly** (e.g. `use ratatui::style::Stylize;`) wherever `.bold()` / `.dim()` / `.fg(...)` chain calls are made. Rustc 1.95's `unused_imports` lint can wrongly flag it; ignore that specific warning — removing the import breaks the build.
- **History RingBuf** has cap 8192. On `HaInitialStates`, every Sparkline card on every dashboard for that instance triggers a `FetchHistory` command sized by the card's `window` field. `RingBuf::fill_from` replaces contents; subsequent live `state_changed` events append normally.
- **Tokens** can be `${ENV_VAR}` (expanded at load) or `token_file: ~/path/to/file` (read at load). Never log token values.
- **Default new dashboard grid is 12×24**, but YAML can specify anything via `grid: { cols, rows }`.
- **Entity domains that have a default action** are wired in `actions::default_action`: light/switch/input_boolean/fan/siren/cover → `toggle`, script/automation/scene → `turn_on`, lock → `unlock`. Other domains return `None` and Enter shows a "no default action" status.

## User-preference notes (from global CLAUDE.md / RTK.md)

- Prefer editing existing files over creating new ones.
- Run `cargo test` before declaring a task complete.
- Don't add features, refactor, or introduce abstractions beyond what the task requires.
- Don't write comments that explain *what* the code does — only *why* when non-obvious.
- Keep `CHANGELOG.md` updated. Every user-visible change → bullet under `## [Unreleased]` in the same commit. The `release` skill cuts Unreleased into a versioned section at tag time.
