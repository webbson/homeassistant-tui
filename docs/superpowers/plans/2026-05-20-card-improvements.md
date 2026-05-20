# Card improvements & new card types — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add per-card color/size overrides, FilteredEntityList `hide_when_empty`, replace Gauge with HA-style gauge, rename Sparkline→Graph with multi-entity + Line/Bar/Pie modes, and add Clock/Statistics/MediaPlayer/Image/Weather cards.

**Architecture:** All new fields hang off the existing `Card` wrapper or live as new `CardKind` variants. Each card type has its own widget file under `src/ui/widgets/`. New `HaCommand` variants extend the existing per-instance command channel for service calls, forecasts, and image fetches. HTTP fetches use a new `src/ha/rest.rs` module wrapping `reqwest`.

**Tech Stack:** Rust 2021, ratatui 0.30, crossterm 0.29, tokio 1, tokio-tungstenite 0.24, serde + serde_yaml, chrono. New: `tui-big-text`, `chrono-tz`, `ratatui-image`, `reqwest`, `image`, `tui-piechart` (or canvas fallback).

**Reference spec:** [docs/superpowers/specs/2026-05-20-card-improvements-design.md](../specs/2026-05-20-card-improvements-design.md)

---

## File structure

**Modified:**
- `Cargo.toml` — new deps per milestone
- `src/dashboard/mod.rs` — `Card.color`, `Card.size`, new `CardKind` variants, normalization helpers
- `src/dashboard/editor.rs` — add-flow + context menus for new fields/cards
- `src/dashboard/persist.rs` — pass through (already round-trips via serde)
- `src/dashboard/query.rs` — unchanged
- `src/screens/editor.rs` — menu UI for color/size/severity/series/etc.
- `src/screens/dashboard.rs` — dispatch new card types to renderers
- `src/event.rs` — new `AppEvent` variants for forecasts, image bytes
- `src/ha/client.rs` — handle new `HaCommand` variants
- `src/ha/protocol.rs` — `weather/get_forecasts` request shape
- `src/ha/mod.rs` — re-export new HA command kinds
- `src/ui/mod.rs` — unchanged at top, but `theme.rs` exposes a public `parse_color` helper
- `src/ui/theme.rs` — public `parse_color`, helper to resolve `card.color` → `Color` with instance fallback
- `src/ui/widgets/card_entity.rs`, `card_toggle.rs`, `card_text.rs`, `card_entity_list.rs` — apply `Card.color` + `Card.size`
- `src/ui/widgets/card_sparkline.rs` → renamed `card_graph.rs`, expanded
- `src/ui/widgets/card_gauge.rs` — replaced with HA-style implementation
- `src/util/history.rs` — unchanged

**Created:**
- `src/ha/rest.rs` — small async HTTP client (reqwest) for image_proxy / camera_proxy
- `src/ui/widgets/card_clock.rs`
- `src/ui/widgets/card_weather.rs`
- `src/ui/widgets/card_image.rs`
- `src/ui/widgets/card_media_player.rs`
- `src/ui/widgets/card_statistics.rs`

---

## Conventions used by every task

- **Branching:** All work lands on `feature/card-improvements`. Each milestone is a series of small commits on that branch. Do not rebase between tasks — use additive commits.
- **Test runner:** `cargo test --quiet`. For a single test: `cargo test <substring> --quiet -- --nocapture`.
- **Lint:** Run `cargo clippy --all-targets --quiet` at the end of each milestone. Fix every warning before declaring the milestone done.
- **Format:** Run `cargo fmt` before every commit.
- **Snapshots:** Use `cargo insta review` (via `cargo install cargo-insta` if missing) after introducing new `insta::assert_snapshot!` calls.
- **Commit style:** Conventional commits — `feat:`, `fix:`, `refactor:`, `test:`, `docs:`. Each step that says "Commit" produces exactly one commit.

---

## Milestone 1: Card-wrapper `color` + `size` overrides

### Task 1.1: Add deps and `CardSize` enum

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/dashboard/mod.rs`

- [ ] **Step 1: Add `tui-big-text` to `Cargo.toml`**

In `[dependencies]`, add (verify version compatible with ratatui 0.30 at install — current as of writing is 0.7):

```toml
tui-big-text = "0.7"
```

- [ ] **Step 2: Run `cargo build` to fetch the new dep**

Run: `cargo build --quiet`
Expected: builds cleanly, downloads `tui-big-text`.

- [ ] **Step 3: Add `CardSize` enum and extend `Card` struct in `src/dashboard/mod.rs`**

Replace the `Card` struct and add `CardSize` immediately above it:

```rust
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CardSize {
    Small,
    #[default]
    Normal,
    Large,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
    #[serde(flatten)]
    pub kind: CardKind,
    pub pos: Pos,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "is_default_size")]
    pub size: CardSize,
}

fn is_default_size(s: &CardSize) -> bool {
    *s == CardSize::Normal
}
```

- [ ] **Step 4: Build to confirm no regressions**

Run: `cargo build --quiet`
Expected: builds cleanly (existing code constructs `Card { kind, pos }` — fix call sites in the next step).

- [ ] **Step 5: Fix all `Card { kind, pos }` constructions to use the new fields**

Search: `rg 'Card \{ kind' src/`. For each hit, change to `Card { kind, pos, color: None, size: CardSize::Normal }`. Import `CardSize` where needed.

- [ ] **Step 6: `cargo build --quiet` and `cargo test --quiet`**

Expected: both pass. No new tests yet.

- [ ] **Step 7: Commit**

```bash
cargo fmt
git add Cargo.toml Cargo.lock src/dashboard/mod.rs $(rg -l 'CardSize::Normal' src/)
git commit -m "feat(dashboard): add per-card color and size overrides on Card wrapper"
```

### Task 1.2: Public `parse_color` + `resolve_card_color` helper

**Files:**
- Modify: `src/ui/theme.rs`
- Test: inline `#[cfg(test)] mod tests` in `src/ui/theme.rs`

- [ ] **Step 1: Write the failing test**

Append to `src/ui/theme.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_card_color_uses_override_when_set() {
        let theme = Theme::empty();
        // override wins over instance
        assert_eq!(
            resolve_card_color(Some("#ff0000"), "any", &theme),
            Color::Rgb(0xff, 0x00, 0x00),
        );
    }

    #[test]
    fn resolve_card_color_falls_back_to_instance_when_no_override() {
        let theme = Theme::empty(); // unknown instance → Color::White
        assert_eq!(resolve_card_color(None, "missing", &theme), Color::White);
    }

    #[test]
    fn resolve_card_color_falls_back_when_override_unparseable() {
        let theme = Theme::empty();
        assert_eq!(
            resolve_card_color(Some("not-a-color"), "missing", &theme),
            Color::White,
        );
    }
}
```

- [ ] **Step 2: Run and confirm failure**

Run: `cargo test theme:: --quiet`
Expected: FAIL — `resolve_card_color` not defined; `parse_color` private.

- [ ] **Step 3: Make `parse_color` public and add `resolve_card_color`**

In `src/ui/theme.rs`, change `fn parse_color` to `pub fn parse_color`, and append above `#[cfg(test)]`:

```rust
pub fn resolve_card_color(card_color: Option<&str>, instance: &str, theme: &Theme) -> Color {
    card_color
        .and_then(parse_color)
        .unwrap_or_else(|| theme.instance_color(instance))
}
```

- [ ] **Step 4: Re-run tests**

Run: `cargo test theme:: --quiet`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cargo fmt
git add src/ui/theme.rs
git commit -m "feat(theme): add resolve_card_color helper for per-card overrides"
```

### Task 1.3: Wire `Card.color` through every existing renderer

**Files:**
- Modify: `src/ui/widgets/card_entity.rs`, `card_toggle.rs`, `card_gauge.rs`, `card_sparkline.rs`, `card_entity_list.rs`, `card_text.rs`
- Modify: `src/screens/dashboard.rs` (passes args to renderers)

- [ ] **Step 1: Change each `render(...)` signature to accept `card_color: Option<&str>` and replace `theme.instance_color(instance)` with `resolve_card_color(card_color, instance, theme)`**

For each widget file, change calls like:
```rust
let color = theme.instance_color(instance);
```
to:
```rust
let color = crate::ui::theme::resolve_card_color(card_color, instance, theme);
```
And add `card_color: Option<&str>` as a new arg before `theme`.

For `card_text.rs` (no instance), pass `card_color` and apply `parse_color(card_color)` if `Some`, else theme default text colour (use `Color::White`).

- [ ] **Step 2: Update every call site in `src/screens/dashboard.rs`**

For each renderer dispatch, pass `card.color.as_deref()` as the new arg. Example:
```rust
widgets::card_entity::render(f, area, &title, instance, entity_state, card.color.as_deref(), theme, selected);
```

- [ ] **Step 3: `cargo build --quiet`**

Expected: builds cleanly. If errors, fix call sites until clean.

- [ ] **Step 4: `cargo test --quiet`**

Expected: all existing tests pass.

- [ ] **Step 5: Commit**

```bash
cargo fmt
git add src/ui/widgets src/screens/dashboard.rs
git commit -m "feat(ui): apply Card.color override to all existing card renderers"
```

### Task 1.4: Wire `Card.size` through every existing renderer

**Files:**
- Modify: each widget that supports a "primary value" rendering: `card_entity.rs`, `card_toggle.rs`, `card_gauge.rs`. (Graph / EntityList / FilteredList / Text ignore size or apply Small-only adjustments.)
- Test: inline tests in `src/dashboard/mod.rs` covering YAML round-trip of `size`

- [ ] **Step 1: Write failing serde round-trip test in `src/dashboard/mod.rs`**

Append to `src/dashboard/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_serde_round_trip_with_size_and_color() {
        let yaml = r#"
type: entity
instance: home
entity: light.kitchen
pos: { col: 0, row: 0, w: 4, h: 2 }
color: "#ff8800"
size: large
"#;
        let card: Card = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(card.color.as_deref(), Some("#ff8800"));
        assert_eq!(card.size, CardSize::Large);
        let back = serde_yaml::to_string(&card).unwrap();
        assert!(back.contains("size: large"));
        assert!(back.contains("color: \"#ff8800\"") || back.contains("color: '#ff8800'"));
    }

    #[test]
    fn card_serde_omits_defaults() {
        let yaml = r#"
type: entity
instance: home
entity: light.kitchen
pos: { col: 0, row: 0, w: 4, h: 2 }
"#;
        let card: Card = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(card.color, None);
        assert_eq!(card.size, CardSize::Normal);
        let back = serde_yaml::to_string(&card).unwrap();
        assert!(!back.contains("size:"));
        assert!(!back.contains("color:"));
    }
}
```

- [ ] **Step 2: Run and confirm pass (test only validates existing behaviour)**

Run: `cargo test card_serde --quiet`
Expected: PASS (Task 1.1 set the fields up correctly). If it fails, fix `skip_serializing_if` until both tests pass.

- [ ] **Step 3: Add a `big_text` helper for `Large` rendering**

Create `src/ui/widgets/big_text.rs`:

```rust
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::Frame;
use tui_big_text::{BigText, PixelSize};

pub fn render_big(f: &mut Frame, area: Rect, text: &str, color: Color) {
    let big = BigText::builder()
        .pixel_size(PixelSize::Quadrant)
        .style(Style::new().fg(color))
        .lines(vec![text.into()])
        .build();
    f.render_widget(big, area);
}

pub fn fits(area: Rect) -> bool {
    // tui-big-text Quadrant glyphs need ~4 cells wide × 4 cells tall per char.
    area.width >= 8 && area.height >= 4
}
```

Add `pub mod big_text;` to `src/ui/widgets/mod.rs`.

- [ ] **Step 4: Use `big_text` in `card_entity.rs` when `size == Large`**

In `card_entity::render`, accept `size: CardSize` as a new arg. After computing the inner area, branch:
```rust
match size {
    CardSize::Large if big_text::fits(inner) => {
        big_text::render_big(f, inner, &state_str, color);
    }
    _ => {
        // existing rendering
    }
}
```

Same pattern in `card_toggle.rs` (for the on/off label) and in the new `card_gauge.rs` (Task 3.x — defer).

- [ ] **Step 5: Update every call site to pass `card.size`**

In `src/screens/dashboard.rs`, change each dispatch to include `card.size`.

- [ ] **Step 6: `cargo build --quiet` and `cargo test --quiet`**

Expected: pass.

- [ ] **Step 7: Commit**

```bash
cargo fmt
git add src/ui/widgets src/screens/dashboard.rs src/dashboard/mod.rs
git commit -m "feat(ui): apply Card.size to entity/toggle renderers (large = big-text)"
```

### Task 1.5: Editor menu entries for color + size

**Files:**
- Modify: `src/dashboard/editor.rs`
- Modify: `src/screens/editor.rs`

- [ ] **Step 1: Locate the card context-menu (`m`) generator**

Run: `rg 'card_menu_items|fn card_menu' src/dashboard/editor.rs`. Read the surrounding fn.

- [ ] **Step 2: Add two new menu items**

In `card_menu_items` (or equivalent), append entries `"Color override"` and `"Size"` to the returned list. Each maps to a new `EditorMode` variant — define them in the `EditorMode` enum:

```rust
EnterColorOverride { card_idx: usize, buf: String },
PickCardSize { card_idx: usize, current: CardSize },
```

- [ ] **Step 3: Wire menu selection to enter the new modes**

In the dispatcher that handles menu choice, route the two new labels to setting `EditorMode::EnterColorOverride { card_idx: selected, buf: card.color.clone().unwrap_or_default() }` and `EditorMode::PickCardSize { card_idx: selected, current: card.size }`.

- [ ] **Step 4: Render the two modes in `src/screens/editor.rs`**

Add overlays:
- `EnterColorOverride` — single-line text prompt, current value shown; Enter commits (parse fails → status-bar warning); Esc cancels; empty input clears the override.
- `PickCardSize` — three-row chooser (Small / Normal / Large), j/k to move, Enter to commit.

Use existing prompt-overlay helpers in `src/screens/editor.rs` for consistency.

- [ ] **Step 5: On commit, push a snapshot onto the undo stack and mutate the dashboard**

```rust
EditorMode::EnterColorOverride { card_idx, buf } => {
    self.push_undo();
    let c = if buf.trim().is_empty() { None } else { Some(buf.clone()) };
    self.dashboard.cards[*card_idx].color = c;
    self.mode = EditorMode::Idle;
    self.dirty = true;
}
EditorMode::PickCardSize { card_idx, current } => {
    self.push_undo();
    self.dashboard.cards[*card_idx].size = *current;
    self.mode = EditorMode::Idle;
    self.dirty = true;
}
```

- [ ] **Step 6: `cargo build --quiet` and `cargo test --quiet`**

Expected: pass.

- [ ] **Step 7: Manual test**

Run: `cargo run -- --config config/config.example.yaml --dashboards config/dashboards.example.yaml`. Press `e`, select a card, press `m`, choose "Color override", type `#00ff00`, Enter. Confirm the card border turns green. Press `m`, "Size", pick Large, confirm the entity value renders in big text. Press `s` to save and check the YAML file contains `color:` and `size:` lines.

- [ ] **Step 8: Commit**

```bash
cargo fmt
git add src/dashboard/editor.rs src/screens/editor.rs
git commit -m "feat(editor): menu entries for per-card color override and size"
```

### Task 1.6: Milestone gate

- [ ] **Step 1: `cargo clippy --all-targets --quiet`**
Expected: clean.
- [ ] **Step 2: `cargo test --quiet`**
Expected: clean.

---

## Milestone 2: FilteredEntityList `hide_when_empty`

### Task 2.1: Add the field + serde defaults

**Files:**
- Modify: `src/dashboard/mod.rs`
- Test: inline

- [ ] **Step 1: Write the failing test**

Append to the existing `#[cfg(test)] mod tests` in `src/dashboard/mod.rs`:

```rust
#[test]
fn filtered_list_hide_when_empty_round_trip() {
    let yaml = r#"
type: filtered_entity_list
instance: home
query: "light.*"
hide_when_empty: true
pos: { col: 0, row: 0, w: 6, h: 4 }
"#;
    let card: Card = serde_yaml::from_str(yaml).unwrap();
    if let CardKind::FilteredEntityList { hide_when_empty, .. } = &card.kind {
        assert!(*hide_when_empty);
    } else {
        panic!("wrong variant");
    }
}
```

- [ ] **Step 2: Run and confirm failure**

Run: `cargo test filtered_list_hide --quiet`
Expected: FAIL — unknown field `hide_when_empty`.

- [ ] **Step 3: Add the field**

In `CardKind::FilteredEntityList { .. }`, add:
```rust
#[serde(default)]
hide_when_empty: bool,
```

- [ ] **Step 4: Run and confirm pass**

Run: `cargo test filtered_list_hide --quiet`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cargo fmt
git add src/dashboard/mod.rs
git commit -m "feat(dashboard): add hide_when_empty field on FilteredEntityList"
```

### Task 2.2: Honour the flag at render time

**Files:**
- Modify: `src/ui/widgets/card_entity_list.rs` (or the filtered-list renderer it routes to)
- Modify: `src/screens/dashboard.rs`

- [ ] **Step 1: Locate the FilteredEntityList render path**

Run: `rg 'FilteredEntityList' src/`. Identify the renderer dispatch in `src/screens/dashboard.rs`.

- [ ] **Step 2: Skip rendering when `hide_when_empty && matches == 0` AND we're not in editor mode**

Wrap the existing renderer call:

```rust
CardKind::FilteredEntityList { instance, query, hide_state, hide_when_empty, title } => {
    let matches = filter_matches(app, instance, query);
    if matches.is_empty() && *hide_when_empty && !in_editor {
        return;
    }
    widgets::card_entity_list::render_filtered(/* args */);
}
```

(`in_editor` is true when the screen is `Screen::Editor`. Pass through as needed.)

- [ ] **Step 3: In editor mode, draw a faint placeholder outline**

When `in_editor && matches.is_empty() && *hide_when_empty`, render a dashed/dim border with the title and a `"(hidden — no matches)"` body line so the card is still pickable.

- [ ] **Step 4: `cargo build --quiet` and `cargo test --quiet`**

Expected: pass.

- [ ] **Step 5: Commit**

```bash
cargo fmt
git add src/ui/widgets src/screens/dashboard.rs
git commit -m "feat(dashboard): honour FilteredEntityList.hide_when_empty at render time"
```

### Task 2.3: Editor menu toggle

**Files:**
- Modify: `src/dashboard/editor.rs`

- [ ] **Step 1: Add "Hide when empty" menu entry for filtered lists**

In the card-context-menu builder, when the selected card is `FilteredEntityList`, append `"Hide when empty: <on|off>"` as a toggle. Selecting it pushes undo and flips the field.

- [ ] **Step 2: `cargo build --quiet`**

Expected: pass.

- [ ] **Step 3: Commit**

```bash
cargo fmt
git add src/dashboard/editor.rs
git commit -m "feat(editor): toggle for FilteredEntityList.hide_when_empty"
```

### Task 2.4: Milestone gate

- [ ] **Step 1: `cargo clippy --all-targets --quiet`** — clean
- [ ] **Step 2: `cargo test --quiet`** — clean

---

## Milestone 3: Sparkline → Graph rewrite (multi-entity + Line/Bar/Pie)

### Task 3.1: Rename file + add new variant skeleton

**Files:**
- Move: `src/ui/widgets/card_sparkline.rs` → `src/ui/widgets/card_graph.rs`
- Modify: `src/ui/widgets/mod.rs`
- Modify: `src/dashboard/mod.rs`

- [ ] **Step 1: Move the file**

```bash
git mv src/ui/widgets/card_sparkline.rs src/ui/widgets/card_graph.rs
```

Update `src/ui/widgets/mod.rs`: replace `pub mod card_sparkline;` with `pub mod card_graph;`.

- [ ] **Step 2: Replace `CardKind::Sparkline` with `CardKind::Graph` + new types**

In `src/dashboard/mod.rs`, delete:
```rust
Sparkline { instance, entity, window, title }
```
Add:

```rust
#[serde(rename = "graph", alias = "sparkline")]
Graph {
    instance: Alias,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    entity: Option<EntityId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    entities: Vec<GraphSeries>,
    #[serde(default)]
    graph_type: GraphType,
    #[serde(default = "default_window")]
    window: String,
    #[serde(default)]
    orientation: BarOrientation,
    #[serde(default)]
    title: Option<String>,
},
```

Above the enum:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphSeries {
    pub entity: EntityId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GraphType {
    #[default]
    Line,
    Bar,
    Pie,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BarOrientation {
    #[default]
    Vertical,
    Horizontal,
}
```

- [ ] **Step 3: Update `Card::title` / `Card::entity_ref` matchers**

Replace `Sparkline { … }` arms with `Graph { entity, entities, … }`. `entity_ref` for `Graph` returns `Some((instance, &series[0].entity))` where `series` is built by `effective_entities` (defined next).

- [ ] **Step 4: Add `effective_entities` helper on `Card`**

```rust
impl Card {
    pub fn graph_entities(&self) -> Vec<&EntityId> {
        match &self.kind {
            CardKind::Graph { entity, entities, .. } => {
                if !entities.is_empty() {
                    entities.iter().map(|s| &s.entity).collect()
                } else if let Some(e) = entity {
                    vec![e]
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        }
    }
}
```

- [ ] **Step 5: `cargo build --quiet`**

Fix every call site that referenced `Sparkline { … }` — there will be matches in `src/app.rs`, `src/screens/dashboard.rs`, `src/dashboard/editor.rs`. Update each to `Graph { … }` and pass through the new fields with sensible defaults.

- [ ] **Step 6: `cargo test --quiet`**

Expected: pre-existing tests pass.

- [ ] **Step 7: Commit**

```bash
cargo fmt
git add -A
git commit -m "refactor(dashboard): rename Sparkline→Graph with multi-series + type fields"
```

### Task 3.2: Series-normalization test + helper

**Files:**
- Modify: `src/dashboard/mod.rs`

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn graph_legacy_sparkline_normalizes_to_single_series() {
    let yaml = r#"
type: sparkline
instance: home
entity: sensor.temp
window: 6h
pos: { col: 0, row: 0, w: 6, h: 4 }
"#;
    let mut card: Card = serde_yaml::from_str(yaml).unwrap();
    card.normalize();
    if let CardKind::Graph { entity, entities, window, .. } = &card.kind {
        assert!(entity.is_none(), "legacy entity should be cleared after normalize");
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].entity.as_str(), "sensor.temp");
        assert_eq!(window, "6h");
    } else {
        panic!("wrong variant");
    }
}

#[test]
fn graph_multi_series_round_trip() {
    let yaml = r#"
type: graph
instance: home
graph_type: bar
orientation: horizontal
entities:
  - { entity: sensor.cpu_0 }
  - { entity: sensor.cpu_1, label: "CPU 1", color: "#ff00ff" }
pos: { col: 0, row: 0, w: 6, h: 4 }
"#;
    let card: Card = serde_yaml::from_str(yaml).unwrap();
    if let CardKind::Graph { entities, graph_type, orientation, .. } = &card.kind {
        assert_eq!(entities.len(), 2);
        assert_eq!(*graph_type, GraphType::Bar);
        assert_eq!(*orientation, BarOrientation::Horizontal);
        assert_eq!(entities[1].label.as_deref(), Some("CPU 1"));
    } else {
        panic!("wrong variant");
    }
}
```

- [ ] **Step 2: Confirm failure**

Run: `cargo test graph_ --quiet`
Expected: FAIL — `Card::normalize` undefined.

- [ ] **Step 3: Add `normalize` to `Card`**

```rust
impl Card {
    pub fn normalize(&mut self) {
        if let CardKind::Graph { entity, entities, .. } = &mut self.kind {
            if entities.is_empty() {
                if let Some(e) = entity.take() {
                    entities.push(GraphSeries { entity: e, label: None, color: None });
                }
            }
        }
    }
}
```

- [ ] **Step 4: Call `normalize` in `dashboard::load`**

Modify `src/dashboard/persist.rs` (or wherever `DashboardFile` is loaded): after parsing, iterate every card and call `card.normalize()`. Add an error if a `Graph` ends with empty `entities`:

```rust
for (d_idx, d) in file.dashboards.iter_mut().enumerate() {
    for (c_idx, c) in d.cards.iter_mut().enumerate() {
        c.normalize();
        if let CardKind::Graph { entities, .. } = &c.kind {
            if entities.is_empty() {
                return Err(format!("dashboard {} card {} graph has no entities", d_idx, c_idx).into());
            }
        }
    }
}
```

- [ ] **Step 5: Re-run tests**

Run: `cargo test graph_ --quiet`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
cargo fmt
git add src/dashboard/mod.rs src/dashboard/persist.rs
git commit -m "feat(dashboard): normalize legacy sparkline entity into Graph.entities"
```

### Task 3.3: Multi-series Line rendering

**Files:**
- Modify: `src/ui/widgets/card_graph.rs`
- Modify: `src/util/history.rs` (only if a multi-key history API is missing)
- Modify: `src/screens/dashboard.rs`

- [ ] **Step 1: Inspect the existing history storage**

Run: `rg 'RingBuf|FetchHistory' src/`. Confirm `RingBuf` is keyed per `(instance, entity)`. If not, adjust storage to map `(Alias, EntityId) → RingBuf`.

- [ ] **Step 2: Replace `card_graph::render` with a multi-series signature**

```rust
pub struct GraphRender<'a> {
    pub area: Rect,
    pub title: &'a str,
    pub instance: &'a str,
    pub series: &'a [GraphSeries],
    pub histories: &'a [(EntityId, Option<&'a RingBuf>)],
    pub window: &'a str,
    pub card_color: Option<&'a str>,
    pub size: CardSize,
    pub theme: &'a Theme,
    pub selected: bool,
}

pub fn render_line(f: &mut Frame, args: GraphRender<'_>) {
    // For each series, look up its history in `args.histories`, build a Dataset
    // styled with resolve_card_color(series.color ?? card_color, instance, theme).
    // Reuse the existing downsample + axis logic.
    // Show a legend row at the bottom when args.series.len() >= 2.
}
```

(Keep the existing downsample helper. Adapt the auto-scale to span all series.)

- [ ] **Step 3: Dispatch in `screens/dashboard.rs`**

When `graph_type == Line`, gather histories and call `render_line`. When `Bar` or `Pie`, defer to stubs that just render `"(unimplemented)"` for now.

- [ ] **Step 4: `cargo build --quiet` and `cargo test --quiet`**

Expected: pass.

- [ ] **Step 5: Manual smoke test**

Run with a dashboard containing a multi-series Graph and confirm two coloured lines render.

- [ ] **Step 6: Commit**

```bash
cargo fmt
git add src/ui/widgets/card_graph.rs src/screens/dashboard.rs
git commit -m "feat(graph): multi-series Line rendering with legend"
```

### Task 3.4: Bar rendering

**Files:**
- Modify: `src/ui/widgets/card_graph.rs`

- [ ] **Step 1: Implement `render_bar`**

```rust
pub fn render_bar(f: &mut Frame, args: GraphRender<'_>, orientation: BarOrientation, current: &[(EntityId, Option<f64>)]) {
    use ratatui::widgets::{BarChart, Bar, BarGroup};
    let bars: Vec<Bar> = current.iter().enumerate().map(|(i, (id, v))| {
        let label = args.series[i].label.clone().unwrap_or_else(|| short_name(id));
        let color = crate::ui::theme::resolve_card_color(
            args.series[i].color.as_deref().or(args.card_color),
            args.instance,
            args.theme,
        );
        Bar::default()
            .label(label.into())
            .value(v.unwrap_or(0.0) as u64)
            .style(Style::new().fg(color))
    }).collect();
    let chart = BarChart::default()
        .data(BarGroup::default().bars(&bars))
        .direction(match orientation {
            BarOrientation::Vertical => ratatui::layout::Direction::Vertical,
            BarOrientation::Horizontal => ratatui::layout::Direction::Horizontal,
        })
        .block(Block::bordered().title(args.title));
    f.render_widget(chart, args.area);
}

fn short_name(id: &EntityId) -> String {
    id.as_str().rsplit_once('.').map(|(_, s)| s.to_string()).unwrap_or_else(|| id.to_string())
}
```

(Check ratatui 0.30's `BarChart` API at implementation time — `direction` may be named differently. If `BarChart::direction` doesn't exist on 0.30, use the orientation-specific constructor or fall back to a hand-rolled implementation.)

- [ ] **Step 2: Wire dispatch in `screens/dashboard.rs` for `graph_type == Bar`**

Build `current` by reading each series entity's latest state, parsing as `f64`.

- [ ] **Step 3: `cargo build --quiet` and `cargo test --quiet`**

Expected: pass.

- [ ] **Step 4: Commit**

```bash
cargo fmt
git add src/ui/widgets/card_graph.rs src/screens/dashboard.rs
git commit -m "feat(graph): Bar mode (current value per series, vertical/horizontal)"
```

### Task 3.5: Pie rendering

**Files:**
- Modify: `Cargo.toml` (add `tui-piechart` or document fallback)
- Modify: `src/ui/widgets/card_graph.rs`

- [ ] **Step 1: Try adding `tui-piechart`**

Run: `cargo search tui-piechart` and inspect the latest version. Add it to `Cargo.toml` if a ratatui-0.30-compatible version exists. If not, skip this dep and implement the fallback below.

- [ ] **Step 2: Implement `render_pie` via `tui-piechart` (preferred) or `Canvas` (fallback)**

Preferred (verify the crate's actual API at install):
```rust
pub fn render_pie(f: &mut Frame, args: GraphRender<'_>, current: &[(EntityId, Option<f64>)]) {
    let slices = build_slices(&args, current);
    // call into tui-piechart's widget here
}
```

Canvas fallback (always works):
```rust
use ratatui::widgets::canvas::{Canvas, Context};

pub fn render_pie(f: &mut Frame, args: GraphRender<'_>, current: &[(EntityId, Option<f64>)]) {
    let total: f64 = current.iter().filter_map(|(_, v)| *v).sum();
    if total <= 0.0 {
        f.render_widget(
            Paragraph::new("(no positive values)").block(Block::bordered().title(args.title)),
            args.area,
        );
        return;
    }
    let canvas = Canvas::default()
        .block(Block::bordered().title(args.title))
        .x_bounds([-1.0, 1.0])
        .y_bounds([-1.0, 1.0])
        .paint(|ctx: &mut Context| {
            let mut start = 0.0_f64;
            for (i, (_, v)) in current.iter().enumerate() {
                let v = v.unwrap_or(0.0).max(0.0);
                if v == 0.0 { continue; }
                let end = start + std::f64::consts::TAU * (v / total);
                let color = crate::ui::theme::resolve_card_color(
                    args.series[i].color.as_deref().or(args.card_color),
                    args.instance,
                    args.theme,
                );
                let steps = 64;
                let mut prev: Option<(f64, f64)> = None;
                for k in 0..=steps {
                    let t = start + (end - start) * (k as f64 / steps as f64);
                    let p = (t.cos(), t.sin());
                    if let Some(pp) = prev {
                        ctx.draw(&ratatui::widgets::canvas::Line { x1: 0.0, y1: 0.0, x2: p.0, y2: p.1, color });
                        ctx.draw(&ratatui::widgets::canvas::Line { x1: pp.0, y1: pp.1, x2: p.0, y2: p.1, color });
                    }
                    prev = Some(p);
                }
                start = end;
            }
        });
    f.render_widget(canvas, args.area);
}
```

- [ ] **Step 3: Wire dispatch for `graph_type == Pie`**

In `screens/dashboard.rs`, gather `current` values and call `render_pie`.

- [ ] **Step 4: `cargo build --quiet` and `cargo test --quiet`**

Expected: pass.

- [ ] **Step 5: Commit**

```bash
cargo fmt
git add Cargo.toml Cargo.lock src/ui/widgets/card_graph.rs src/screens/dashboard.rs
git commit -m "feat(graph): Pie mode via tui-piechart (with Canvas fallback)"
```

### Task 3.6: Editor add-flow for Graph

**Files:**
- Modify: `src/dashboard/editor.rs`
- Modify: `src/screens/editor.rs`

- [ ] **Step 1: Replace the existing single-entity sparkline add-flow**

Add new `EditorMode` variants in order:
```rust
GraphPickType { },
GraphPickInstance { graph_type: GraphType },
GraphAddEntities { instance: Alias, graph_type: GraphType, accumulated: Vec<GraphSeries> },
GraphFinishConfig { instance: Alias, graph_type: GraphType, series: Vec<GraphSeries> },
```

`GraphFinishConfig` collects `window` for `Line`, `orientation` for `Bar`, then constructs and inserts the card.

- [ ] **Step 2: Render each mode in `src/screens/editor.rs`**

Use a 3-row select for `GraphPickType` (Line/Bar/Pie). For `GraphAddEntities`, after each entity is picked, prompt "add another? y/n". For the finish step, prompt the type-specific field.

- [ ] **Step 3: Context-menu items for an existing Graph card**

In the card-context-menu builder, when `Graph` is selected, append:
- `"Add series"` — enters `GraphAddEntities` with the current list pre-populated.
- `"Remove series"` — opens a list-pick over current `entities`.
- `"Set series colour"` — pick a series, prompt colour.
- `"Set series label"` — pick a series, prompt label.
- `"Change type"` — cycles Line/Bar/Pie.
- `"Window"` — only when `Line`.
- `"Orientation"` — only when `Bar`.

- [ ] **Step 4: `cargo build --quiet` and `cargo test --quiet`**

Expected: pass.

- [ ] **Step 5: Commit**

```bash
cargo fmt
git add src/dashboard/editor.rs src/screens/editor.rs
git commit -m "feat(editor): graph add-flow with multi-entity + type/orientation prompts"
```

### Task 3.7: Milestone gate

- [ ] **Step 1: `cargo clippy --all-targets --quiet`** — clean
- [ ] **Step 2: `cargo test --quiet`** — clean
- [ ] **Step 3: Manual sanity check** — load `config/dashboards.example.yaml` with an old `type: sparkline` card; confirm it loads, renders as a single-series line.

---

## Milestone 4: HA-style Gauge replacement

### Task 4.1: Update `CardKind::Gauge` shape

**Files:**
- Modify: `src/dashboard/mod.rs`

- [ ] **Step 1: Add `Severity` struct and update `Gauge` variant**

In `src/dashboard/mod.rs`:

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Severity {
    pub green: f64,
    pub yellow: f64,
    pub red: f64,
}

// inside CardKind:
Gauge {
    instance: Alias,
    entity: EntityId,
    min: f64,
    max: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    unit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    severity: Option<Severity>,
    #[serde(default = "default_true")]
    needle: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    title: Option<String>,
},
```

Add the `default_true` helper above the enum:
```rust
fn default_true() -> bool { true }
```

- [ ] **Step 2: Write a serde round-trip test**

```rust
#[test]
fn gauge_round_trip_with_severity() {
    let yaml = r#"
type: gauge
instance: home
entity: sensor.cpu
min: 0
max: 100
unit: "%"
severity: { green: 0, yellow: 60, red: 85 }
needle: true
pos: { col: 0, row: 0, w: 4, h: 4 }
"#;
    let card: Card = serde_yaml::from_str(yaml).unwrap();
    if let CardKind::Gauge { severity, needle, .. } = &card.kind {
        assert!(severity.is_some());
        assert!(*needle);
    } else { panic!() }
}

#[test]
fn gauge_legacy_round_trip() {
    let yaml = r#"
type: gauge
instance: home
entity: sensor.cpu
min: 0
max: 100
unit: "%"
pos: { col: 0, row: 0, w: 4, h: 4 }
"#;
    let card: Card = serde_yaml::from_str(yaml).unwrap();
    if let CardKind::Gauge { severity, needle, .. } = &card.kind {
        assert!(severity.is_none());
        assert!(*needle, "needle defaults to true");
    } else { panic!() }
}
```

- [ ] **Step 3: Run and confirm pass**

Run: `cargo test gauge_ --quiet` — PASS.

- [ ] **Step 4: Commit**

```bash
cargo fmt
git add src/dashboard/mod.rs
git commit -m "feat(dashboard): expand Gauge with severity + needle (back-compat)"
```

### Task 4.2: Replace `card_gauge::render`

**Files:**
- Modify: `src/ui/widgets/card_gauge.rs`

- [ ] **Step 1: Replace the file body**

```rust
use ratatui::layout::Rect;
use ratatui::style::{Color, Style, Stylize};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::dashboard::{CardSize, Severity};
use crate::ui::theme::{resolve_card_color, Theme};

#[allow(clippy::too_many_arguments)]
pub fn render(
    f: &mut Frame,
    area: Rect,
    title: &str,
    instance: &str,
    value: Option<f64>,
    min: f64,
    max: f64,
    unit: Option<&str>,
    severity: Option<&Severity>,
    needle: bool,
    card_color: Option<&str>,
    size: CardSize,
    theme: &Theme,
    selected: bool,
) {
    let base = resolve_card_color(card_color, instance, theme);
    let mut block = Block::bordered()
        .title(format!(" {title} "))
        .border_style(Style::new().fg(base));
    if selected {
        block = block.border_style(Style::new().fg(base).bold());
    }
    let inner = block.inner(area);
    f.render_widget(block, area);

    let v = value.unwrap_or(min);
    let clamped = v.clamp(min, max);
    let frac = if max > min { (clamped - min) / (max - min) } else { 0.0 };

    let arc_w = inner.width.saturating_sub(2) as usize;
    let arc_y = inner.y + inner.height / 2;
    let mut arc = String::with_capacity(arc_w);
    for i in 0..arc_w {
        let pos = i as f64 / arc_w.max(1) as f64;
        let cell_value = min + pos * (max - min);
        let _color = segment_color(cell_value, severity, base);
        arc.push('█');
        // colour applied per-cell using styled spans — see below
    }

    // Render as styled spans, one per cell, so each cell can have its own colour.
    use ratatui::text::Span;
    let mut spans: Vec<Span> = Vec::with_capacity(arc_w);
    let needle_idx = (frac * arc_w as f64) as usize;
    for i in 0..arc_w {
        let pos = i as f64 / arc_w.max(1) as f64;
        let cell_value = min + pos * (max - min);
        let mut color = segment_color(cell_value, severity, base);
        let mut style = Style::new().fg(color);
        if needle && i == needle_idx {
            style = Style::new().fg(Color::Black).bg(color);
        }
        spans.push(Span::styled("█", style));
        let _ = &mut color; // suppress "unused mut" if branch elided
    }
    let arc_area = Rect { x: inner.x + 1, y: arc_y, width: arc_w as u16, height: 1 };
    f.render_widget(Paragraph::new(ratatui::text::Line::from(spans)), arc_area);

    let value_text = match (value, unit) {
        (Some(v), Some(u)) => format!("{v:.1} {u}"),
        (Some(v), None) => format!("{v:.1}"),
        (None, _) => "—".into(),
    };
    let label_area = Rect { x: inner.x, y: arc_y + 1, width: inner.width, height: 1 };
    if size == CardSize::Large && crate::ui::widgets::big_text::fits(label_area) {
        crate::ui::widgets::big_text::render_big(f, label_area, &value_text, base);
    } else {
        f.render_widget(
            Paragraph::new(value_text).style(Style::new().fg(base)).alignment(ratatui::layout::Alignment::Center),
            label_area,
        );
    }
}

fn segment_color(value: f64, severity: Option<&Severity>, base: Color) -> Color {
    let Some(s) = severity else { return base };
    if value >= s.red { Color::Red }
    else if value >= s.yellow { Color::Yellow }
    else { Color::Green }
}
```

- [ ] **Step 2: Update dispatch in `screens/dashboard.rs`** — pass new args.

- [ ] **Step 3: `cargo build --quiet` and `cargo test --quiet`**

Expected: pass.

- [ ] **Step 4: Manual smoke test** — load a config with one gauge with severity zones and confirm colours change as state crosses thresholds.

- [ ] **Step 5: Commit**

```bash
cargo fmt
git add src/ui/widgets/card_gauge.rs src/screens/dashboard.rs
git commit -m "feat(gauge): HA-style horizontal arc with severity zones + needle"
```

### Task 4.3: Editor menu entries

**Files:**
- Modify: `src/dashboard/editor.rs`
- Modify: `src/screens/editor.rs`

- [ ] **Step 1: Add menu items** — `"Severity thresholds"` and `"Needle: <on|off>"` to the gauge context menu. New `EditorMode::EnterSeverity { card_idx, green, yellow, red, field: SeverityField }` with three sequential numeric prompts.

- [ ] **Step 2: `cargo build --quiet`** — clean.

- [ ] **Step 3: Commit**

```bash
cargo fmt
git add src/dashboard/editor.rs src/screens/editor.rs
git commit -m "feat(editor): severity threshold + needle toggle for Gauge"
```

### Task 4.4: Milestone gate

- [ ] **Step 1: `cargo clippy --all-targets --quiet`** — clean
- [ ] **Step 2: `cargo test --quiet`** — clean

---

## Milestone 5: Clock card

### Task 5.1: Add dep + variant

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/dashboard/mod.rs`

- [ ] **Step 1: Add `chrono-tz`**

```toml
chrono-tz = "0.10"
```

Run: `cargo build --quiet`. Expected: clean.

- [ ] **Step 2: Add `CardKind::Clock`**

```rust
Clock {
    #[serde(default = "default_clock_format")]
    format: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    timezone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    title: Option<String>,
},
```

```rust
fn default_clock_format() -> String { "%H:%M:%S".into() }
```

Update `Card::title` — add `CardKind::Clock { title, .. } => title.as_deref().unwrap_or("Clock"),`.

- [ ] **Step 3: Write the round-trip test**

```rust
#[test]
fn clock_round_trip() {
    let yaml = r#"
type: clock
format: "%H:%M"
timezone: Europe/Stockholm
pos: { col: 0, row: 0, w: 4, h: 2 }
"#;
    let card: Card = serde_yaml::from_str(yaml).unwrap();
    if let CardKind::Clock { format, timezone, .. } = &card.kind {
        assert_eq!(format, "%H:%M");
        assert_eq!(timezone.as_deref(), Some("Europe/Stockholm"));
    } else { panic!() }
}
```

- [ ] **Step 4: `cargo test clock_ --quiet`** — PASS.

- [ ] **Step 5: Commit**

```bash
cargo fmt
git add Cargo.toml Cargo.lock src/dashboard/mod.rs
git commit -m "feat(dashboard): add Clock card variant"
```

### Task 5.2: Renderer

**Files:**
- Create: `src/ui/widgets/card_clock.rs`
- Modify: `src/ui/widgets/mod.rs`

- [ ] **Step 1: Create the file**

```rust
use chrono::Local;
use chrono_tz::Tz;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::dashboard::CardSize;
use crate::ui::theme::{parse_color, Theme};
use crate::ui::widgets::big_text;

#[allow(clippy::too_many_arguments)]
pub fn render(
    f: &mut Frame,
    area: Rect,
    title: &str,
    format: &str,
    timezone: Option<&str>,
    card_color: Option<&str>,
    size: CardSize,
    _theme: &Theme,
    selected: bool,
) {
    let color = card_color.and_then(parse_color).unwrap_or(Color::White);
    let mut block = Block::bordered()
        .title(format!(" {title} "))
        .border_style(Style::new().fg(color));
    if selected {
        block = block.border_style(Style::new().fg(color).bold());
    }
    let inner = block.inner(area);
    f.render_widget(block, area);

    let now_str = match timezone.and_then(|tz| tz.parse::<Tz>().ok()) {
        Some(tz) => chrono::Utc::now().with_timezone(&tz).format(format).to_string(),
        None => Local::now().format(format).to_string(),
    };

    if matches!(size, CardSize::Large) && big_text::fits(inner) {
        big_text::render_big(f, inner, &now_str, color);
    } else {
        f.render_widget(
            Paragraph::new(now_str).style(Style::new().fg(color)).alignment(Alignment::Center),
            inner,
        );
    }
}
```

Register in `mod.rs`: `pub mod card_clock;`.

- [ ] **Step 2: Dispatch in `screens/dashboard.rs`**

Add a match arm for `CardKind::Clock { format, timezone, title }`. The 250ms tick already redraws.

- [ ] **Step 3: `cargo build --quiet` and `cargo test --quiet`** — clean.

- [ ] **Step 4: Manual smoke test** — add a Clock card to dashboards.example.yaml, run, confirm time updates.

- [ ] **Step 5: Commit**

```bash
cargo fmt
git add src/ui/widgets/card_clock.rs src/ui/widgets/mod.rs src/screens/dashboard.rs
git commit -m "feat(ui): Clock card renderer with optional timezone + big-text large mode"
```

### Task 5.3: Editor add-flow + menu

**Files:**
- Modify: `src/dashboard/editor.rs`
- Modify: `src/screens/editor.rs`

- [ ] **Step 1: Add Clock to the add-card type list**

Three prompts: title (optional), format (default `%H:%M:%S`), timezone (optional).

- [ ] **Step 2: Card context menu** — "Format" and "Timezone" entries with text prompts.

- [ ] **Step 3: `cargo build --quiet` and `cargo test --quiet`** — clean.

- [ ] **Step 4: Commit**

```bash
cargo fmt
git add src/dashboard/editor.rs src/screens/editor.rs
git commit -m "feat(editor): Clock add-flow + format/timezone menu"
```

### Task 5.4: Milestone gate

- [ ] **Step 1: `cargo clippy --all-targets --quiet`** — clean
- [ ] **Step 2: `cargo test --quiet`** — clean

---

## Milestone 6: Statistics card

### Task 6.1: Variant + compute helper

**Files:**
- Modify: `src/dashboard/mod.rs`
- Modify: `src/util/history.rs` (add a `compute_stat` if missing)

- [ ] **Step 1: Add the variant**

```rust
Statistics {
    instance: Alias,
    entity: EntityId,
    #[serde(default = "default_window")]
    window: String,
    metric: StatsMetric,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    unit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    title: Option<String>,
},
```

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StatsMetric { Avg, Min, Max, Sum, Count }
```

Update `Card::title` + `Card::entity_ref`.

- [ ] **Step 2: Test for the compute helper**

In `src/util/history.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::dashboard::StatsMetric;

    fn buf_from(values: &[f64]) -> RingBuf {
        let mut b = RingBuf::new(64);
        for (i, v) in values.iter().enumerate() {
            b.push(chrono::Utc::now() + chrono::Duration::seconds(i as i64), *v);
        }
        b
    }

    #[test]
    fn metric_min_max_avg_sum_count() {
        let b = buf_from(&[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(b.compute(StatsMetric::Min), Some(1.0));
        assert_eq!(b.compute(StatsMetric::Max), Some(4.0));
        assert_eq!(b.compute(StatsMetric::Avg), Some(2.5));
        assert_eq!(b.compute(StatsMetric::Sum), Some(10.0));
        assert_eq!(b.compute(StatsMetric::Count), Some(4.0));
    }

    #[test]
    fn metric_empty_buffer_returns_none() {
        let b = RingBuf::new(64);
        assert_eq!(b.compute(StatsMetric::Avg), None);
    }
}
```

- [ ] **Step 3: Confirm failure**

Run: `cargo test metric_ --quiet` — FAIL (`compute` not defined).

- [ ] **Step 4: Implement `RingBuf::compute`**

```rust
pub fn compute(&self, metric: crate::dashboard::StatsMetric) -> Option<f64> {
    let vals: Vec<f64> = self.values().collect();
    if vals.is_empty() { return None; }
    use crate::dashboard::StatsMetric::*;
    Some(match metric {
        Min => vals.iter().cloned().fold(f64::INFINITY, f64::min),
        Max => vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        Sum => vals.iter().sum(),
        Avg => vals.iter().sum::<f64>() / vals.len() as f64,
        Count => vals.len() as f64,
    })
}
```

- [ ] **Step 5: `cargo test metric_ --quiet`** — PASS.

- [ ] **Step 6: Commit**

```bash
cargo fmt
git add src/dashboard/mod.rs src/util/history.rs
git commit -m "feat(stats): Statistics card variant + RingBuf::compute helper"
```

### Task 6.2: Renderer

**Files:**
- Create: `src/ui/widgets/card_statistics.rs`
- Modify: `src/ui/widgets/mod.rs`
- Modify: `src/screens/dashboard.rs`

- [ ] **Step 1: Create the file**

```rust
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::dashboard::{CardSize, StatsMetric};
use crate::ui::theme::{resolve_card_color, Theme};
use crate::ui::widgets::big_text;
use crate::util::history::RingBuf;

#[allow(clippy::too_many_arguments)]
pub fn render(
    f: &mut Frame,
    area: Rect,
    title: &str,
    instance: &str,
    history: Option<&RingBuf>,
    metric: StatsMetric,
    window: &str,
    unit: Option<&str>,
    card_color: Option<&str>,
    size: CardSize,
    theme: &Theme,
    selected: bool,
) {
    let color = resolve_card_color(card_color, instance, theme);
    let mut block = Block::bordered()
        .title(format!(" {title} "))
        .border_style(Style::new().fg(color));
    if selected {
        block = block.border_style(Style::new().fg(color).bold());
    }
    let inner = block.inner(area);
    f.render_widget(block, area);

    let value = history.and_then(|h| h.compute(metric));
    let text = match (value, unit) {
        (Some(v), Some(u)) => format!("{v:.2} {u}"),
        (Some(v), None) => format!("{v:.2}"),
        (None, _) => "—".into(),
    };
    let subtitle = format!("{} · {}", metric_label(metric), window);

    let [value_area, label_area] = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Fill(1),
        ratatui::layout::Constraint::Length(1),
    ]).areas(inner);

    if matches!(size, CardSize::Large) && big_text::fits(value_area) {
        big_text::render_big(f, value_area, &text, color);
    } else {
        f.render_widget(
            Paragraph::new(text).style(Style::new().fg(color)).alignment(Alignment::Center),
            value_area,
        );
    }
    f.render_widget(
        Paragraph::new(subtitle).style(Style::new().dim()).alignment(Alignment::Center),
        label_area,
    );
}

fn metric_label(m: StatsMetric) -> &'static str {
    match m {
        StatsMetric::Avg => "avg",
        StatsMetric::Min => "min",
        StatsMetric::Max => "max",
        StatsMetric::Sum => "sum",
        StatsMetric::Count => "count",
    }
}
```

Register module + dispatch.

- [ ] **Step 2: History fetch**

In the existing path that fans out `FetchHistory` on `HaInitialStates`, include `Statistics` cards (in addition to `Graph`). Search `rg 'FetchHistory' src/`; add the `Statistics` arm.

- [ ] **Step 3: `cargo build --quiet` and `cargo test --quiet`** — clean.

- [ ] **Step 4: Commit**

```bash
cargo fmt
git add src/ui/widgets/card_statistics.rs src/ui/widgets/mod.rs src/screens/dashboard.rs src/app.rs
git commit -m "feat(stats): Statistics card renderer + history backfill"
```

### Task 6.3: Editor add-flow + menu

**Files:**
- Modify: `src/dashboard/editor.rs`
- Modify: `src/screens/editor.rs`

- [ ] **Step 1: Add Statistics to the card type list** — prompts: instance, entity, metric (cycle Avg/Min/Max/Sum/Count), window, unit (optional), title (optional).

- [ ] **Step 2: Card context menu** — "Metric", "Window", "Unit".

- [ ] **Step 3: `cargo build --quiet` and `cargo test --quiet`** — clean.

- [ ] **Step 4: Commit**

```bash
cargo fmt
git add src/dashboard/editor.rs src/screens/editor.rs
git commit -m "feat(editor): Statistics card add-flow + metric/window/unit menu"
```

### Task 6.4: Milestone gate — clippy + test clean.

---

## Milestone 7: MediaPlayer card

### Task 7.1: Variant + service calls

**Files:**
- Modify: `src/dashboard/mod.rs`
- Modify: `src/actions.rs` (or add a new helper for media controls)

- [ ] **Step 1: Add `CardKind::MediaPlayer`**

```rust
MediaPlayer {
    instance: Alias,
    entity: EntityId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    title: Option<String>,
},
```

Update `Card::title` + `entity_ref`.

- [ ] **Step 2: Add a helper to enumerate media services**

In `src/actions.rs`, append:
```rust
pub fn media_service_for_key(k: char) -> Option<&'static str> {
    match k {
        ' ' => Some("media_play_pause"),
        'n' => Some("media_next_track"),
        'p' => Some("media_previous_track"),
        '+' => Some("volume_up"),
        '-' => Some("volume_down"),
        'm' => Some("volume_mute"),
        _ => None,
    }
}
```

(`volume_mute` toggles via the `is_volume_muted` attribute — pass `{ is_volume_muted: !current }` in service_data from the caller.)

- [ ] **Step 3: `cargo test --quiet`** — clean.

- [ ] **Step 4: Commit**

```bash
cargo fmt
git add src/dashboard/mod.rs src/actions.rs
git commit -m "feat(media): MediaPlayer card variant + media service mapping"
```

### Task 7.2: Renderer

**Files:**
- Create: `src/ui/widgets/card_media_player.rs`
- Modify: `src/ui/widgets/mod.rs`
- Modify: `src/screens/dashboard.rs`

- [ ] **Step 1: Create the file**

```rust
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::widgets::{Block, Gauge, Paragraph};
use ratatui::Frame;

use crate::dashboard::CardSize;
use crate::ha::EntityState;
use crate::ui::theme::{resolve_card_color, Theme};
use crate::ui::widgets::big_text;

#[allow(clippy::too_many_arguments)]
pub fn render(
    f: &mut Frame,
    area: Rect,
    title: &str,
    instance: &str,
    state: Option<&EntityState>,
    card_color: Option<&str>,
    size: CardSize,
    theme: &Theme,
    selected: bool,
) {
    let color = resolve_card_color(card_color, instance, theme);
    let mut block = Block::bordered()
        .title(format!(" {title} "))
        .border_style(Style::new().fg(color));
    if selected { block = block.border_style(Style::new().fg(color).bold()); }
    let inner = block.inner(area);
    f.render_widget(block, area);

    let st = state;
    let media_title = st.and_then(|s| s.attributes.get("media_title")).and_then(|v| v.as_str()).unwrap_or("—").to_string();
    let media_artist = st.and_then(|s| s.attributes.get("media_artist")).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let app_name = st.and_then(|s| s.attributes.get("app_name")).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let volume = st.and_then(|s| s.attributes.get("volume_level")).and_then(|v| v.as_f64()).unwrap_or(0.0);
    let muted = st.and_then(|s| s.attributes.get("is_volume_muted")).and_then(|v| v.as_bool()).unwrap_or(false);
    let player_state = st.map(|s| s.state.as_str()).unwrap_or("off");

    let [t_row, a_row, app_row, gauge_row, state_row] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ]).areas(inner);

    if matches!(size, CardSize::Large) && big_text::fits(t_row) {
        big_text::render_big(f, t_row, &media_title, color);
    } else {
        f.render_widget(Paragraph::new(media_title).style(Style::new().fg(color)), t_row);
    }
    f.render_widget(Paragraph::new(media_artist).style(Style::new().dim()), a_row);
    f.render_widget(Paragraph::new(app_name).style(Style::new().dim()), app_row);
    let vol_label = if muted { "muted".to_string() } else { format!("vol {:.0}%", volume * 100.0) };
    f.render_widget(
        Gauge::default()
            .ratio(if muted { 0.0 } else { volume.clamp(0.0, 1.0) })
            .label(vol_label)
            .gauge_style(Style::new().fg(color)),
        gauge_row,
    );
    f.render_widget(
        Paragraph::new(format!("[{player_state}]")).style(Style::new().fg(Color::DarkGray)),
        state_row,
    );
}
```

Register module + dispatch.

- [ ] **Step 2: Keybindings — selection-scoped**

In `src/app.rs`, where dashboard key handling lives: when `Screen::Dashboard` and the selected card is `MediaPlayer`, intercept the keys before generic dispatch. For each match from `media_service_for_key`, send `HaCommand::CallService { domain: "media_player", service, target_entity, data }`.

- [ ] **Step 3: `cargo build --quiet` and `cargo test --quiet`** — clean.

- [ ] **Step 4: Commit**

```bash
cargo fmt
git add src/ui/widgets/card_media_player.rs src/ui/widgets/mod.rs src/screens/dashboard.rs src/app.rs
git commit -m "feat(media): MediaPlayer renderer + selection-scoped keybindings"
```

### Task 7.3: Editor add-flow

**Files:**
- Modify: `src/dashboard/editor.rs`
- Modify: `src/screens/editor.rs`

- [ ] **Step 1: Add MediaPlayer to the type list** — restrict entity picker to `media_player.*` domain.

- [ ] **Step 2: `cargo build --quiet` and `cargo test --quiet`** — clean.

- [ ] **Step 3: Commit**

```bash
cargo fmt
git add src/dashboard/editor.rs src/screens/editor.rs
git commit -m "feat(editor): MediaPlayer add-flow"
```

### Task 7.4: Milestone gate — clippy + test clean.

---

## Milestone 8: Image card

### Task 8.1: Add deps + create `src/ha/rest.rs`

**Files:**
- Modify: `Cargo.toml`
- Create: `src/ha/rest.rs`
- Modify: `src/ha/mod.rs`

- [ ] **Step 1: Add deps**

```toml
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls-native-roots", "stream"] }
ratatui-image = "8"
image = "0.25"
```

(Verify ratatui-image's ratatui 0.30 compatibility at install. If incompatible, drop the dep and adapt step 2 of Task 8.4 to use the halfblocks-only manual path described in the spec.)

Run: `cargo build --quiet` — clean.

- [ ] **Step 2: Create `src/ha/rest.rs`**

```rust
use crate::config::InstanceConfig;
use reqwest::Client;
use std::sync::OnceLock;
use url::Url;

static CLIENT: OnceLock<Client> = OnceLock::new();

fn client() -> &'static Client {
    CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("reqwest client builds")
    })
}

pub fn https_base_from_ws(ws_url: &str) -> Result<Url, String> {
    let url = Url::parse(ws_url).map_err(|e| e.to_string())?;
    let mut https = url.clone();
    let scheme = match url.scheme() {
        "ws" => "http",
        "wss" => "https",
        other => return Err(format!("unexpected scheme: {other}")),
    };
    https.set_scheme(scheme).map_err(|_| "scheme replace failed".to_string())?;
    https.set_path("/");
    Ok(https)
}

pub async fn fetch_image_proxy(
    instance: &InstanceConfig,
    entity_id: &str,
    token: &str,
) -> Result<Vec<u8>, String> {
    let base = https_base_from_ws(&instance.url)?;
    let url = base.join(&format!("api/image_proxy/{entity_id}")).map_err(|e| e.to_string())?;
    fetch(url, token).await
}

pub async fn fetch_camera_proxy(
    instance: &InstanceConfig,
    entity_id: &str,
    token: &str,
) -> Result<Vec<u8>, String> {
    let base = https_base_from_ws(&instance.url)?;
    let url = base.join(&format!("api/camera_proxy/{entity_id}")).map_err(|e| e.to_string())?;
    fetch(url, token).await
}

async fn fetch(url: Url, token: &str) -> Result<Vec<u8>, String> {
    let resp = client()
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    Ok(bytes.to_vec())
}
```

(Adjust `InstanceConfig` field names to match your actual struct — see `src/config/load.rs`.)

- [ ] **Step 3: Register in `src/ha/mod.rs`** — `pub mod rest;`.

- [ ] **Step 4: Test `https_base_from_ws`**

In `src/ha/rest.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ws_to_https() {
        let u = https_base_from_ws("wss://ha.example/api/websocket").unwrap();
        assert_eq!(u.scheme(), "https");
        assert_eq!(u.host_str(), Some("ha.example"));
    }
    #[test]
    fn ws_plaintext() {
        let u = https_base_from_ws("ws://ha.local:8123/api/websocket").unwrap();
        assert_eq!(u.scheme(), "http");
    }
}
```

Run: `cargo test ws_ --quiet` — PASS.

- [ ] **Step 5: Commit**

```bash
cargo fmt
git add Cargo.toml Cargo.lock src/ha/rest.rs src/ha/mod.rs
git commit -m "feat(ha): REST client for image_proxy + camera_proxy"
```

### Task 8.2: `Image` card variant

**Files:**
- Modify: `src/dashboard/mod.rs`

- [ ] **Step 1: Add variant + supporting enum**

```rust
Image {
    instance: Alias,
    source: ImageSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    refresh_seconds: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    title: Option<String>,
},
```

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ImageSource {
    ImageEntity { entity: EntityId },
    Camera { entity: EntityId },
}
```

Update `Card::title` + `entity_ref`.

- [ ] **Step 2: Round-trip test**

```rust
#[test]
fn image_card_round_trip() {
    let yaml = r#"
type: image
instance: home
source: { kind: camera, entity: camera.front_door }
refresh_seconds: 30
pos: { col: 0, row: 0, w: 6, h: 4 }
"#;
    let card: Card = serde_yaml::from_str(yaml).unwrap();
    if let CardKind::Image { source, refresh_seconds, .. } = &card.kind {
        assert!(matches!(source, ImageSource::Camera { .. }));
        assert_eq!(*refresh_seconds, Some(30));
    } else { panic!() }
}
```

Run: `cargo test image_card_ --quiet` — PASS.

- [ ] **Step 3: Commit**

```bash
cargo fmt
git add src/dashboard/mod.rs
git commit -m "feat(dashboard): add Image card variant + ImageSource enum"
```

### Task 8.3: Fetch plumbing — `HaCommand` + `AppEvent` + handler

**Files:**
- Modify: `src/ha/client.rs` (or wherever `HaCommand` is defined)
- Modify: `src/event.rs`
- Modify: `src/app.rs`

- [ ] **Step 1: Add `HaCommand::FetchImageBytes`**

```rust
HaCommand::FetchImageBytes {
    entity: EntityId,
    kind: ImageFetchKind,
}
```

```rust
pub enum ImageFetchKind { Image, Camera }
```

- [ ] **Step 2: Handle the command in the per-instance task**

In `src/ha/client.rs`'s command loop, on `FetchImageBytes`:
```rust
HaCommand::FetchImageBytes { entity, kind } => {
    let token = token_for_instance(&cfg);
    let bytes_res = match kind {
        ImageFetchKind::Image => rest::fetch_image_proxy(&cfg, entity.as_str(), &token).await,
        ImageFetchKind::Camera => rest::fetch_camera_proxy(&cfg, entity.as_str(), &token).await,
    };
    let _ = tx.send(AppEvent::HaImageBytes {
        instance: cfg.alias.clone(),
        entity,
        result: bytes_res,
    });
}
```

- [ ] **Step 3: Add `AppEvent::HaImageBytes`**

In `src/event.rs`:
```rust
HaImageBytes {
    instance: Alias,
    entity: EntityId,
    result: Result<Vec<u8>, String>,
},
```

- [ ] **Step 4: Handle in `App::handle_event` (in `src/app.rs`)**

Decode the bytes via `image::load_from_memory(&bytes)`, build a `ratatui_image::picker::Picker`'s `StatefulProtocol`, store keyed by `(instance, entity)`. On error, set `last_error`.

- [ ] **Step 5: `cargo build --quiet` and `cargo test --quiet`** — clean.

- [ ] **Step 6: Commit**

```bash
cargo fmt
git add src/ha src/event.rs src/app.rs
git commit -m "feat(ha): FetchImageBytes command + HaImageBytes event"
```

### Task 8.4: Renderer + per-card refresh timer

**Files:**
- Create: `src/ui/widgets/card_image.rs`
- Modify: `src/ui/widgets/mod.rs`
- Modify: `src/screens/dashboard.rs`
- Modify: `src/app.rs`

- [ ] **Step 1: Create the file**

```rust
use ratatui::layout::Rect;
use ratatui::style::{Color, Style, Stylize};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;
use ratatui_image::StatefulImage;
use ratatui_image::protocol::StatefulProtocol;

use crate::ui::theme::{resolve_card_color, Theme};

#[allow(clippy::too_many_arguments)]
pub fn render(
    f: &mut Frame,
    area: Rect,
    title: &str,
    instance: &str,
    protocol: Option<&mut StatefulProtocol>,
    error: Option<&str>,
    card_color: Option<&str>,
    theme: &Theme,
    selected: bool,
) {
    let color = resolve_card_color(card_color, instance, theme);
    let mut block = Block::bordered()
        .title(format!(" {title} "))
        .border_style(Style::new().fg(color));
    if selected { block = block.border_style(Style::new().fg(color).bold()); }
    let inner = block.inner(area);
    f.render_widget(block, area);

    if let Some(err) = error {
        f.render_widget(Paragraph::new(format!("Image unavailable\n{err}")).style(Style::new().fg(Color::Red)), inner);
        return;
    }
    if let Some(p) = protocol {
        f.render_stateful_widget(StatefulImage::default(), inner, p);
    } else {
        f.render_widget(Paragraph::new("(loading)").style(Style::new().fg(Color::DarkGray)), inner);
    }
}
```

- [ ] **Step 2: Per-card refresh tick**

In `src/app.rs`, when building `App`, for every `Image { source: Camera, refresh_seconds: Some(n), .. }` card across all dashboards, spawn a tokio interval that emits an internal `AppEvent::RefreshImage { instance, entity }`. (Add the variant to `AppEvent`.) On receipt, send `HaCommand::FetchImageBytes`.

For `ImageEntity` images, refetch on `HaEntityUpdated` when the entity is in `image.*` domain and its state changed.

- [ ] **Step 3: First-render fetch**

When the dashboard first paints an Image card and no protocol exists for its key, issue one fetch.

- [ ] **Step 4: `cargo build --quiet` and `cargo test --quiet`** — clean.

- [ ] **Step 5: Manual smoke test** — wire a real camera entity in dashboards, run the binary, confirm the image renders and refreshes.

- [ ] **Step 6: Commit**

```bash
cargo fmt
git add src/ui/widgets/card_image.rs src/ui/widgets/mod.rs src/screens/dashboard.rs src/app.rs src/event.rs
git commit -m "feat(image): Image card renderer + camera refresh timer"
```

### Task 8.5: Editor add-flow

**Files:**
- Modify: `src/dashboard/editor.rs`
- Modify: `src/screens/editor.rs`

- [ ] **Step 1: Add Image to the type list** — prompts: source kind (Image / Camera), entity (filtered to that domain), refresh_seconds if Camera, title (optional).

- [ ] **Step 2: `cargo build --quiet` and `cargo test --quiet`** — clean.

- [ ] **Step 3: Commit**

```bash
cargo fmt
git add src/dashboard/editor.rs src/screens/editor.rs
git commit -m "feat(editor): Image card add-flow with image.*/camera.* picker"
```

### Task 8.6: Milestone gate — clippy + test clean.

---

## Milestone 9: Weather card

### Task 9.1: Variant

**Files:**
- Modify: `src/dashboard/mod.rs`

- [ ] **Step 1: Add variant**

```rust
Weather {
    instance: Alias,
    entity: EntityId,
    #[serde(default = "default_true")]
    show_forecast: bool,
    #[serde(default = "default_forecast_days")]
    forecast_days: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    title: Option<String>,
},
```

```rust
fn default_forecast_days() -> u8 { 3 }
```

Update `Card::title` + `entity_ref`.

- [ ] **Step 2: Round-trip test + commit** (mirror the pattern from Image task).

```bash
cargo fmt
git add src/dashboard/mod.rs
git commit -m "feat(dashboard): add Weather card variant"
```

### Task 9.2: Forecast fetch over WS

**Files:**
- Modify: `src/ha/protocol.rs`
- Modify: `src/ha/client.rs`
- Modify: `src/event.rs`
- Modify: `src/app.rs`

- [ ] **Step 1: Add `HaCommand::GetWeatherForecast { entity, kind: ForecastKind }`**

```rust
pub enum ForecastKind { Daily, Hourly }
```

- [ ] **Step 2: Build the WS request**

In `src/ha/protocol.rs`, add a `call_service` helper for `weather.get_forecasts` with `service_data: { type: "daily" }` and `target: { entity_id: <entity> }`. The response is a service call result with `service_response`.

- [ ] **Step 3: Add `AppEvent::HaWeatherForecast { instance, entity, forecast: Vec<ForecastDay> }`**

Define `ForecastDay { datetime: DateTime<Utc>, condition: String, temperature: f64, templow: Option<f64>, … }` in `src/ha/mod.rs`.

- [ ] **Step 4: Cache on the app**

In `App`, add `weather_forecasts: HashMap<(Alias, EntityId), Vec<ForecastDay>>`.

- [ ] **Step 5: Trigger fetch**

On `HaInitialStates`, for every Weather card on every dashboard for that instance, send `GetWeatherForecast`. Re-fetch every 30 minutes via a tokio interval (one shared timer that loops over all weather cards).

- [ ] **Step 6: `cargo build --quiet` and `cargo test --quiet`** — clean.

- [ ] **Step 7: Commit**

```bash
cargo fmt
git add src/ha src/event.rs src/app.rs
git commit -m "feat(weather): WS-based forecast fetch + cache"
```

### Task 9.3: Renderer

**Files:**
- Create: `src/ui/widgets/card_weather.rs`
- Modify: `src/ui/widgets/mod.rs`
- Modify: `src/screens/dashboard.rs`

- [ ] **Step 1: Create the file**

```rust
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::dashboard::CardSize;
use crate::ha::{EntityState, ForecastDay};
use crate::ui::theme::{resolve_card_color, Theme};

#[allow(clippy::too_many_arguments)]
pub fn render(
    f: &mut Frame,
    area: Rect,
    title: &str,
    instance: &str,
    state: Option<&EntityState>,
    forecast: &[ForecastDay],
    show_forecast: bool,
    forecast_days: u8,
    card_color: Option<&str>,
    size: CardSize,
    theme: &Theme,
    selected: bool,
) {
    let color = resolve_card_color(card_color, instance, theme);
    let mut block = Block::bordered()
        .title(format!(" {title} "))
        .border_style(Style::new().fg(color));
    if selected { block = block.border_style(Style::new().fg(color).bold()); }
    let inner = block.inner(area);
    f.render_widget(block, area);

    let cond = state.map(|s| s.state.as_str()).unwrap_or("unknown");
    let temp = state.and_then(|s| s.attributes.get("temperature")).and_then(|v| v.as_f64());
    let humidity = state.and_then(|s| s.attributes.get("humidity")).and_then(|v| v.as_f64());
    let wind = state.and_then(|s| s.attributes.get("wind_speed")).and_then(|v| v.as_f64());

    let header = format!(
        "{glyph}  {temp}°  {cond}",
        glyph = condition_glyph(cond),
        temp = temp.map(|t| format!("{t:.1}")).unwrap_or_else(|| "—".into()),
    );

    if matches!(size, CardSize::Small) {
        f.render_widget(Paragraph::new(header).style(Style::new().fg(color)), inner);
        return;
    }

    let rows = if matches!(size, CardSize::Large) || !show_forecast {
        Layout::vertical([Constraint::Length(2), Constraint::Length(1)]).split(inner)
    } else {
        Layout::vertical([Constraint::Length(2), Constraint::Length(1), Constraint::Fill(1)]).split(inner)
    };
    f.render_widget(Paragraph::new(header).style(Style::new().fg(color)), rows[0]);
    let sub = format!(
        "h {h}%   w {w}",
        h = humidity.map(|v| format!("{v:.0}")).unwrap_or_else(|| "—".into()),
        w = wind.map(|v| format!("{v:.0}")).unwrap_or_else(|| "—".into()),
    );
    f.render_widget(Paragraph::new(sub).style(Style::new().dim()), rows[1]);

    if !matches!(size, CardSize::Large) && show_forecast && rows.len() == 3 {
        let take = forecast_days as usize;
        let mut lines = Vec::with_capacity(take.min(forecast.len()));
        for day in forecast.iter().take(take) {
            let date = day.datetime.format("%a").to_string();
            lines.push(format!(
                "{date}  {g}  {t:.0}°{lo}",
                date = date,
                g = condition_glyph(&day.condition),
                t = day.temperature,
                lo = day.templow.map(|l| format!(" / {l:.0}°")).unwrap_or_default(),
            ));
        }
        f.render_widget(
            Paragraph::new(lines.join("\n")).style(Style::new().fg(Color::Gray)),
            rows[2],
        );
    }
    let _ = Alignment::Center;
}

fn condition_glyph(c: &str) -> &'static str {
    match c {
        "sunny" | "clear-day" => "☀",
        "partlycloudy" | "partly-cloudy" => "⛅",
        "cloudy" => "☁",
        "rainy" => "🌧",
        "pouring" => "🌧",
        "snowy" => "❄",
        "fog" => "🌫",
        "lightning" | "lightning-rainy" => "⛈",
        _ => "·",
    }
}
```

- [ ] **Step 2: Register module + dispatch** in `mod.rs` and `screens/dashboard.rs`.

- [ ] **Step 3: `cargo build --quiet` and `cargo test --quiet`** — clean.

- [ ] **Step 4: Commit**

```bash
cargo fmt
git add src/ui/widgets/card_weather.rs src/ui/widgets/mod.rs src/screens/dashboard.rs
git commit -m "feat(weather): Weather card renderer with forecast strip"
```

### Task 9.4: Editor add-flow

**Files:**
- Modify: `src/dashboard/editor.rs`
- Modify: `src/screens/editor.rs`

- [ ] **Step 1: Add Weather to the type list** — entity restricted to `weather.*`; prompt show_forecast (y/n), forecast_days (1..=7), title (optional).

- [ ] **Step 2: `cargo build --quiet` and `cargo test --quiet`** — clean.

- [ ] **Step 3: Commit**

```bash
cargo fmt
git add src/dashboard/editor.rs src/screens/editor.rs
git commit -m "feat(editor): Weather card add-flow"
```

### Task 9.5: Milestone gate — clippy + test clean.

---

## Final: README, examples, snapshot review

### Task F.1: Update README

**Files:**
- Modify: `README.md`
- Modify: `config/dashboards.example.yaml`

- [ ] **Step 1: README** — add a section per new card type with one YAML example each, and a note on the `color` + `size` overrides.

- [ ] **Step 2: Example dashboards** — include one card of each new type in `config/dashboards.example.yaml`.

- [ ] **Step 3: Commit**

```bash
git add README.md config/dashboards.example.yaml
git commit -m "docs: README + example dashboards for new card types"
```

### Task F.2: Insta snapshots

- [ ] **Step 1: `cargo test --quiet`** — generates any new pending snapshots.
- [ ] **Step 2: `cargo insta review`** — accept reasonable snapshots.
- [ ] **Step 3: `git add tests/ && git commit -m "test: insta snapshots for new cards"`** (or commit alongside the milestone that introduced them).

### Task F.3: Update `primer.md`

- [ ] **Step 1:** Rewrite `primer.md` to reflect the new card types and milestone completion. Document any deferred work or known limitations (e.g. ratatui-image fallback if used).
- [ ] **Step 2:** Commit `docs: update primer for card improvements`.

### Task F.4: Final verification

- [ ] **Step 1: `cargo fmt -- --check`** — clean
- [ ] **Step 2: `cargo clippy --all-targets --quiet`** — clean
- [ ] **Step 3: `cargo test --quiet`** — clean
- [ ] **Step 4: Manual end-to-end** — run against a real HA instance with one of every card type configured; verify each renders and interacts as designed.

---

## Self-review notes

- Each new card type has: variant definition, serde round-trip test, renderer, dispatch wiring, editor add-flow, milestone gate.
- The Graph rewrite is split into 7 sub-tasks so the rename, normalization, and three render modes each commit independently.
- Image and Weather are isolated to their own milestones because they introduce new IO paths (REST + WS forecast) — placing them last reduces risk of breaking earlier milestones.
- Old YAML compatibility is preserved through serde aliases (`sparkline → graph`) and default-valued additive fields (severity, needle, size, color, hide_when_empty).
- The plan does not introduce abstractions beyond the per-card widget pattern already in use.
