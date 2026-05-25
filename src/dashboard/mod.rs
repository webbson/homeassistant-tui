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
            (None | Some("free"), Some(grid), cards_opt, None) => DashboardLayout::Free {
                grid,
                cards: cards_opt.unwrap_or_default(),
            },
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
                DashboardLayout::Grid {
                    rows: rows_opt.unwrap_or_default(),
                }
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
        Ok(Dashboard {
            name: raw.name,
            layout,
        })
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

/// Navigation direction for grid 2D traversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavDir {
    Up,
    Down,
    Left,
    Right,
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

    /// Remove cards that don't satisfy `pred`. Empty columns/rows are kept in
    /// place to avoid layout surprises.
    pub fn retain_cards(&mut self, mut pred: impl FnMut(&Card) -> bool) {
        match &mut self.layout {
            DashboardLayout::Free { cards, .. } => cards.retain(|c| pred(c)),
            DashboardLayout::Grid { rows } => {
                for row in rows.iter_mut() {
                    for col in row.columns.iter_mut() {
                        col.cards.retain(|c| pred(c));
                    }
                }
            }
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

    #[allow(dead_code)]
    pub fn card_by_id(&self, id: CardId) -> Option<&Card> {
        self.cards_iter().find(|c| c.id == id)
    }

    #[allow(dead_code)]
    pub fn card_by_id_mut(&mut self, id: CardId) -> Option<&mut Card> {
        self.cards_iter_mut().find(|c| c.id == id)
    }

    /// Resolve a `CardId` to its flat index.
    pub fn flat_idx_of(&self, id: CardId) -> Option<usize> {
        self.cards_iter().position(|c| c.id == id)
    }

    /// Reverse of `flat_idx_from_grid`: flat index → (row, col, pos_in_col).
    pub fn locate_grid_flat(&self, flat_idx: usize) -> Option<(usize, usize, usize)> {
        let DashboardLayout::Grid { rows } = &self.layout else {
            return None;
        };
        let mut f = 0usize;
        for (ri, row) in rows.iter().enumerate() {
            for (ci, col) in row.columns.iter().enumerate() {
                if flat_idx >= f && flat_idx < f + col.cards.len() {
                    return Some((ri, ci, flat_idx - f));
                }
                f += col.cards.len();
            }
        }
        None
    }

    /// Compute the flat index for a (row, col, pos_in_col) triple in a grid layout.
    pub fn flat_idx_from_grid(
        &self,
        row_idx: usize,
        col_idx: usize,
        pos_in_col: usize,
    ) -> Option<usize> {
        let DashboardLayout::Grid { rows } = &self.layout else {
            return None;
        };
        let mut f = 0usize;
        for (ri, row) in rows.iter().enumerate() {
            for (ci, col) in row.columns.iter().enumerate() {
                if ri == row_idx && ci == col_idx {
                    if pos_in_col < col.cards.len() {
                        return Some(f + pos_in_col);
                    } else {
                        return None;
                    }
                }
                f += col.cards.len();
            }
        }
        None
    }

    #[allow(dead_code)]
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

    #[allow(dead_code)]
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
    #[allow(dead_code)]
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

    /// Remove a card by flat index regardless of layout. Returns the removed card.
    pub fn remove_card_at(&mut self, flat_idx: usize) -> Option<Card> {
        match &mut self.layout {
            DashboardLayout::Free { cards, .. } => {
                if flat_idx < cards.len() {
                    Some(cards.remove(flat_idx))
                } else {
                    None
                }
            }
            DashboardLayout::Grid { rows } => {
                let mut f = 0usize;
                for row in rows.iter_mut() {
                    for col in row.columns.iter_mut() {
                        if flat_idx >= f && flat_idx < f + col.cards.len() {
                            return Some(col.cards.remove(flat_idx - f));
                        }
                        f += col.cards.len();
                    }
                }
                None
            }
        }
    }

    /// Append a new row to a grid-layout dashboard.
    pub fn grid_add_row(&mut self, height: RowHeight, n_cols: usize) {
        let DashboardLayout::Grid { rows } = &mut self.layout else {
            return;
        };
        rows.push(GridRow {
            height,
            fill_height: None,
            columns: (0..n_cols.max(1))
                .map(|_| GridColumn {
                    fill_height: None,
                    cards: vec![],
                })
                .collect(),
        });
    }

    /// Remove a row by index. Returns false if index is out of range or last row.
    pub fn grid_remove_row(&mut self, row_idx: usize) -> bool {
        let DashboardLayout::Grid { rows } = &mut self.layout else {
            return false;
        };
        if rows.len() <= 1 || row_idx >= rows.len() {
            return false;
        }
        rows.remove(row_idx);
        true
    }

    /// Swap two adjacent rows (move row_idx up or down).
    pub fn grid_move_row(&mut self, row_idx: usize, up: bool) -> bool {
        let DashboardLayout::Grid { rows } = &mut self.layout else {
            return false;
        };
        let other = if up {
            row_idx.checked_sub(1)
        } else {
            row_idx.checked_add(1)
        };
        let Some(other) = other else {
            return false;
        };
        if other >= rows.len() {
            return false;
        }
        rows.swap(row_idx, other);
        true
    }

    /// Add a column to a row.
    pub fn grid_add_column(&mut self, row_idx: usize) -> bool {
        let DashboardLayout::Grid { rows } = &mut self.layout else {
            return false;
        };
        let Some(row) = rows.get_mut(row_idx) else {
            return false;
        };
        row.columns.push(GridColumn {
            fill_height: None,
            cards: vec![],
        });
        true
    }

    /// Remove a column (and its cards) from a row.
    pub fn grid_remove_column(&mut self, row_idx: usize, col_idx: usize) -> bool {
        let DashboardLayout::Grid { rows } = &mut self.layout else {
            return false;
        };
        let Some(row) = rows.get_mut(row_idx) else {
            return false;
        };
        if row.columns.len() <= 1 || col_idx >= row.columns.len() {
            return false;
        }
        row.columns.remove(col_idx);
        true
    }

    /// Swap two adjacent columns within a row.
    pub fn grid_move_column(&mut self, row_idx: usize, col_idx: usize, left: bool) -> bool {
        let DashboardLayout::Grid { rows } = &mut self.layout else {
            return false;
        };
        let Some(row) = rows.get_mut(row_idx) else {
            return false;
        };
        let other = if left {
            col_idx.checked_sub(1)
        } else {
            col_idx.checked_add(1)
        };
        let Some(other) = other else {
            return false;
        };
        if other >= row.columns.len() {
            return false;
        }
        row.columns.swap(col_idx, other);
        true
    }

    /// Move a card up or down within its column.
    pub fn grid_move_card_in_column(
        &mut self,
        row_idx: usize,
        col_idx: usize,
        pos: usize,
        up: bool,
    ) -> bool {
        let DashboardLayout::Grid { rows } = &mut self.layout else {
            return false;
        };
        let Some(row) = rows.get_mut(row_idx) else {
            return false;
        };
        let Some(col) = row.columns.get_mut(col_idx) else {
            return false;
        };
        let other = if up {
            pos.checked_sub(1)
        } else {
            pos.checked_add(1)
        };
        let Some(other) = other else {
            return false;
        };
        if other >= col.cards.len() {
            return false;
        }
        col.cards.swap(pos, other);
        true
    }

    /// Set the height of a grid row.
    pub fn grid_set_row_height(&mut self, row_idx: usize, height: RowHeight) -> bool {
        let DashboardLayout::Grid { rows } = &mut self.layout else {
            return false;
        };
        let Some(row) = rows.get_mut(row_idx) else {
            return false;
        };
        row.height = height;
        true
    }

    /// Toggle the `fill_height` default for a row.
    pub fn grid_toggle_row_fill_height(&mut self, row_idx: usize) -> bool {
        let DashboardLayout::Grid { rows } = &mut self.layout else {
            return false;
        };
        let Some(row) = rows.get_mut(row_idx) else {
            return false;
        };
        let current = row.fill_height.unwrap_or(false);
        row.fill_height = Some(!current);
        true
    }

    /// Toggle `fill_height` for a specific column.
    pub fn grid_toggle_column_fill_height(&mut self, row_idx: usize, col_idx: usize) -> bool {
        let DashboardLayout::Grid { rows } = &mut self.layout else {
            return false;
        };
        let Some(row) = rows.get_mut(row_idx) else {
            return false;
        };
        let Some(col) = row.columns.get_mut(col_idx) else {
            return false;
        };
        let current = col.fill_height.unwrap_or(false);
        col.fill_height = Some(!current);
        true
    }

    /// Return the flat index of the neighboring card in a grid layout.
    /// Returns `None` if the selection is already at the boundary or the layout is Free.
    pub fn neighbor(&self, flat_idx: usize, dir: NavDir) -> Option<usize> {
        let DashboardLayout::Grid { rows } = &self.layout else {
            return None;
        };

        // Locate (ri, ci, pos_in_col, col_flat_start) for flat_idx.
        let mut f = 0usize;
        let mut ri0 = 0;
        let mut ci0 = 0;
        let mut pos0 = 0;
        let mut col_start0 = 0;
        let mut found = false;
        'outer: for (ri, row) in rows.iter().enumerate() {
            for (ci, col) in row.columns.iter().enumerate() {
                if flat_idx >= f && flat_idx < f + col.cards.len() {
                    ri0 = ri;
                    ci0 = ci;
                    pos0 = flat_idx - f;
                    col_start0 = f;
                    found = true;
                    break 'outer;
                }
                f += col.cards.len();
            }
        }
        if !found {
            return None;
        }

        let col_flat_start = |target_ri: usize, target_ci: usize| -> Option<usize> {
            let mut s = 0usize;
            for (i, row) in rows.iter().enumerate() {
                if i == target_ri {
                    for (j, col) in row.columns.iter().enumerate() {
                        if j == target_ci {
                            return Some(s);
                        }
                        s += col.cards.len();
                    }
                    return None;
                }
                for col in &row.columns {
                    s += col.cards.len();
                }
            }
            None
        };

        match dir {
            NavDir::Up => {
                if pos0 == 0 {
                    None
                } else {
                    Some(col_start0 + pos0 - 1)
                }
            }
            NavDir::Down => {
                let col_len = rows[ri0].columns[ci0].cards.len();
                if pos0 + 1 >= col_len {
                    None
                } else {
                    Some(col_start0 + pos0 + 1)
                }
            }
            NavDir::Left => {
                let (tri, tci) = if ci0 > 0 {
                    (ri0, ci0 - 1)
                } else if ri0 > 0 {
                    let prev_ri = ri0 - 1;
                    let last_ci = rows[prev_ri].columns.len().checked_sub(1)?;
                    (prev_ri, last_ci)
                } else {
                    return None;
                };
                let start = col_flat_start(tri, tci)?;
                let col_len = rows[tri].columns[tci].cards.len();
                if col_len == 0 {
                    return None;
                }
                Some(start + pos0.min(col_len - 1))
            }
            NavDir::Right => {
                let row_cols = rows[ri0].columns.len();
                let (tri, tci) = if ci0 + 1 < row_cols {
                    (ri0, ci0 + 1)
                } else if ri0 + 1 < rows.len() {
                    (ri0 + 1, 0)
                } else {
                    return None;
                };
                let start = col_flat_start(tri, tci)?;
                let col_len = rows[tri].columns[tci].cards.len();
                if col_len == 0 {
                    return None;
                }
                Some(start + pos0.min(col_len - 1))
            }
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
                    Err(E::custom(format!(
                        "expected \"auto\" or integer, got {:?}",
                        v
                    )))
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

/// A single entry in an `EntityList` card — either a bare entity ID string or
/// a full object with optional per-entry display overrides.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum EntityListItem {
    Bare(EntityId),
    Full {
        entity: EntityId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hide_state: Option<bool>,
    },
}

impl EntityListItem {
    pub fn entity_id(&self) -> &EntityId {
        match self {
            Self::Bare(e) | Self::Full { entity: e, .. } => e,
        }
    }

    pub fn name_override(&self) -> Option<&str> {
        if let Self::Full { name: Some(n), .. } = self {
            Some(n.as_str())
        } else {
            None
        }
    }

    pub fn hide_state_override(&self) -> Option<bool> {
        if let Self::Full { hide_state, .. } = self {
            *hide_state
        } else {
            None
        }
    }
}

/// Per-entity display overrides used by `FilteredEntityList`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct EntityOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hide_state: Option<bool>,
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
        entities: Vec<EntityListItem>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
        overrides: std::collections::BTreeMap<EntityId, EntityOverride>,
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
    AttributeList {
        instance: Alias,
        entity: EntityId,
        /// Top-level attribute key (e.g. `top10`).
        attribute: String,
        /// Row format template. Supports `{key}` and `{key|filter}` tokens.
        template: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<usize>,
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
            CardKind::AttributeList { title, entity, .. } => {
                title.as_deref().unwrap_or(entity.as_str())
            }
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
            CardKind::AttributeList {
                instance, entity, ..
            } => Some((instance, entity)),
            CardKind::Text { .. }
            | CardKind::EntityList { .. }
            | CardKind::FilteredEntityList { .. }
            | CardKind::Clock { .. } => None,
        }
    }

    /// Return a mutable reference to the instance alias for card kinds that
    /// have one (all except Text and Clock).
    pub fn instance_mut(&mut self) -> Option<&mut Alias> {
        match &mut self.kind {
            CardKind::Entity { instance, .. }
            | CardKind::Toggle { instance, .. }
            | CardKind::Gauge { instance, .. }
            | CardKind::Graph { instance, .. }
            | CardKind::EntityList { instance, .. }
            | CardKind::FilteredEntityList { instance, .. }
            | CardKind::Statistics { instance, .. }
            | CardKind::MediaPlayer { instance, .. }
            | CardKind::Image { instance, .. }
            | CardKind::Weather { instance, .. }
            | CardKind::AttributeList { instance, .. } => Some(instance),
            CardKind::Text { .. } | CardKind::Clock { .. } => None,
        }
    }

    /// Return the instance alias for card kinds that have one.
    pub fn instance_ref(&self) -> Option<&Alias> {
        match &self.kind {
            CardKind::Entity { instance, .. }
            | CardKind::Toggle { instance, .. }
            | CardKind::Gauge { instance, .. }
            | CardKind::Graph { instance, .. }
            | CardKind::EntityList { instance, .. }
            | CardKind::FilteredEntityList { instance, .. }
            | CardKind::Statistics { instance, .. }
            | CardKind::MediaPlayer { instance, .. }
            | CardKind::Image { instance, .. }
            | CardKind::Weather { instance, .. }
            | CardKind::AttributeList { instance, .. } => Some(instance),
            CardKind::Text { .. } | CardKind::Clock { .. } => None,
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

    /// Compute the preferred terminal-row height for a grid-layout column.
    /// `available_width` is the column width in terminal columns (for text-wrap).
    /// `dynamic_count` is the resolved entity count for FilteredEntityList / AttributeList.
    /// `in_editor` keeps FilteredEntityList with `hide_when_empty` visible as a placeholder.
    ///
    /// If `self.height` is set it always wins. Otherwise falls back to per-kind static defaults,
    /// scaled to honor `CardSize::Large` (needs room for big-text glyphs) and `CardSize::Small`.
    pub fn preferred_height(
        &self,
        available_width: u16,
        dynamic_count: Option<usize>,
        in_editor: bool,
    ) -> u16 {
        if let Some(h) = self.height {
            return h;
        }
        let inner_w = (available_width.saturating_sub(2)).max(1) as usize;
        // Border (2) + big-text glyph (4) — minimum so `big_text::fits` succeeds.
        const LARGE_MIN: u16 = 6;
        let base = match &self.kind {
            CardKind::Entity { .. } | CardKind::Toggle { .. } => 3,
            CardKind::Gauge { .. } => 5,
            CardKind::Clock { .. } => 3,
            CardKind::Statistics { .. } => 4,
            CardKind::MediaPlayer { .. } => 6,
            CardKind::Weather { show_forecast, .. } => {
                if *show_forecast {
                    12
                } else {
                    6
                }
            }
            CardKind::Image { .. } => 10,
            CardKind::Graph { .. } => 10,
            CardKind::Text { markdown, .. } => {
                let lines: u16 = markdown
                    .split('\n')
                    .map(|l| {
                        let chars = l.chars().count().max(1);
                        ((chars + inner_w - 1) / inner_w) as u16
                    })
                    .sum();
                lines.max(1) + 2
            }
            CardKind::EntityList { entities, .. } => (entities.len() as u16).saturating_add(2),
            CardKind::FilteredEntityList {
                hide_when_empty, ..
            } => {
                if *hide_when_empty && dynamic_count == Some(0) && !in_editor {
                    return 0;
                }
                let count = dynamic_count.unwrap_or(4);
                (count as u16).saturating_add(2).max(4)
            }
            CardKind::AttributeList { limit, .. } => match dynamic_count {
                Some(n) => (n as u16).saturating_add(2),
                None => limit.map(|n| (n as u16).saturating_add(2)).unwrap_or(6),
            },
        };
        match self.size {
            CardSize::Large => base.max(LARGE_MIN),
            CardSize::Small => match &self.kind {
                CardKind::Weather { .. } | CardKind::MediaPlayer { .. } => 3,
                _ => base,
            },
            CardSize::Normal => base,
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

    fn make_grid_dashboard(col_card_counts: &[&[usize]]) -> Dashboard {
        // col_card_counts: one slice per row, each element = number of cards in that column.
        let mut next_id = 1u64;
        let rows: Vec<GridRow> = col_card_counts
            .iter()
            .map(|cols| GridRow {
                height: RowHeight::Fixed(10),
                fill_height: None,
                columns: cols
                    .iter()
                    .map(|&n| GridColumn {
                        fill_height: None,
                        cards: (0..n)
                            .map(|_| {
                                let id = CardId(next_id);
                                next_id += 1;
                                Card {
                                    id,
                                    kind: CardKind::Text {
                                        markdown: String::new(),
                                        title: None,
                                    },
                                    pos: None,
                                    height: None,
                                    color: None,
                                    size: CardSize::Normal,
                                }
                            })
                            .collect(),
                    })
                    .collect(),
            })
            .collect();
        Dashboard {
            name: "test".into(),
            layout: DashboardLayout::Grid { rows },
        }
    }

    #[test]
    fn neighbor_up_down_within_column() {
        // 1 row, 1 col, 3 cards (flat 0/1/2)
        let dash = make_grid_dashboard(&[&[3]]);
        assert_eq!(dash.neighbor(0, NavDir::Up), None, "top of col stays");
        assert_eq!(dash.neighbor(0, NavDir::Down), Some(1));
        assert_eq!(dash.neighbor(1, NavDir::Up), Some(0));
        assert_eq!(dash.neighbor(2, NavDir::Down), None, "bottom of col stays");
    }

    #[test]
    fn neighbor_left_right_between_columns() {
        // 1 row, 3 cols of 2 cards each: flat 0-1 / 2-3 / 4-5
        let dash = make_grid_dashboard(&[&[2, 2, 2]]);
        assert_eq!(dash.neighbor(0, NavDir::Left), None, "leftmost col stays");
        assert_eq!(
            dash.neighbor(0, NavDir::Right),
            Some(2),
            "go to col 1 pos 0"
        );
        assert_eq!(
            dash.neighbor(3, NavDir::Left),
            Some(1),
            "col 1 pos 1 → col 0 pos 1"
        );
        // col 2 flat 4-5; neighbor Right from last col → None
        assert_eq!(dash.neighbor(4, NavDir::Right), None);
    }

    #[test]
    fn neighbor_wraps_row_boundary() {
        // 2 rows: row0 has 1 col of 2 cards; row1 has 1 col of 2 cards
        // flat: 0-1 (row0 col0), 2-3 (row1 col0)
        let dash = make_grid_dashboard(&[&[2], &[2]]);
        // Left from row0 col0 → None (first row)
        assert_eq!(dash.neighbor(0, NavDir::Left), None);
        // Right from row0 col0 → row1 col0 pos0 = flat 2
        assert_eq!(dash.neighbor(0, NavDir::Right), Some(2));
        // Left from row1 col0 → row0 col0 pos0 = flat 0
        assert_eq!(dash.neighbor(2, NavDir::Left), Some(0));
        // Right from row1 col0 → None (last row)
        assert_eq!(dash.neighbor(2, NavDir::Right), None);
    }

    #[test]
    fn flat_idx_roundtrip() {
        // 2 rows: row0 has 2 cols (1 card each), row1 has 1 col (2 cards)
        // flat order: 0 (r0c0p0), 1 (r0c1p0), 2 (r1c0p0), 3 (r1c0p1)
        let dash = make_grid_dashboard(&[&[1, 1], &[2]]);
        assert_eq!(dash.flat_idx_from_grid(0, 0, 0), Some(0));
        assert_eq!(dash.flat_idx_from_grid(0, 1, 0), Some(1));
        assert_eq!(dash.flat_idx_from_grid(1, 0, 0), Some(2));
        assert_eq!(dash.flat_idx_from_grid(1, 0, 1), Some(3));
        assert_eq!(dash.flat_idx_from_grid(1, 0, 2), None); // out of range

        assert_eq!(dash.locate_grid_flat(0), Some((0, 0, 0)));
        assert_eq!(dash.locate_grid_flat(1), Some((0, 1, 0)));
        assert_eq!(dash.locate_grid_flat(2), Some((1, 0, 0)));
        assert_eq!(dash.locate_grid_flat(3), Some((1, 0, 1)));
        assert_eq!(dash.locate_grid_flat(4), None);
    }

    #[test]
    fn grid_structural_ops() {
        let mut dash = make_grid_dashboard(&[&[2, 1]]);
        // Save the id of flat 0 to verify selection stability.
        let id0 = dash.cards_iter().next().unwrap().id;

        // Move row down: still only 1 row, should be a no-op.
        assert!(!dash.grid_move_row(0, false));

        // Add a second row.
        dash.grid_add_row(RowHeight::Auto, 2);
        if let DashboardLayout::Grid { rows } = &dash.layout {
            assert_eq!(rows.len(), 2);
        }

        // Add column to row 0.
        dash.grid_add_column(0);
        if let DashboardLayout::Grid { rows } = &dash.layout {
            assert_eq!(rows[0].columns.len(), 3);
        }

        // Remove column from row 0 (must leave ≥1 column).
        assert!(dash.grid_remove_column(0, 2));
        if let DashboardLayout::Grid { rows } = &dash.layout {
            assert_eq!(rows[0].columns.len(), 2);
        }

        // Move card in column: card 0 and 1 are both in col 0 of row 0.
        // Swap them (move card at pos 0 down).
        let before_id = dash.flat_idx_of(id0).unwrap();
        assert_eq!(before_id, 0);
        dash.grid_move_card_in_column(0, 0, 0, false); // move pos-0 down → pos-1
                                                       // id0 should now be at flat index 1.
        assert_eq!(dash.flat_idx_of(id0), Some(1));
    }

    // ---- EntityListItem / EntityOverride YAML round-trip tests ----

    #[test]
    fn entity_list_bare_entries_round_trip() {
        let yaml = r#"
type: entity_list
instance: home
entities:
  - light.kitchen
  - light.bedroom
pos: { col: 0, row: 0, w: 4, h: 4 }
"#;
        let card: Card = serde_yaml::from_str(yaml).unwrap();
        let CardKind::EntityList { entities, .. } = &card.kind else {
            panic!("expected EntityList");
        };
        assert_eq!(entities.len(), 2);
        assert!(matches!(&entities[0], EntityListItem::Bare(e) if e == "light.kitchen"));
        assert!(matches!(&entities[1], EntityListItem::Bare(e) if e == "light.bedroom"));
        // Verify bare form is preserved on serialization (not expanded to {entity: ...}).
        let back = serde_yaml::to_string(&card).unwrap();
        assert!(
            back.contains("- light.kitchen"),
            "bare string form not preserved: {back}"
        );
        assert!(
            back.contains("- light.bedroom"),
            "bare string form not preserved: {back}"
        );
        assert!(
            !back.contains("entity: light.kitchen"),
            "bare was expanded: {back}"
        );
    }

    #[test]
    fn entity_list_full_entry_with_name_round_trip() {
        let yaml = r#"
type: entity_list
instance: home
entities:
  - entity: light.kitchen
    name: Kitchen
pos: { col: 0, row: 0, w: 4, h: 4 }
"#;
        let card: Card = serde_yaml::from_str(yaml).unwrap();
        let CardKind::EntityList { entities, .. } = &card.kind else {
            panic!("expected EntityList");
        };
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].entity_id(), "light.kitchen");
        assert_eq!(entities[0].name_override(), Some("Kitchen"));
        assert_eq!(entities[0].hide_state_override(), None);
        let back = serde_yaml::to_string(&card).unwrap();
        assert!(back.contains("entity: light.kitchen"));
        assert!(back.contains("name: Kitchen"));
    }

    #[test]
    fn entity_list_full_entry_hide_state_round_trip() {
        let yaml = r#"
type: entity_list
instance: home
entities:
  - entity: light.bedroom
    hide_state: true
pos: { col: 0, row: 0, w: 4, h: 4 }
"#;
        let card: Card = serde_yaml::from_str(yaml).unwrap();
        let CardKind::EntityList { entities, .. } = &card.kind else {
            panic!("expected EntityList");
        };
        assert_eq!(entities[0].hide_state_override(), Some(true));
        let back = serde_yaml::to_string(&card).unwrap();
        assert!(back.contains("hide_state: true"));
    }

    #[test]
    fn entity_list_mixed_bare_and_full_round_trip() {
        let yaml = r#"
type: entity_list
instance: home
entities:
  - light.kitchen
  - entity: light.bedroom
    name: Bedroom
pos: { col: 0, row: 0, w: 4, h: 4 }
"#;
        let card: Card = serde_yaml::from_str(yaml).unwrap();
        let CardKind::EntityList { entities, .. } = &card.kind else {
            panic!("expected EntityList");
        };
        assert_eq!(entities.len(), 2);
        assert!(matches!(&entities[0], EntityListItem::Bare(_)));
        assert!(matches!(
            &entities[1],
            EntityListItem::Full { name: Some(_), .. }
        ));
        let back = serde_yaml::to_string(&card).unwrap();
        assert!(back.contains("- light.kitchen"));
        assert!(back.contains("name: Bedroom"));
    }

    #[test]
    fn filtered_entity_list_overrides_round_trip() {
        let yaml = r#"
type: filtered_entity_list
instance: home
query: "light.*"
hide_state: false
overrides:
  light.kitchen:
    name: Kitchen
    hide_state: true
pos: { col: 0, row: 0, w: 4, h: 4 }
"#;
        let card: Card = serde_yaml::from_str(yaml).unwrap();
        let CardKind::FilteredEntityList { overrides, .. } = &card.kind else {
            panic!("expected FilteredEntityList");
        };
        let ov = overrides.get("light.kitchen").expect("override missing");
        assert_eq!(ov.name.as_deref(), Some("Kitchen"));
        assert_eq!(ov.hide_state, Some(true));
        let back = serde_yaml::to_string(&card).unwrap();
        assert!(back.contains("overrides:"));
        assert!(back.contains("light.kitchen:"));
    }

    #[test]
    fn filtered_entity_list_empty_overrides_omitted() {
        let yaml = r#"
type: filtered_entity_list
instance: home
query: "light.*"
hide_state: false
pos: { col: 0, row: 0, w: 4, h: 4 }
"#;
        let card: Card = serde_yaml::from_str(yaml).unwrap();
        let CardKind::FilteredEntityList { overrides, .. } = &card.kind else {
            panic!("expected FilteredEntityList");
        };
        assert!(overrides.is_empty());
        let back = serde_yaml::to_string(&card).unwrap();
        // Empty overrides map must not appear in serialized output.
        assert!(
            !back.contains("overrides:"),
            "empty overrides should be omitted: {back}"
        );
    }

    // Required canonical test names -----------------------------------------------

    #[test]
    fn entity_list_item_bare_round_trips() {
        let yaml = r#"
type: entity_list
instance: home
entities:
  - light.kitchen
pos: { col: 0, row: 0, w: 4, h: 4 }
"#;
        let card: Card = serde_yaml::from_str(yaml).unwrap();
        let CardKind::EntityList { entities, .. } = &card.kind else {
            panic!("expected EntityList");
        };
        assert!(matches!(&entities[0], EntityListItem::Bare(e) if e == "light.kitchen"));
        let back = serde_yaml::to_string(&card).unwrap();
        assert!(
            back.contains("- light.kitchen"),
            "bare string not preserved: {back}"
        );
        assert!(
            !back.contains("entity: light.kitchen"),
            "bare was expanded: {back}"
        );
    }

    #[test]
    fn entity_list_item_full_round_trips() {
        let yaml = r#"
type: entity_list
instance: home
entities:
  - entity: light.kitchen
    name: Kitchen
    hide_state: true
pos: { col: 0, row: 0, w: 4, h: 4 }
"#;
        let card: Card = serde_yaml::from_str(yaml).unwrap();
        let CardKind::EntityList { entities, .. } = &card.kind else {
            panic!("expected EntityList");
        };
        assert_eq!(entities[0].entity_id(), "light.kitchen");
        assert_eq!(entities[0].name_override(), Some("Kitchen"));
        assert_eq!(entities[0].hide_state_override(), Some(true));
        let back = serde_yaml::to_string(&card).unwrap();
        assert!(back.contains("entity: light.kitchen"));
        assert!(back.contains("name: Kitchen"));
        assert!(back.contains("hide_state: true"));
    }

    #[test]
    fn entity_list_item_mixed_forms() {
        let yaml = r#"
type: entity_list
instance: home
entities:
  - light.kitchen
  - entity: light.bedroom
    name: Bedroom
pos: { col: 0, row: 0, w: 4, h: 4 }
"#;
        let card: Card = serde_yaml::from_str(yaml).unwrap();
        let CardKind::EntityList { entities, .. } = &card.kind else {
            panic!("expected EntityList");
        };
        assert_eq!(entities.len(), 2);
        assert!(matches!(&entities[0], EntityListItem::Bare(_)));
        assert!(matches!(
            &entities[1],
            EntityListItem::Full { name: Some(_), .. }
        ));
        let back = serde_yaml::to_string(&card).unwrap();
        assert!(back.contains("- light.kitchen"));
        assert!(back.contains("name: Bedroom"));
    }

    #[test]
    fn filtered_entity_list_overrides_default_empty() {
        let yaml = r#"
type: filtered_entity_list
instance: home
query: "sensor.*"
hide_state: false
pos: { col: 0, row: 0, w: 4, h: 4 }
"#;
        let card: Card = serde_yaml::from_str(yaml).unwrap();
        let CardKind::FilteredEntityList { overrides, .. } = &card.kind else {
            panic!("expected FilteredEntityList");
        };
        assert!(overrides.is_empty(), "overrides should default to empty");
    }

    #[test]
    fn filtered_entity_list_overrides_omitted_when_empty() {
        let yaml = r#"
type: filtered_entity_list
instance: home
query: "sensor.*"
hide_state: false
pos: { col: 0, row: 0, w: 4, h: 4 }
"#;
        let card: Card = serde_yaml::from_str(yaml).unwrap();
        let back = serde_yaml::to_string(&card).unwrap();
        assert!(
            !back.contains("overrides:"),
            "empty overrides should be omitted: {back}"
        );
    }

    #[test]
    fn selection_preserved_through_row_move() {
        let mut dash = make_grid_dashboard(&[&[1], &[1]]);
        // flat 0 = row0, flat 1 = row1
        let id_row0 = dash.cards_iter().next().unwrap().id;

        // Move row 0 down: row0 → row1 position.
        dash.grid_move_row(0, false);
        // Card that was at flat 0 should now be at flat 1.
        assert_eq!(dash.flat_idx_of(id_row0), Some(1));
    }
}
