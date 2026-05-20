pub mod editor;
pub mod layout;
pub mod persist;
pub mod query;

use std::fmt;

use serde::de::Visitor;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::config::Alias;
use crate::ha::EntityId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardFile {
    pub dashboards: Vec<Dashboard>,
}

/// The layout strategy for a dashboard.
#[derive(Debug, Clone)]
pub enum DashboardLayout {
    /// Free-canvas layout: cards have explicit (col, row, w, h) positions.
    Free { grid: Grid, cards: Vec<Card> },
    /// Stacked-column layout: rows of columns, cards fill column width.
    Grid { rows: Vec<GridRow> },
}

/// A dashboard with a name and a layout (Free or Grid).
/// Serializes as `{name, grid, cards}` for Free (backward-compat with existing YAML)
/// and as `{name, type: grid, rows: [...]}` for Grid.
#[derive(Debug, Clone)]
pub struct Dashboard {
    pub name: String,
    pub layout: DashboardLayout,
}

// ── raw serde intermediate ──────────────────────────────────────────────────

#[derive(Deserialize)]
struct DashboardRaw {
    name: String,
    #[serde(rename = "type", default)]
    layout_type: Option<String>,
    #[serde(default)]
    grid: Option<Grid>,
    #[serde(default)]
    cards: Option<Vec<Card>>,
    #[serde(default)]
    rows: Option<Vec<GridRow>>,
}

impl<'de> Deserialize<'de> for Dashboard {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let raw = DashboardRaw::deserialize(de)?;
        let layout = match (raw.layout_type.as_deref(), raw.grid, raw.cards, raw.rows) {
            // Explicit `type: free` or inferred from having grid+cards and no rows
            (None | Some("free"), Some(grid), cards_opt, None) => {
                DashboardLayout::Free { grid, cards: cards_opt.unwrap_or_default() }
            }
            (Some("free"), _, _, Some(_)) => {
                return Err(serde::de::Error::custom(
                    "dashboard has `type: free` but also `rows` — remove one",
                ));
            }
            // Explicit `type: grid`
            (Some("grid"), grid_opt, cards_opt, rows_opt) => {
                if grid_opt.is_some() || cards_opt.is_some() {
                    return Err(serde::de::Error::custom(
                        "dashboard has `type: grid` but also `grid`/`cards` fields — remove them",
                    ));
                }
                DashboardLayout::Grid { rows: rows_opt.unwrap_or_default() }
            }
            // Inferred from rows presence
            (None, None, None, Some(rows)) => DashboardLayout::Grid { rows },
            // Conflicting free+grid signals
            (None, Some(_), _, Some(_)) => {
                return Err(serde::de::Error::custom(
                    "ambiguous dashboard layout: has both `grid`/`cards` and `rows`; add `type: free` or `type: grid`",
                ));
            }
            // Nothing useful
            (None, None, None, None) => {
                return Err(serde::de::Error::custom(
                    "dashboard has neither `grid`/`cards` (free layout) nor `rows` (grid layout)",
                ));
            }
            // None + has cards but no grid — partial free layout
            (None, None, Some(_), None) => {
                return Err(serde::de::Error::custom(
                    "free dashboard has `cards` but no `grid`",
                ));
            }
            (t, _, _, _) => {
                return Err(serde::de::Error::custom(format!(
                    "unrecognized dashboard type {:?}",
                    t
                )));
            }
        };
        Ok(Dashboard { name: raw.name, layout })
    }
}

impl Serialize for Dashboard {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        match &self.layout {
            DashboardLayout::Free { grid, cards } => {
                let mut m = s.serialize_map(Some(3))?;
                m.serialize_entry("name", &self.name)?;
                m.serialize_entry("grid", grid)?;
                m.serialize_entry("cards", cards)?;
                m.end()
            }
            DashboardLayout::Grid { rows } => {
                let mut m = s.serialize_map(Some(3))?;
                m.serialize_entry("name", &self.name)?;
                m.serialize_entry("type", "grid")?;
                m.serialize_entry("rows", rows)?;
                m.end()
            }
        }
    }
}

// ── Dashboard helpers ───────────────────────────────────────────────────────

impl Dashboard {
    /// Return the free-canvas grid, or `None` for grid-layout dashboards.
    pub fn free_grid(&self) -> Option<Grid> {
        match &self.layout {
            DashboardLayout::Free { grid, .. } => Some(*grid),
            DashboardLayout::Grid { .. } => None,
        }
    }

    /// Return `true` if this is a free-canvas dashboard.
    pub fn is_free(&self) -> bool {
        matches!(self.layout, DashboardLayout::Free { .. })
    }

    /// Iterate all cards in flat order (free: by index; grid: row-major).
    pub fn cards_iter(&self) -> Box<dyn Iterator<Item = &Card> + '_> {
        match &self.layout {
            DashboardLayout::Free { cards, .. } => Box::new(cards.iter()),
            DashboardLayout::Grid { rows } => Box::new(
                rows.iter()
                    .flat_map(|r| r.columns.iter())
                    .flat_map(|c| c.cards.iter()),
            ),
        }
    }

    /// Iterate all cards mutably in flat order.
    pub fn cards_iter_mut(&mut self) -> Box<dyn Iterator<Item = &mut Card> + '_> {
        match &mut self.layout {
            DashboardLayout::Free { cards, .. } => Box::new(cards.iter_mut()),
            DashboardLayout::Grid { rows } => Box::new(
                rows.iter_mut()
                    .flat_map(|r| r.columns.iter_mut())
                    .flat_map(|c| c.cards.iter_mut()),
            ),
        }
    }

    /// Get card by flat index.
    pub fn card(&self, idx: usize) -> Option<&Card> {
        self.cards_iter().nth(idx)
    }

    /// Get card mutably by flat index.
    pub fn card_mut(&mut self, idx: usize) -> Option<&mut Card> {
        self.cards_iter_mut().nth(idx)
    }

    /// Total card count.
    pub fn card_count(&self) -> usize {
        self.cards_iter().count()
    }

    /// Get card by stable ID.
    pub fn card_by_id(&self, id: CardId) -> Option<&Card> {
        self.cards_iter().find(|c| c.id == id)
    }

    /// Get card mutably by stable ID.
    pub fn card_by_id_mut(&mut self, id: CardId) -> Option<&mut Card> {
        self.cards_iter_mut().find(|c| c.id == id)
    }

    /// Resolve a `CardId` to its flat index.
    pub fn flat_idx_of(&self, id: CardId) -> Option<usize> {
        self.cards_iter().position(|c| c.id == id)
    }

    /// Remove a card by stable ID; returns the removed card.
    pub fn remove_card_by_id(&mut self, id: CardId) -> Option<Card> {
        match &mut self.layout {
            DashboardLayout::Free { cards, .. } => {
                let idx = cards.iter().position(|c| c.id == id)?;
                Some(cards.remove(idx))
            }
            DashboardLayout::Grid { rows } => {
                for row in rows {
                    for col in &mut row.columns {
                        if let Some(idx) = col.cards.iter().position(|c| c.id == id) {
                            return Some(col.cards.remove(idx));
                        }
                    }
                }
                None
            }
        }
    }

    /// Append a card to a free-canvas dashboard. Panics if called on a grid dashboard.
    pub fn push_card_free(&mut self, card: Card) {
        let DashboardLayout::Free { cards, .. } = &mut self.layout else {
            panic!("push_card_free called on grid-layout dashboard");
        };
        cards.push(card);
    }

    /// Append a card to a specific column of a grid-layout dashboard.
    pub fn insert_card_grid(&mut self, row_idx: usize, col_idx: usize, card: Card) {
        let DashboardLayout::Grid { rows } = &mut self.layout else {
            panic!("insert_card_grid called on free-layout dashboard");
        };
        if let Some(row) = rows.get_mut(row_idx) {
            if let Some(col) = row.columns.get_mut(col_idx) {
                col.cards.push(card);
            }
        }
    }

    /// Locate a card's (row, col, pos_in_col) in a grid-layout dashboard.
    pub fn locate_grid(&self, id: CardId) -> Option<(usize, usize, usize)> {
        let DashboardLayout::Grid { rows } = &self.layout else {
            return None;
        };
        for (ri, row) in rows.iter().enumerate() {
            for (ci, col) in row.columns.iter().enumerate() {
                if let Some(pi) = col.cards.iter().position(|c| c.id == id) {
                    return Some((ri, ci, pi));
                }
            }
        }
        None
    }

    /// Compute the next unique `CardId` for this dashboard (max existing + 1).
    pub fn next_card_id(&self) -> CardId {
        let max = self.cards_iter().map(|c| c.id.0).max().unwrap_or(0);
        CardId(max + 1)
    }

    /// Remove a free-canvas card by flat index. Returns the removed card.
    pub fn remove_card_at_free(&mut self, idx: usize) -> Option<Card> {
        let DashboardLayout::Free { cards, .. } = &mut self.layout else {
            return None;
        };
        if idx < cards.len() {
            Some(cards.remove(idx))
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Grid {
    pub cols: u16,
    pub rows: u16,
}

// ── Grid-layout types ──────────────────────────────────────────────────────

/// Row height: either a fixed number of terminal rows, or auto (fills remaining).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowHeight {
    Fixed(u16),
    Auto,
}

impl Serialize for RowHeight {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            RowHeight::Fixed(n) => s.serialize_u16(*n),
            RowHeight::Auto => s.serialize_str("auto"),
        }
    }
}

impl<'de> Deserialize<'de> for RowHeight {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = RowHeight;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "an integer row height or the string \"auto\"")
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<RowHeight, E> {
                Ok(RowHeight::Fixed(v as u16))
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<RowHeight, E> {
                if v > 0 {
                    Ok(RowHeight::Fixed(v as u16))
                } else {
                    Err(E::custom("row height must be positive"))
                }
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<RowHeight, E> {
                if v.eq_ignore_ascii_case("auto") {
                    Ok(RowHeight::Auto)
                } else {
                    Err(E::custom(format!("expected \"auto\" or integer, got {:?}", v)))
                }
            }
        }
        de.deserialize_any(V)
    }
}

/// One column inside a `GridRow`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridColumn {
    /// Override `fill_height` from the parent row. `None` inherits the row default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill_height: Option<bool>,
    #[serde(default)]
    pub cards: Vec<Card>,
}

impl GridColumn {
    /// Resolve fill_height, falling back to the row-level default.
    pub fn effective_fill_height(&self, row_default: bool) -> bool {
        self.fill_height.unwrap_or(row_default)
    }
}

/// One horizontal band of columns in a grid-layout dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridRow {
    pub height: RowHeight,
    /// Default `fill_height` for all columns in this row; overrideable per column.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill_height: Option<bool>,
    pub columns: Vec<GridColumn>,
}

impl GridRow {
    pub fn fill_height_default(&self) -> bool {
        self.fill_height.unwrap_or(false)
    }
}

// ── Dashboard layout ────────────────────────────────────────────────────────

/// Discriminant used in the custom `Dashboard` serde impl.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutKind {
    Free,
    Grid,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CardSize {
    Small,
    #[default]
    Normal,
    Large,
}

/// Stable per-card identity. Assigned at load time; zero means unassigned.
/// Hand-written YAML may omit this field; `persist::load` assigns fresh IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct CardId(pub u64);

impl CardId {
    pub const ZERO: CardId = CardId(0);
    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
    #[serde(default, skip_serializing_if = "CardId::is_zero")]
    pub id: CardId,
    #[serde(flatten)]
    pub kind: CardKind,
    /// Present on free-layout cards; absent on grid-layout cards.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<Pos>,
    /// Optional explicit height (terminal rows) for grid-layout cards.
    /// Overrides the per-kind natural height heuristic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "is_default_size")]
    pub size: CardSize,
}

fn is_default_size(s: &CardSize) -> bool {
    *s == CardSize::Normal
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Severity {
    pub green: f64,
    pub yellow: f64,
    pub red: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Pos {
    pub col: u16,
    pub row: u16,
    pub w: u16,
    pub h: u16,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ImageSource {
    ImageEntity { entity: EntityId },
    Camera { entity: EntityId },
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BarOrientation {
    #[default]
    Vertical,
    Horizontal,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StatsMetric {
    Avg,
    Min,
    Max,
    Sum,
    Count,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CardKind {
    Entity {
        instance: Alias,
        entity: EntityId,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        ticker: bool,
    },
    Toggle {
        instance: Alias,
        entity: EntityId,
        #[serde(default)]
        title: Option<String>,
    },
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
    Text {
        markdown: String,
        #[serde(default)]
        title: Option<String>,
    },
    EntityList {
        instance: Alias,
        entities: Vec<EntityId>,
        #[serde(default)]
        title: Option<String>,
    },
    FilteredEntityList {
        instance: Alias,
        /// Query string: `glob[filter1][filter2]...`
        query: String,
        #[serde(default)]
        hide_state: bool,
        #[serde(default)]
        hide_when_empty: bool,
        #[serde(default)]
        title: Option<String>,
    },
    Clock {
        #[serde(default = "default_clock_format")]
        format: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timezone: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
    },
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
    MediaPlayer {
        instance: Alias,
        entity: EntityId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
    },
    Image {
        instance: Alias,
        source: ImageSource,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        refresh_seconds: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
    },
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
}

fn default_window() -> String {
    "1h".into()
}

fn default_clock_format() -> String {
    "%H:%M:%S".into()
}

fn default_forecast_days() -> u8 {
    3
}

impl Card {
    pub fn title(&self) -> &str {
        match &self.kind {
            CardKind::Entity { title, entity, .. }
            | CardKind::Toggle { title, entity, .. }
            | CardKind::Gauge { title, entity, .. } => title.as_deref().unwrap_or(entity),
            CardKind::Graph {
                title,
                entity,
                entities,
                ..
            } => title.as_deref().unwrap_or_else(|| {
                entities
                    .first()
                    .map(|s| s.entity.as_str())
                    .or(entity.as_deref())
                    .unwrap_or("Graph")
            }),
            CardKind::Text { title, .. } => title.as_deref().unwrap_or("Text"),
            CardKind::EntityList { title, .. } => title.as_deref().unwrap_or("Entities"),
            CardKind::FilteredEntityList { title, .. } => title.as_deref().unwrap_or("Filtered"),
            CardKind::Clock { title, .. } => title.as_deref().unwrap_or("Clock"),
            CardKind::Statistics { title, entity, .. } => {
                title.as_deref().unwrap_or(entity.as_str())
            }
            CardKind::MediaPlayer { title, entity, .. } => {
                title.as_deref().unwrap_or(entity.as_str())
            }
            CardKind::Image { title, source, .. } => title.as_deref().unwrap_or(match source {
                ImageSource::ImageEntity { entity } | ImageSource::Camera { entity } => {
                    entity.as_str()
                }
            }),
            CardKind::Weather { title, entity, .. } => title.as_deref().unwrap_or(entity.as_str()),
        }
    }

    pub fn entity_ref(&self) -> Option<(&Alias, &EntityId)> {
        match &self.kind {
            CardKind::Entity {
                instance, entity, ..
            }
            | CardKind::Toggle {
                instance, entity, ..
            }
            | CardKind::Gauge {
                instance, entity, ..
            } => Some((instance, entity)),
            CardKind::Graph {
                instance,
                entity,
                entities,
                ..
            } => {
                if !entities.is_empty() {
                    Some((instance, &entities[0].entity))
                } else if let Some(e) = entity {
                    Some((instance, e))
                } else {
                    None
                }
            }
            CardKind::Statistics {
                instance, entity, ..
            }
            | CardKind::MediaPlayer {
                instance, entity, ..
            }
            | CardKind::Weather {
                instance, entity, ..
            } => Some((instance, entity)),
            CardKind::Image {
                instance, source, ..
            } => Some((
                instance,
                match source {
                    ImageSource::ImageEntity { entity } | ImageSource::Camera { entity } => entity,
                },
            )),
            CardKind::Text { .. }
            | CardKind::EntityList { .. }
            | CardKind::FilteredEntityList { .. }
            | CardKind::Clock { .. } => None,
        }
    }

    pub fn graph_entities(&self) -> Vec<&EntityId> {
        match &self.kind {
            CardKind::Graph {
                entity, entities, ..
            } => {
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

    pub fn normalize(&mut self) {
        if let CardKind::Graph {
            entity, entities, ..
        } = &mut self.kind
        {
            if entities.is_empty() {
                if let Some(e) = entity.take() {
                    entities.push(GraphSeries {
                        entity: e,
                        label: None,
                        color: None,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_serde_round_trip_with_size_and_color() {
        let yaml = r##"
type: entity
instance: home
entity: light.kitchen
pos: { col: 0, row: 0, w: 4, h: 2 }
color: "#ff8800"
size: large
"##;
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
        if let CardKind::Graph {
            entity,
            entities,
            window,
            ..
        } = &card.kind
        {
            assert!(
                entity.is_none(),
                "legacy entity should be cleared after normalize"
            );
            assert_eq!(entities.len(), 1);
            assert_eq!(entities[0].entity.as_str(), "sensor.temp");
            assert_eq!(window, "6h");
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn graph_multi_series_round_trip() {
        let yaml = r##"
type: graph
instance: home
graph_type: bar
orientation: horizontal
entities:
  - { entity: sensor.cpu_0 }
  - { entity: sensor.cpu_1, label: "CPU 1", color: "#ff00ff" }
pos: { col: 0, row: 0, w: 6, h: 4 }
"##;
        let card: Card = serde_yaml::from_str(yaml).unwrap();
        if let CardKind::Graph {
            entities,
            graph_type,
            orientation,
            ..
        } = &card.kind
        {
            assert_eq!(entities.len(), 2);
            assert_eq!(*graph_type, GraphType::Bar);
            assert_eq!(*orientation, BarOrientation::Horizontal);
            assert_eq!(entities[1].label.as_deref(), Some("CPU 1"));
        } else {
            panic!("wrong variant");
        }
    }

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
        if let CardKind::Gauge {
            severity, needle, ..
        } = &card.kind
        {
            assert!(severity.is_some());
            assert!(*needle);
        } else {
            panic!("wrong variant")
        }
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
        if let CardKind::Gauge {
            severity, needle, ..
        } = &card.kind
        {
            assert!(severity.is_none());
            assert!(*needle, "needle defaults to true");
        } else {
            panic!("wrong variant")
        }
    }

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
        if let CardKind::FilteredEntityList {
            hide_when_empty, ..
        } = &card.kind
        {
            assert!(*hide_when_empty);
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn clock_round_trip() {
        let yaml = r#"
type: clock
format: "%H:%M"
timezone: "Europe/London"
title: "My Clock"
pos: { col: 0, row: 0, w: 4, h: 3 }
"#;
        let card: Card = serde_yaml::from_str(yaml).unwrap();
        if let CardKind::Clock {
            format,
            timezone,
            title,
        } = &card.kind
        {
            assert_eq!(format, "%H:%M");
            assert_eq!(timezone.as_deref(), Some("Europe/London"));
            assert_eq!(title.as_deref(), Some("My Clock"));
        } else {
            panic!("wrong variant");
        }
        let back = serde_yaml::to_string(&card).unwrap();
        assert!(back.contains("type: clock"));
        assert!(back.contains("format: '%H:%M'") || back.contains("format: \"%H:%M\""));
    }

    #[test]
    fn clock_defaults() {
        let yaml = r#"
type: clock
pos: { col: 0, row: 0, w: 4, h: 3 }
"#;
        let card: Card = serde_yaml::from_str(yaml).unwrap();
        if let CardKind::Clock {
            format,
            timezone,
            title,
        } = &card.kind
        {
            assert_eq!(format, "%H:%M:%S");
            assert!(timezone.is_none());
            assert!(title.is_none());
        } else {
            panic!("wrong variant");
        }
        let back = serde_yaml::to_string(&card).unwrap();
        assert!(!back.contains("timezone:"));
        assert!(!back.contains("title:"));
    }

    #[test]
    fn statistics_round_trip() {
        let yaml = r#"
type: statistics
instance: home
entity: sensor.temperature
window: 6h
metric: avg
unit: "°C"
title: "Avg Temp"
pos: { col: 0, row: 0, w: 4, h: 3 }
"#;
        let card: Card = serde_yaml::from_str(yaml).unwrap();
        if let CardKind::Statistics {
            instance,
            entity,
            window,
            metric,
            unit,
            title,
        } = &card.kind
        {
            assert_eq!(instance.as_str(), "home");
            assert_eq!(entity.as_str(), "sensor.temperature");
            assert_eq!(window, "6h");
            assert_eq!(*metric, StatsMetric::Avg);
            assert_eq!(unit.as_deref(), Some("°C"));
            assert_eq!(title.as_deref(), Some("Avg Temp"));
        } else {
            panic!("wrong variant");
        }
        let back = serde_yaml::to_string(&card).unwrap();
        assert!(back.contains("type: statistics"));
        assert!(back.contains("metric: avg"));
    }

    #[test]
    fn media_player_round_trip() {
        let yaml = r#"
type: media_player
instance: home
entity: media_player.living_room
pos: { col: 0, row: 0, w: 6, h: 4 }
"#;
        let card: Card = serde_yaml::from_str(yaml).unwrap();
        if let CardKind::MediaPlayer { entity, .. } = &card.kind {
            assert_eq!(entity.as_str(), "media_player.living_room");
        } else {
            panic!("wrong variant");
        }
        let back = serde_yaml::to_string(&card).unwrap();
        assert!(back.contains("type: media_player"));
    }

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
        if let CardKind::Image {
            source,
            refresh_seconds,
            ..
        } = &card.kind
        {
            assert!(matches!(source, ImageSource::Camera { .. }));
            assert_eq!(*refresh_seconds, Some(30));
        } else {
            panic!("wrong variant");
        }
        let back = serde_yaml::to_string(&card).unwrap();
        assert!(back.contains("type: image"));
        assert!(back.contains("refresh_seconds: 30"));
    }

    #[test]
    fn statistics_defaults() {
        let yaml = r#"
type: statistics
instance: home
entity: sensor.temperature
metric: min
pos: { col: 0, row: 0, w: 4, h: 3 }
"#;
        let card: Card = serde_yaml::from_str(yaml).unwrap();
        if let CardKind::Statistics {
            window,
            unit,
            title,
            ..
        } = &card.kind
        {
            assert_eq!(window, "1h");
            assert!(unit.is_none());
            assert!(title.is_none());
        } else {
            panic!("wrong variant");
        }
        let back = serde_yaml::to_string(&card).unwrap();
        assert!(!back.contains("unit:"));
        assert!(!back.contains("title:"));
    }

    #[test]
    fn weather_round_trip() {
        let yaml = r#"
type: weather
instance: home
entity: weather.home
show_forecast: true
forecast_days: 5
pos: { col: 0, row: 0, w: 6, h: 4 }
"#;
        let card: Card = serde_yaml::from_str(yaml).unwrap();
        if let CardKind::Weather {
            entity,
            show_forecast,
            forecast_days,
            ..
        } = &card.kind
        {
            assert_eq!(entity.as_str(), "weather.home");
            assert!(*show_forecast);
            assert_eq!(*forecast_days, 5);
        } else {
            panic!("wrong variant")
        }
    }
}
