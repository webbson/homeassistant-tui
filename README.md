# ha-tui

Terminal UI for [Home Assistant](https://www.home-assistant.io/). Multi-instance, color-coded entities, YAML-defined dashboards, interactive dashboard editor with **mouse + keyboard**. Written in Rust on top of [Ratatui](https://ratatui.rs).

## Features

- Connect to multiple HA instances at once over WebSocket (long-lived access tokens).
- Color-code every entity by the instance it came from.
- Browse entities live; trigger domain-aware default actions with Enter (toggle lights / switches / covers, run scripts, etc.).
- Compose dashboards in YAML with 12 card types — or use the in-app editor.
- Interactive dashboard editor: cursor + selection model, undo/redo, save back to YAML. Works with keyboard **or** mouse.
- Logs to `~/.local/share/ha-tui/log/` (XDG-aware) so terminal output stays clean.

## Install

### Homebrew (macOS & Linux)

```bash
brew install webbson/tap/ha-tui
```

### Shell installer (macOS & Linux)

Grab the script linked from the [latest release](https://github.com/webbson/homeassistant-tui/releases/latest); it drops the binary in `~/.cargo/bin`.

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/webbson/homeassistant-tui/releases/latest/download/ha-tui-installer.sh | sh
```

### Prebuilt tarballs

Download from the [releases page](https://github.com/webbson/homeassistant-tui/releases/latest) — macOS arm64/x64 and Linux arm64/x64 are published per tag.

### From source

```bash
# Rust toolchain (one-time)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build
cargo build --release
# Optional: install to ~/.cargo/bin
cargo install --path .
```

## Configure

On first run, ha-tui auto-creates `~/.config/ha-tui/config.yaml` and `dashboards.yaml` if absent. Add your HA instance URL and a long-lived access token (HA **Profile → Long-Lived Access Tokens**) to `config.yaml` to connect.

See [docs/configuration.md](docs/configuration.md) for the full config schema, token file support, per-instance color, and file locations.

## Run

```bash
ha-tui
# or
ha-tui --config /path/to/config.yaml --dashboards /path/to/dashboards.yaml
```

## Keys

### Global

| Key | Action |
|---|---|
| `Esc` | Quit (or close current overlay/editor first) |
| `E` | Entity search modal |
| `i` | Instance list modal |
| `1`..`9` | Jump to dashboard N |
| `e` | Edit the currently shown dashboard |
| `?` | Help overlay |

### Entity search modal

| Key | Action |
|---|---|
| type | Fuzzy filter on `entity_id` |
| `↓`/`↑` or `PageUp`/`PageDown` | Navigate |
| `f` | Cycle instance filter (all → first → next → …) |
| `Enter` | Domain-default action (toggle lights/switches, turn_on scripts/scenes, etc.) |
| `Esc` | Close modal |

### Dashboard editor

| Key | Action |
|---|---|
| `hjkl` / arrows | Move cursor |
| `Enter` (no selection) | Select card under cursor |
| `Enter` (with selection) | Move selected card to cursor; deselect |
| `HJKL` | Resize selected card (left/down/up/right) |
| `a` | Add card (menu-driven type picker, then entity/options) |
| `m` | Context menu for selected card or dashboard |
| `d` | Delete selected card (confirm) |
| `u` | Undo |
| `s` | Save dashboard YAML |
| `Esc` | Exit editor (confirm if unsaved) |

Mouse in editor:
- Click a cell → select the card there.
- Click + drag a card → move it.
- Mouse cursor also drives the cursor cell — keyboard and mouse share state.

## Dashboards

Dashboards are edited in-app — press `e` to open the editor. For hand-editing YAML or the full card-type reference (12 types: entity, toggle, gauge, graph, text, entity_list, filtered_entity_list, clock, statistics, media_player, image, weather), see [docs/dashboard-yaml.md](docs/dashboard-yaml.md). A working example lives at [`config/dashboards.example.yaml`](config/dashboards.example.yaml).

## Terminal requirements

- Linux / macOS: any modern terminal (iTerm2, Alacritty, kitty, WezTerm, gnome-terminal) works.
- 256-color or true-color recommended for the instance color palette.
- Some weather glyphs (☀ ⛅ ☁ 🌧 ❄ 🌫 ⛈) require a font with emoji/Unicode coverage (e.g. Nerd Fonts, Iosevka, or any system font on macOS/Linux).

## Architecture

- `src/main.rs` — entry; tokio runtime; ratatui init/restore.
- `src/app.rs` — `App` state, async event loop (`tokio::select!` over crossterm `EventStream`, mpsc `AppEvent` channel, tick).
- `src/ha/{client,protocol,registry}.rs` — one tokio task per instance, custom thin Home Assistant WebSocket protocol layer over `tokio-tungstenite`.
- `src/dashboard/{mod,layout,persist,editor}.rs` — dashboard model, grid → Rect math, YAML round-trip, editor reducer with undo stack.
- `src/screens/` + `src/ui/widgets/` — per-screen draw logic and reusable widgets.
- `src/ui/theme.rs` — instance color palette resolver.
- `src/actions.rs` — domain-aware default action for `Enter`.

## Status

This is an early build (v0.1). Implemented milestones:

- [x] Connect + auth to one or more HA instances
- [x] Live entity browser with color-coded multi-instance view
- [x] Service calls via Enter (domain defaults)
- [x] YAML dashboards with 12 card types
- [x] Interactive editor (mouse + keyboard, undo, save)
- [x] History backfill (sparkline + statistics cards)
- [x] Weather forecasts via `weather.get_forecasts`
- [x] Image / camera proxy cards
- [ ] Free-form service-call dialog
- [ ] OAuth (refresh-token) auth flow
- [ ] Comment-preserving YAML save

## License

MIT
