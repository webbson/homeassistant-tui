# ha-tui

Terminal UI for [Home Assistant](https://www.home-assistant.io/). Multi-instance, color-coded entities, YAML-defined dashboards, interactive dashboard editor with **mouse + keyboard**. Written in Rust on top of [Ratatui](https://ratatui.rs).

## Features

- Connect to multiple HA instances at once over WebSocket (long-lived access tokens).
- Color-code every entity by the instance it came from.
- Browse entities live; trigger domain-aware default actions with Enter (toggle lights / switches / covers, run scripts, etc.).
- Compose dashboards in YAML: 5 card types — `entity`, `toggle`, `gauge`, `sparkline`, `text`.
- Interactive dashboard editor: cursor + selection model, undo/redo, save back to YAML. Works with keyboard **or** mouse.
- Logs to `~/.local/share/ha-tui/log/` (XDG-aware) so terminal output stays clean.

## Install

```bash
# Rust toolchain (one-time)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build
cargo build --release
# Optional: install to ~/.cargo/bin
cargo install --path .
```

## Configure

Copy the example files into your XDG config dir — `~/.config/ha-tui/` on both Linux and macOS:

```bash
mkdir -p ~/.config/ha-tui
cp config/config.example.yaml      ~/.config/ha-tui/config.yaml
cp config/dashboards.example.yaml  ~/.config/ha-tui/dashboards.yaml
```

Edit `config.yaml`:

```yaml
instances:
  - alias: home
    url: "wss://ha.example.com/api/websocket"
    token: "${HA_HOME_TOKEN}"     # env-expand at load
  - alias: cabin
    url: "ws://192.168.50.10:8123/api/websocket"
    token_file: "~/.config/ha-tui/cabin.token"
    color: "yellow"               # optional override; otherwise auto-assigned
log_level: "info"
```

Tokens are created in HA at **Profile → Long-Lived Access Tokens**.

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
| `q` / `Esc` | Quit |
| `E` | Entity browser |
| `i` | Instances screen |
| `1`..`9` | Jump to dashboard N |
| `e` | Edit the currently shown dashboard |
| `?` | Help overlay (TBD) |

### Entity browser

| Key | Action |
|---|---|
| `j`/`k` or `↓`/`↑` | Navigate |
| `PageUp`/`PageDown` | Jump 10 |
| `Home`/`End` | First / last |
| `f` | Cycle instance filter (all → first → next → …) |
| `Enter` | Domain-default action (toggle lights/switches, turn_on scripts/scenes, etc.) |

### Dashboard editor

| Key | Action |
|---|---|
| `hjkl` / arrows | Move cursor |
| `Enter` (no selection) | Select card under cursor |
| `Enter` (with selection) | Move selected card to cursor; deselect |
| `HJKL` | Resize selected card (left/down/up/right) |
| `a` | Add card (then pick type `1..5`, then enter `entity` or `instance/entity`) |
| `d` | Delete selected card (confirm) |
| `u` | Undo |
| `s` | Save dashboard YAML |
| `Esc` | Exit editor (confirm if unsaved) |

Mouse in editor:
- Click a cell → select the card there.
- Click + drag a card → move it.
- Mouse cursor also drives the cursor cell — keyboard and mouse share state.

## Dashboard YAML

```yaml
dashboards:
  - name: "Home"
    grid: { cols: 12, rows: 8 }
    cards:
      - type: entity
        instance: home
        entity: light.kitchen
        title: "Kitchen Light"
        pos: { col: 0, row: 0, w: 3, h: 2 }
      - type: gauge
        instance: cabin
        entity: sensor.cabin_temp
        min: -20
        max: 30
        unit: "°C"
        pos: { col: 6, row: 0, w: 3, h: 3 }
      - type: sparkline
        instance: cabin
        entity: sensor.power
        window: "1h"
        pos: { col: 0, row: 2, w: 6, h: 3 }
      - type: text
        markdown: "## Notes"
        pos: { col: 9, row: 0, w: 3, h: 4 }
```

`pos` units are grid cells, not characters.

## Terminal requirements

- Linux / macOS: any modern terminal (iTerm2, Alacritty, kitty, WezTerm, gnome-terminal) works.
- Windows: use **Windows Terminal** or Alacritty. The legacy `conhost` terminal does not support SGR mouse encoding.
- 256-color or true-color recommended for the instance color palette.

## Architecture

- `src/main.rs` — entry; tokio runtime; ratatui init/restore.
- `src/app.rs` — `App` state, async event loop (`tokio::select!` over crossterm `EventStream`, mpsc `AppEvent` channel, tick).
- `src/ha/{client,protocol,registry}.rs` — one tokio task per instance, custom thin Home Assistant WebSocket protocol layer over `tokio-tungstenite`.
- `src/dashboard/{mod,layout,persist,editor}.rs` — dashboard model, grid → Rect math, YAML round-trip, editor reducer with undo stack.
- `src/screens/` + `src/ui/widgets/` — per-screen draw logic and reusable widgets.
- `src/ui/theme.rs` — instance color palette resolver.
- `src/actions.rs` — domain-aware default action for `Enter`.

## Status

This is an early build (v0.1). Functional milestones:

- [x] Connect + auth to one or more HA instances
- [x] Live entity browser with color-coded multi-instance view
- [x] Service calls via Enter (domain defaults)
- [x] YAML dashboards with 5 card types
- [x] Interactive editor (mouse + keyboard, undo, save)
- [ ] Free-form service-call dialog
- [ ] History backfill via HA `history/period` API
- [ ] OAuth (refresh-token) auth flow
- [ ] Windows CI build
- [ ] Comment-preserving YAML save

## License

MIT
