# ha-tui — primer

## What this is
Terminal UI for Home Assistant. Rust + Ratatui 0.30. Multi-instance, color-coded entities, YAML dashboards, interactive dashboard editor (mouse + keyboard). Long-lived access token auth.

## Current state — M0–M5 complete + M6 partial
- `cargo build` passes (0 errors, ~10 dead-code warnings on future-use fields).
- `cargo test` — 15 tests pass.
- Binary runs `cargo run --` and renders empty UI when no config; loads config + connects multi-instance when present.

## What's implemented
- **HA WebSocket client** (custom, on `tokio-tungstenite`): auth handshake, `subscribe_events`, `call_service`, reconnect with backoff. One tokio task per instance, mpsc to/from main.
- **Multi-instance**: connect N HA servers concurrently. Each gets distinct color (auto from 8-color palette, or `color:` override per instance — also supports `#rrggbb`).
- **Entity browser**: live state list, color-prefixed by instance, JSON attrs in right pane, `f` cycles instance filter.
- **Service calls**: Enter triggers domain-aware default (`light.toggle`, `script.turn_on`, `cover.toggle`, `lock.unlock`, …). Errors surface in status bar.
- **YAML dashboards**: 5 card types — `entity`, `toggle`, `gauge`, `sparkline`, `text`. `1..9` switches dashboards.
- **Dashboard editor** (`e` from dashboard view):
  - Keyboard: `hjkl` cursor, `HJKL` resize, `Enter` select / place, `a` add (palette + entity prompt), `d` delete (confirm), `u` undo, `s` save, `Esc` exit (confirm if dirty).
  - Mouse: click cell to select card, drag card to move. Mouse capture enabled via crossterm.
- **Help overlay**: `?` toggles a centered modal.
- **Logging**: tracing → rotating daily file at `~/Library/Application Support/ha-tui/log/ha-tui.log` (XDG-equivalent on Linux).
- **CLI**: `--config <path>`, `--dashboards <path>`.

## Files
- [Cargo.toml](Cargo.toml) — deps pinned (ratatui 0.30, crossterm 0.28, tokio 1, tokio-tungstenite 0.24).
- [src/main.rs](src/main.rs) — clap, tracing init, tokio runtime, ratatui init/restore.
- [src/app.rs](src/app.rs) — `App` state, event loop, all key/mouse handlers, editor dispatcher.
- [src/event.rs](src/event.rs) — `AppEvent` enum.
- [src/actions.rs](src/actions.rs) — domain default action for Enter.
- [src/config/{mod,load}.rs](src/config/load.rs) — YAML config + env / token-file resolution.
- [src/ha/{mod,client,protocol,registry}.rs](src/ha/client.rs) — HA WS protocol layer.
- [src/dashboard/{mod,layout,persist,editor}.rs](src/dashboard/editor.rs) — dashboard model + editor reducer.
- [src/screens/{mod,entities,instances,dashboard,editor}.rs](src/screens/mod.rs) — screen state + draw entry points.
- [src/ui/{mod,theme}.rs](src/ui/mod.rs) + [src/ui/widgets/](src/ui/widgets/) — render layer.
- [src/util/{logging,history}.rs](src/util/history.rs) — log appender, sparkline ring buffer.
- [config/config.example.yaml](config/config.example.yaml), [config/dashboards.example.yaml](config/dashboards.example.yaml).
- [README.md](README.md) — full user docs.

## Plan
Full plan at `~/.claude/plans/i-would-like-to-async-panda.md`. Milestones M0–M5 done, M6 partial (README + help overlay shipped; CI + Windows test pending).

## Next steps
1. **Commit current state** — `git commit` currently blocked by locked 1Password SSH agent. Either unlock it (`op signin`) or temporarily disable signing for this initial commit.
2. **Test against a real HA instance** — set up `~/.config/ha-tui/config.yaml` with a valid long-lived token and run `cargo run --release`. Verify the acceptance checklist in the plan.
3. **Windows CI** — add GitHub Actions workflow building on Linux + macOS + Windows.
4. **Free-form service call dialog** — current Enter handles only domain defaults.

## Open blockers
- **1Password SSH agent locked** — commits fail with `1Password: failed to fill whole buffer`. No code committed yet. All M0–M5 work is staged + on disk only.
