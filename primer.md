# ha-tui — primer

## What this is
Terminal UI for Home Assistant. Rust + Ratatui 0.30. Multi-instance, color-coded entities, YAML dashboards, interactive dashboard editor (mouse + keyboard). Long-lived access token auth.

## Current state — M1–M9 complete, branch `feature/card-improvements`
- `cargo build` passes (0 errors, 0 warnings).
- `cargo test` — 42 tests pass.
- `cargo clippy --all-targets` — clean.

## What's implemented
- **HA WebSocket client** (custom, on `tokio-tungstenite`): auth handshake, `subscribe_events`, `call_service`, reconnect with backoff. One tokio task per instance, mpsc to/from main.
- **Multi-instance**: connect N HA servers concurrently. Each gets distinct color (auto from 8-color palette, or `color:` override per instance).
- **Entity browser**: live state list, color-prefixed by instance, JSON attrs in right pane, `f` cycles instance filter.
- **Service calls**: Enter triggers domain-aware default (`light.toggle`, etc.).
- **YAML dashboards**: all card types below.
- **Dashboard editor** (`e`): keyboard + mouse, add/delete/move/resize/undo/save.
- **Help overlay**: `?` toggles modal.

## Card types (M1–M9 complete)
- `entity` — state + optional ticker animation
- `toggle` — on/off indicator
- `gauge` — HA-style horizontal arc with severity zones + needle
- `graph` — multi-series sparkline (Line/Bar/Pie, axes, backfilled history)
- `text` — markdown body
- `entity_list` — static entity list
- `filtered_entity_list` — dynamic glob+filter query
- `clock` — strftime format, optional IANA timezone
- `statistics` — Avg/Min/Max/Sum/Count over configurable window
- `media_player` — title/artist/app/volume gauge
- `image` — image entity or camera proxy with optional refresh interval
- `weather` — current conditions + optional forecast strip, 30-min auto-refresh

## Files
- `src/app.rs` — App state, event loop, all handlers (4000+ lines)
- `src/event.rs` — AppEvent enum (includes HaWeatherForecast, RefreshWeatherForecast)
- `src/ha/mod.rs` — HaCommand (including GetWeatherForecast), ForecastKind, ForecastDay
- `src/ha/client.rs` — WS loop, pending_history + pending_weather maps, parse_weather_forecast
- `src/ha/protocol.rs` — ClientMsg/ServerMsg, weather_get_forecasts_payload()
- `src/dashboard/mod.rs` — CardKind (including Weather), Card methods
- `src/dashboard/editor.rs` — EditorMode (including Wx* variants), CardTypeStub (including Weather)
- `src/screens/dashboard.rs` — render_card dispatch (all variants including Weather)
- `src/screens/editor.rs` — draw handlers (all variants including Wx*)
- `src/ui/widgets/card_weather.rs` — Weather renderer (Small/Normal/Large + forecast strip)

## Plan / milestone tracking
Full plan at `docs/superpowers/plans/2026-05-20-card-improvements.md`.
Spec at `docs/superpowers/specs/2026-05-20-card-improvements-design.md`.

## Next steps
1. **M10 Final**: Update README, add example YAML snippets for new card types, update primer, merge feature branch to main.
2. **Test against a real HA instance**: verify weather.get_forecasts works with HA 2024.x+.
3. Weather cards on older HA (pre-2024) will get empty forecasts gracefully (defensive parser).

## Open blockers
- None. Build is clean, all tests pass.
