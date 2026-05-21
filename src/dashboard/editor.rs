use crate::dashboard::{
    BarOrientation, Card, CardKind, CardSize, Dashboard, DashboardLayout, GraphSeries,
    GraphType, Pos, RowHeight, StatsMetric,
};

const MAX_UNDO: usize = 32;

#[derive(Debug, Clone)]
pub struct EditorState {
    pub dash_idx: usize,
    pub cursor_col: u16,
    pub cursor_row: u16,
    pub selected_card: Option<usize>,
    pub mode: EditorMode,
    pub undo_stack: Vec<Dashboard>,
    pub dirty: bool,
    pub source_path: Option<std::path::PathBuf>,
    /// When `Some(idx)`, the in-progress picker/title flow replaces card[idx]
    /// instead of appending a new card.
    pub edit_target: Option<usize>,
    /// During the Image add-flow, set after `ImagePickSourceKind` so the
    /// downstream entity picker can filter to `image.*` or `camera.*` only.
    pub image_pending_is_camera: Option<bool>,
    /// Focus position within a grid-layout dashboard. `None` for Free dashboards.
    pub grid_focus: Option<GridFocus>,
}

#[derive(Debug, Clone)]
pub enum EditorMode {
    Browse,
    /// User pressed `a` — picking a card type.
    PickingType {
        selected: usize,
    },
    /// Card type chosen, multi-instance — pick which HA instance.
    PickingInstance {
        card_type: CardTypeStub,
        selected: usize,
    },
    /// Instance picked — fuzzy-search for entity within that instance.
    PickingEntity {
        card_type: CardTypeStub,
        instance: String,
        query: String,
        selected: usize,
    },
    /// Entity picked — optional title override (Enter on empty = use friendly_name).
    EditingTitle {
        card_type: CardTypeStub,
        instance: String,
        entity: String,
        friendly_name: String,
        title_buffer: String,
    },
    /// Text card body input.
    EditingTextBody {
        title_buffer: String,
        body_buffer: String,
        focus_body: bool,
    },
    /// Multi-select entity picker (used only for EntityList card type).
    PickingMulti {
        instance: String,
        query: String,
        selected: usize,
        picked: Vec<(String, String)>,
    },
    /// Title input for the in-progress EntityList card.
    EditingEntityListTitle {
        instance: String,
        picked: Vec<(String, String)>,
        title_buffer: String,
    },
    /// Edit per-entry display overrides (name, hide_state) for a specific row in
    /// an EntityList or FilteredEntityList card.
    EditEntityListItemOverride {
        card_idx: usize,
        item_idx: usize,
        /// `Some(entity_id)` for FilteredEntityList rows (keyed by id in the overrides map);
        /// `None` for EntityList rows (stored directly on the item).
        entity_id: Option<String>,
        name_buf: String,
        hide_state: bool,
        /// Only `true` for the FilteredEntityList path while the user is still
        /// typing the entity_id (before they press Tab or Enter to move to name).
        focus_entity_id: bool,
    },
    /// Rename current dashboard.
    Renaming {
        buffer: String,
    },
    /// Adjust grid dimensions.
    ResizingGrid {
        cols_buffer: String,
        rows_buffer: String,
        focus_rows: bool,
    },
    /// Confirm dialog before exiting with unsaved changes.
    ConfirmExit,
    /// Confirm dialog before deleting a card.
    ConfirmDelete,
    /// Rename the title of the selected card.
    RenamingCard {
        card_idx: usize,
        buffer: String,
    },
    /// Edit the sparkline window (e.g. "1h", "24h", "7d") for the selected card.
    EditingWindow {
        card_idx: usize,
        buffer: String,
    },
    /// Compose / edit a FilteredEntityList query + title + hide_state.
    EditingFilterQuery {
        instance: String,
        query_buffer: String,
        title_buffer: String,
        hide_state: bool,
        focus: FilterFocus,
    },
    /// Contextual settings menu (card- or dashboard-scoped).
    Menu {
        context: MenuContext,
        items: Vec<MenuItem>,
        selected: usize,
    },
    /// Enter a named color or #rrggbb for the selected card.
    EnterColorOverride {
        card_idx: usize,
        buf: String,
    },
    /// Pick Small / Normal / Large for the selected card.
    PickCardSize {
        card_idx: usize,
        current: CardSize,
    },
    // ---- Graph add-flow ----
    /// Step 1: pick Line / Bar / Pie.
    GraphPickType,
    /// Step 2: pick HA instance.
    GraphPickInstance {
        graph_type: GraphType,
        selected: usize,
    },
    /// Step 3: pick entities one by one; loop until user says "done".
    GraphAddEntities {
        instance: String,
        graph_type: GraphType,
        accumulated: Vec<GraphSeries>,
        query: String,
        selected: usize,
        /// true while we're showing the "add another? (y/n)" prompt
        asking_more: bool,
    },
    /// Step 4a (Line): enter window string.
    GraphEditWindowAdd {
        instance: String,
        graph_type: GraphType,
        series: Vec<GraphSeries>,
        window_buf: String,
        title_buf: String,
        title_stage: bool,
    },
    /// Step 4b (Bar): pick orientation.
    GraphPickOrientationAdd {
        instance: String,
        series: Vec<GraphSeries>,
        current: BarOrientation,
        title_buf: String,
        title_stage: bool,
    },
    // ---- Graph context-menu flows ----
    /// Add one series to an existing Graph card.
    GraphAddOneSeries {
        card_idx: usize,
        query: String,
        selected: usize,
    },
    /// Pick which series to operate on.
    GraphPickSeriesIndex {
        card_idx: usize,
        op: SeriesIndexOp,
        selected: usize,
    },
    /// Edit the color of one series.
    GraphEditSeriesColor {
        card_idx: usize,
        series_idx: usize,
        buf: String,
    },
    /// Edit the label of one series.
    GraphEditSeriesLabel {
        card_idx: usize,
        series_idx: usize,
        buf: String,
    },
    /// Edit window on an existing Graph card (from menu).
    GraphEditWindow {
        card_idx: usize,
        buf: String,
    },
    /// Pick orientation on an existing Bar Graph card (from menu).
    GraphPickOrientation {
        card_idx: usize,
        current: BarOrientation,
    },
    // ---- Gauge severity flow ----
    /// Step 1 of 3: enter the "green" lower threshold.
    EditSeverityGreen {
        card_idx: usize,
        buf: String,
        accum: SeverityAccum,
    },
    /// Step 2 of 3: enter the "yellow" warning threshold.
    EditSeverityYellow {
        card_idx: usize,
        buf: String,
        accum: SeverityAccum,
    },
    /// Step 3 of 3: enter the "red" critical threshold.
    EditSeverityRed {
        card_idx: usize,
        buf: String,
        accum: SeverityAccum,
    },
    // ---- Clock add-flow ----
    /// Step 1: optional title.
    ClockAddTitle {
        title_buffer: String,
    },
    /// Step 2: strftime format string.
    ClockAddFormat {
        title: Option<String>,
        format_buffer: String,
    },
    /// Step 3: optional IANA timezone (empty = local).
    ClockAddTimezone {
        title: Option<String>,
        format: String,
        tz_buffer: String,
    },
    // ---- Clock context-menu flows ----
    /// Edit the strftime format of an existing Clock card.
    ClockEditFormat {
        card_idx: usize,
        buf: String,
    },
    /// Edit the timezone of an existing Clock card.
    ClockEditTimezone {
        card_idx: usize,
        buf: String,
    },
    // ---- Statistics add-flow ----
    /// Step 3 (after entity pick): choose Avg/Min/Max/Sum/Count.
    StatsPickMetric {
        instance: String,
        entity: String,
        friendly_name: String,
        selected: usize,
    },
    /// Step 4: enter window string (default "1h").
    StatsEditWindowAdd {
        instance: String,
        entity: String,
        friendly_name: String,
        metric: StatsMetric,
        buf: String,
    },
    /// Step 5: optional unit string.
    StatsEditUnitAdd {
        instance: String,
        entity: String,
        friendly_name: String,
        metric: StatsMetric,
        window: String,
        buf: String,
    },
    /// Step 6: optional title.
    StatsEditTitleAdd {
        instance: String,
        entity: String,
        friendly_name: String,
        metric: StatsMetric,
        window: String,
        unit: Option<String>,
        buf: String,
    },
    // ---- Statistics context-menu flows ----
    /// Edit the metric of an existing Statistics card.
    StatsEditMetric {
        card_idx: usize,
        selected: usize,
    },
    /// Edit the window of an existing Statistics card.
    StatsEditWindow {
        card_idx: usize,
        buf: String,
    },
    /// Edit the unit of an existing Statistics card.
    StatsEditUnit {
        card_idx: usize,
        buf: String,
    },
    // ---- Image add-flow ----
    /// Step 1: pick source kind — 1 = image entity, 2 = camera.
    ImagePickSourceKind {
        selected: usize,
    },
    /// Step 2 (camera only): optional refresh interval in seconds.
    ImageEditRefreshSeconds {
        instance: String,
        entity: String,
        friendly_name: String,
        is_camera: bool,
        buf: String,
    },
    /// Step 3: optional title override.
    ImageEditTitleAdd {
        instance: String,
        entity: String,
        friendly_name: String,
        is_camera: bool,
        refresh_seconds: Option<u32>,
        buf: String,
    },
    // ---- Weather add-flow ----
    /// Step 3 (after entity pick): toggle show_forecast on/off.
    WxEditShowForecast {
        instance: String,
        entity: String,
        friendly_name: String,
        selected: usize, // 0 = yes, 1 = no
    },
    /// Step 4: numeric forecast_days (1-7, default 3).
    WxEditForecastDays {
        instance: String,
        entity: String,
        friendly_name: String,
        show_forecast: bool,
        buf: String,
    },
    /// Step 5: optional title.
    WxEditTitleAdd {
        instance: String,
        entity: String,
        friendly_name: String,
        show_forecast: bool,
        forecast_days: u8,
        buf: String,
    },
    /// Pick which EntityList item to edit an override for.
    /// Opened before `EditEntityListItemOverride` for EntityList cards so the
    /// user can choose which row rather than always defaulting to item 0.
    PickEntityListItemToOverride {
        card_idx: usize,
        /// (item_idx, entity_id) pairs from the card's entity list.
        items: Vec<(usize, String)>,
        selected: usize,
    },
    // ---- Cross-dashboard transfer flow ----
    /// Step 1: pick which dashboard to move/copy the card to.
    PickingTargetDashboard {
        op: TransferOp,
        source_card_idx: usize,
        selected: usize,
    },
    /// Step 2 (Grid target): pick which row.
    PickingTargetGridRow {
        op: TransferOp,
        source_card_idx: usize,
        target_dash: usize,
        selected: usize,
    },
    /// Step 3 (Grid target): pick which column within the chosen row.
    PickingTargetGridColumn {
        op: TransferOp,
        source_card_idx: usize,
        target_dash: usize,
        target_row: usize,
        selected: usize,
    },
    // ---- New-dashboard layout picker ----
    /// Shown immediately after `n` — user picks Free or Grid before the dashboard is configured.
    PickingNewDashboardLayout {
        selected: usize, // 0 = Free, 1 = Grid
    },
    /// Confirm before deleting the current dashboard.
    ConfirmDeleteDashboard,
    // ---- Grid structural flows ----
    /// Enter height for a new row (integer terminal rows or "auto").
    PickingNewRowHeight {
        buf: String,
    },
    /// Enter number of columns for the new row (after height was accepted).
    PickingNewRowColumnCount {
        height: RowHeight,
        buf: String,
    },
    /// Edit the height of an existing row.
    EditingRowHeight {
        row_idx: usize,
        buf: String,
    },
    /// Confirm before removing a row.
    ConfirmRemoveRow {
        row_idx: usize,
    },
    /// Confirm before removing a column.
    ConfirmRemoveColumn {
        row_idx: usize,
        col_idx: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferOp {
    Move,
    Copy,
}

/// Where the editor focus sits within a grid-layout dashboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridFocus {
    Card { row: usize, col: usize, pos_in_col: usize },
    Column { row: usize, col: usize },
    Row { row: usize },
}

/// Accumulates the first two threshold values while collecting severity input.
#[derive(Debug, Clone, Copy, Default)]
pub struct SeverityAccum {
    pub green: f64,
    pub yellow: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeriesIndexOp {
    Remove,
    SetColor,
    SetLabel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterFocus {
    Query,
    Title,
    HideToggle,
}

#[derive(Debug, Clone, Copy)]
pub enum MenuContext {
    Dashboard,
    Card(usize),
    Row(usize),
    GridColumn(usize, usize), // (row_idx, col_idx)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    RenameCard,
    ChangeEntity,
    #[allow(dead_code)]
    EditWindow,
    EditQuery,
    ToggleHideState,
    ToggleHideWhenEmpty,
    ToggleTicker,
    SetColorOverride,
    SetCardSize,
    DeleteCard,
    RenameDashboard,
    ResizeGrid,
    // Graph-specific actions
    AddGraphSeries,
    RemoveGraphSeries,
    SetGraphSeriesColor,
    SetGraphSeriesLabel,
    CycleGraphType,
    EditGraphWindow,
    EditGraphOrientation,
    // Gauge-specific actions
    EditSeverityThresholds,
    ToggleGaugeNeedle,
    // Clock-specific actions
    ClockEditFormat,
    ClockEditTimezone,
    // Statistics-specific actions
    StatsEditMetric,
    StatsEditWindow,
    StatsEditUnit,
    // Cross-dashboard transfer
    MoveToDashboard,
    CopyToDashboard,
    // Grid card actions
    MoveCardUpInColumn,
    MoveCardDownInColumn,
    MoveToColumn,
    // Row actions
    SetRowHeight,
    ToggleRowFillHeight,
    AddColumn,
    RemoveRow,
    MoveRowUp,
    MoveRowDown,
    // Column actions
    SetColumnFillHeight,
    RemoveColumn,
    MoveColumnLeft,
    MoveColumnRight,
    // Dashboard-level grid actions
    AddRow,
    // EntityList / FilteredEntityList per-entry overrides
    EditEntryOverride,
    // Text card content editing
    EditTextContent,
    // Dashboard management
    DeleteDashboard,
}

#[derive(Debug, Clone)]
pub struct MenuItem {
    pub action: MenuAction,
    pub label: &'static str,
}

pub fn card_menu_items(card: &Card) -> Vec<MenuItem> {
    let mut items: Vec<MenuItem> = Vec::new();
    items.push(MenuItem {
        action: MenuAction::RenameCard,
        label: "Rename card",
    });
    if let CardKind::Text { .. } = &card.kind {
        items.push(MenuItem {
            action: MenuAction::EditTextContent,
            label: "Edit content",
        });
    }
    let entity_change_label = match &card.kind {
        CardKind::EntityList { .. } => Some("Change entities"),
        CardKind::FilteredEntityList { .. }
        | CardKind::Graph { .. }
        | CardKind::Text { .. }
        | CardKind::Clock { .. } => None,
        _ => Some("Change entity"),
    };
    if let Some(label) = entity_change_label {
        items.push(MenuItem {
            action: MenuAction::ChangeEntity,
            label,
        });
    }
    if let CardKind::Graph {
        graph_type,
        entities,
        ..
    } = &card.kind
    {
        items.push(MenuItem {
            action: MenuAction::AddGraphSeries,
            label: "Add series",
        });
        if entities.len() > 1 {
            items.push(MenuItem {
                action: MenuAction::RemoveGraphSeries,
                label: "Remove series",
            });
        }
        items.push(MenuItem {
            action: MenuAction::SetGraphSeriesColor,
            label: "Set series colour",
        });
        items.push(MenuItem {
            action: MenuAction::SetGraphSeriesLabel,
            label: "Set series label",
        });
        items.push(MenuItem {
            action: MenuAction::CycleGraphType,
            label: "Change type",
        });
        if *graph_type == GraphType::Line {
            items.push(MenuItem {
                action: MenuAction::EditGraphWindow,
                label: "Window",
            });
        }
        if *graph_type == GraphType::Bar {
            items.push(MenuItem {
                action: MenuAction::EditGraphOrientation,
                label: "Orientation",
            });
        }
    }
    if let CardKind::FilteredEntityList {
        hide_when_empty, ..
    } = &card.kind
    {
        items.push(MenuItem {
            action: MenuAction::EditQuery,
            label: "Edit filter query",
        });
        items.push(MenuItem {
            action: MenuAction::ToggleHideState,
            label: "Toggle hide state column",
        });
        let hwe_label: &'static str = if *hide_when_empty {
            "Hide when empty: on"
        } else {
            "Hide when empty: off"
        };
        items.push(MenuItem {
            action: MenuAction::ToggleHideWhenEmpty,
            label: hwe_label,
        });
    }
    if let CardKind::Gauge { needle, .. } = &card.kind {
        items.push(MenuItem {
            action: MenuAction::EditSeverityThresholds,
            label: "Severity thresholds",
        });
        let needle_label: &'static str = if *needle { "Needle: on" } else { "Needle: off" };
        // card_idx is only available at dispatch time; use a placeholder, the
        // action carries the idx via the MenuContext at dispatch.
        items.push(MenuItem {
            action: MenuAction::ToggleGaugeNeedle,
            label: needle_label,
        });
    }
    if let CardKind::Clock { .. } = &card.kind {
        items.push(MenuItem {
            action: MenuAction::ClockEditFormat,
            label: "Format",
        });
        items.push(MenuItem {
            action: MenuAction::ClockEditTimezone,
            label: "Timezone",
        });
    }
    if let CardKind::Statistics { .. } = &card.kind {
        items.push(MenuItem {
            action: MenuAction::StatsEditMetric,
            label: "Metric",
        });
        items.push(MenuItem {
            action: MenuAction::StatsEditWindow,
            label: "Window",
        });
        items.push(MenuItem {
            action: MenuAction::StatsEditUnit,
            label: "Unit",
        });
    }
    if matches!(card.kind, CardKind::Entity { .. }) {
        items.push(MenuItem {
            action: MenuAction::ToggleTicker,
            label: "Toggle ticker mode",
        });
    }
    items.push(MenuItem {
        action: MenuAction::SetColorOverride,
        label: "Color override",
    });
    items.push(MenuItem {
        action: MenuAction::SetCardSize,
        label: "Size",
    });
    items.push(MenuItem {
        action: MenuAction::MoveToDashboard,
        label: "Move to dashboard…",
    });
    items.push(MenuItem {
        action: MenuAction::CopyToDashboard,
        label: "Copy to dashboard…",
    });
    items.push(MenuItem {
        action: MenuAction::DeleteCard,
        label: "Delete card",
    });
    items
}

/// Grid-only card menu additions (appended by `open_menu` when in a grid dashboard).
pub fn grid_card_extra_items() -> Vec<MenuItem> {
    vec![
        MenuItem { action: MenuAction::MoveCardUpInColumn, label: "Move up in column" },
        MenuItem { action: MenuAction::MoveCardDownInColumn, label: "Move down in column" },
        MenuItem { action: MenuAction::MoveToColumn, label: "Move to column…" },
    ]
}

pub fn row_menu_items() -> Vec<MenuItem> {
    vec![
        MenuItem { action: MenuAction::SetRowHeight, label: "Set row height" },
        MenuItem { action: MenuAction::ToggleRowFillHeight, label: "Toggle fill_height default" },
        MenuItem { action: MenuAction::AddColumn, label: "Add column" },
        MenuItem { action: MenuAction::MoveRowUp, label: "Move row up" },
        MenuItem { action: MenuAction::MoveRowDown, label: "Move row down" },
        MenuItem { action: MenuAction::RemoveRow, label: "Remove row" },
    ]
}

pub fn column_menu_items() -> Vec<MenuItem> {
    vec![
        MenuItem { action: MenuAction::SetColumnFillHeight, label: "Toggle fill_height" },
        MenuItem { action: MenuAction::MoveColumnLeft, label: "Move column left" },
        MenuItem { action: MenuAction::MoveColumnRight, label: "Move column right" },
        MenuItem { action: MenuAction::RemoveColumn, label: "Remove column" },
    ]
}

pub fn dashboard_menu_items(is_grid: bool) -> Vec<MenuItem> {
    let mut items = vec![
        MenuItem { action: MenuAction::RenameDashboard, label: "Rename dashboard" },
    ];
    if is_grid {
        items.push(MenuItem { action: MenuAction::AddRow, label: "Add row" });
    } else {
        items.push(MenuItem { action: MenuAction::ResizeGrid, label: "Grid size (cols × rows)" });
    }
    items.push(MenuItem { action: MenuAction::DeleteDashboard, label: "Delete dashboard" });
    items
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardTypeStub {
    Entity,
    Toggle,
    Gauge,
    Graph,
    Text,
    EntityList,
    FilteredEntityList,
    Clock,
    Statistics,
    MediaPlayer,
    Image,
    Weather,
}

impl CardTypeStub {
    pub const ALL: &'static [CardTypeStub] = &[
        CardTypeStub::Entity,
        CardTypeStub::Toggle,
        CardTypeStub::Gauge,
        CardTypeStub::Graph,
        CardTypeStub::Text,
        CardTypeStub::EntityList,
        CardTypeStub::FilteredEntityList,
        CardTypeStub::Clock,
        CardTypeStub::Statistics,
        CardTypeStub::MediaPlayer,
        CardTypeStub::Image,
        CardTypeStub::Weather,
    ];
    pub fn label(self) -> &'static str {
        match self {
            CardTypeStub::Entity => "entity",
            CardTypeStub::Toggle => "toggle",
            CardTypeStub::Gauge => "gauge",
            CardTypeStub::Graph => "graph",
            CardTypeStub::Text => "text",
            CardTypeStub::EntityList => "entity list (multi)",
            CardTypeStub::FilteredEntityList => "filtered list (glob + filters)",
            CardTypeStub::Clock => "clock",
            CardTypeStub::Statistics => "statistics",
            CardTypeStub::MediaPlayer => "media player",
            CardTypeStub::Image => "image / camera",
            CardTypeStub::Weather => "weather",
        }
    }
}

impl EditorState {
    pub fn new(dash_idx: usize, source_path: Option<std::path::PathBuf>) -> Self {
        Self {
            dash_idx,
            cursor_col: 0,
            cursor_row: 0,
            selected_card: None,
            mode: EditorMode::Browse,
            undo_stack: Vec::new(),
            dirty: false,
            source_path,
            edit_target: None,
            image_pending_is_camera: None,
            grid_focus: None,
        }
    }

    pub fn snapshot(&mut self, dash: &Dashboard) {
        if self.undo_stack.len() == MAX_UNDO {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(dash.clone());
    }

    pub fn undo(&mut self, dash: &mut Dashboard) -> bool {
        if let Some(prev) = self.undo_stack.pop() {
            *dash = prev;
            self.dirty = true;
            true
        } else {
            false
        }
    }

    pub fn move_cursor(&mut self, dx: i32, dy: i32, dash: &Dashboard) {
        if let Some(grid) = dash.free_grid() {
            self.cursor_col = clamp_add(self.cursor_col, dx, grid.cols.saturating_sub(1));
            self.cursor_row = clamp_add(self.cursor_row, dy, grid.rows.saturating_sub(1));
        }
    }

    pub fn select_at_cursor(&mut self, dash: &Dashboard) {
        self.selected_card = card_at(dash, self.cursor_col, self.cursor_row);
    }

    pub fn resize_selected(&mut self, dw: i32, dh: i32, dash: &mut Dashboard) {
        let Some(i) = self.selected_card else { return };
        let (grid_cols, grid_rows) = dash.free_grid().map(|g| (g.cols, g.rows)).unwrap_or((12, 8));
        let Some(card) = dash.card_mut(i) else {
            return;
        };
        self.snapshot_inner(card);
        let Some(pos) = card.pos.as_mut() else { return; };
        let new_w = clamp_dim(pos.w, dw, grid_cols - pos.col);
        let new_h = clamp_dim(pos.h, dh, grid_rows - pos.row);
        pos.w = new_w.max(1);
        pos.h = new_h.max(1);
        self.dirty = true;
    }

    pub fn move_selected_to_cursor(&mut self, dash: &mut Dashboard) {
        let Some(i) = self.selected_card else { return };
        let target_col = self.cursor_col;
        let target_row = self.cursor_row;
        let (grid_cols, grid_rows) = dash.free_grid().map(|g| (g.cols, g.rows)).unwrap_or((12, 8));
        let Some(card) = dash.card_mut(i) else {
            return;
        };
        let Some(pos) = card.pos.as_mut() else { return; };
        let new_col = target_col.min(grid_cols.saturating_sub(pos.w));
        let new_row = target_row.min(grid_rows.saturating_sub(pos.h));
        if pos.col != new_col || pos.row != new_row {
            pos.col = new_col;
            pos.row = new_row;
            self.dirty = true;
        }
    }

    pub fn delete_selected(&mut self, dash: &mut Dashboard) {
        let Some(i) = self.selected_card.take() else {
            return;
        };
        if dash.remove_card_at(i).is_some() {
            self.grid_focus = None;
            self.dirty = true;
        }
    }

    pub fn add_card(&mut self, dash: &mut Dashboard, kind: CardKind) {
        if let Some(idx) = self.edit_target.take() {
            if let Some(card) = dash.card_mut(idx) {
                card.kind = kind;
                self.dirty = true;
                self.selected_card = Some(idx);
                return;
            }
        }
        match &dash.layout {
            DashboardLayout::Grid { .. } => {
                let (row_idx, col_idx) = match self.grid_focus {
                    Some(GridFocus::Card { row, col, .. }) => (row, col),
                    Some(GridFocus::Column { row, col }) => (row, col),
                    Some(GridFocus::Row { row }) => (row, 0),
                    None => (0, 0),
                };
                let card = Card {
                    id: dash.next_card_id(),
                    pos: None,
                    height: None,
                    kind,
                    color: None,
                    size: CardSize::Normal,
                };
                dash.insert_card_grid(row_idx, col_idx, card);
                // Move focus to the newly added card (last in that column).
                if let DashboardLayout::Grid { rows } = &dash.layout {
                    if let Some(row) = rows.get(row_idx) {
                        if let Some(col) = row.columns.get(col_idx) {
                            let pos_in_col = col.cards.len().saturating_sub(1);
                            self.grid_focus = Some(GridFocus::Card { row: row_idx, col: col_idx, pos_in_col });
                        }
                    }
                }
                self.selected_card = self.grid_focus.and_then(|gf| {
                    if let GridFocus::Card { row, col, pos_in_col } = gf {
                        dash.flat_idx_from_grid(row, col, pos_in_col)
                    } else {
                        None
                    }
                });
                self.dirty = true;
            }
            DashboardLayout::Free { .. } => {
                let (gcols, grows) = dash.free_grid().map(|g| (g.cols, g.rows)).unwrap_or((12, 8));
                let card = Card {
                    id: dash.next_card_id(),
                    pos: Some(Pos {
                        col: self.cursor_col,
                        row: self.cursor_row,
                        w: 3.min(gcols.saturating_sub(self.cursor_col).max(1)),
                        h: 2.min(grows.saturating_sub(self.cursor_row).max(1)),
                    }),
                    height: None,
                    kind,
                    color: None,
                    size: CardSize::Normal,
                };
                dash.push_card_free(card);
                self.selected_card = Some(dash.card_count() - 1);
                self.dirty = true;
            }
        }
    }

    /// Replace just the title of the card at `idx` (keeps everything else).
    pub fn retitle_card(&mut self, dash: &mut Dashboard, idx: usize, new_title: Option<String>) {
        let Some(card) = dash.card_mut(idx) else {
            return;
        };
        match &mut card.kind {
            CardKind::Entity { title, .. }
            | CardKind::Toggle { title, .. }
            | CardKind::Gauge { title, .. }
            | CardKind::Graph { title, .. }
            | CardKind::Text { title, .. }
            | CardKind::EntityList { title, .. }
            | CardKind::FilteredEntityList { title, .. }
            | CardKind::Clock { title, .. }
            | CardKind::Statistics { title, .. }
            | CardKind::MediaPlayer { title, .. }
            | CardKind::Image { title, .. }
            | CardKind::Weather { title, .. } => {
                *title = new_title;
            }
        }
        self.dirty = true;
    }

    fn snapshot_inner(&mut self, _card: &Card) {
        // Reserved hook for finer-grained undo. Currently full-dashboard undo only.
    }
}

fn clamp_add(v: u16, delta: i32, max: u16) -> u16 {
    let n = i32::from(v) + delta;
    n.clamp(0, i32::from(max)) as u16
}

fn clamp_dim(v: u16, delta: i32, headroom: u16) -> u16 {
    let n = i32::from(v) + delta;
    n.clamp(1, i32::from(headroom).max(1)) as u16
}

pub fn card_at(dash: &Dashboard, col: u16, row: u16) -> Option<usize> {
    if dash.free_grid().is_none() {
        return None;
    }
    // Iterate in reverse so newest (drawn last) wins.
    let cards: Vec<_> = dash.cards_iter().enumerate().collect();
    for (i, c) in cards.into_iter().rev() {
        if let Some(pos) = c.pos {
            if col >= pos.col
                && col < pos.col + pos.w
                && row >= pos.row
                && row < pos.row + pos.h
            {
                return Some(i);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dashboard::{CardId, Grid};
    use crate::dashboard::Pos;

    fn make_dash() -> Dashboard {
        Dashboard {
            name: "t".into(),
            layout: DashboardLayout::Free {
                grid: Grid { cols: 12, rows: 8 },
                cards: vec![
                    Card {
                        id: CardId(1),
                        pos: Some(Pos {
                            col: 0,
                            row: 0,
                            w: 3,
                            h: 2,
                        }),
                        height: None,
                        kind: CardKind::Text {
                            markdown: "a".into(),
                            title: None,
                        },
                        color: None,
                        size: CardSize::Normal,
                    },
                    Card {
                        id: CardId(2),
                        pos: Some(Pos {
                            col: 4,
                            row: 0,
                            w: 2,
                            h: 2,
                        }),
                        height: None,
                        kind: CardKind::Text {
                            markdown: "b".into(),
                            title: None,
                        },
                        color: None,
                        size: CardSize::Normal,
                    },
                ],
            },
        }
    }

    #[test]
    fn card_at_hits() {
        let d = make_dash();
        assert_eq!(card_at(&d, 1, 1), Some(0));
        assert_eq!(card_at(&d, 4, 0), Some(1));
        assert_eq!(card_at(&d, 7, 0), None);
    }

    #[test]
    fn move_clamps_to_grid() {
        let d = make_dash();
        let mut e = EditorState::new(0, None);
        e.cursor_col = 11;
        e.cursor_row = 7;
        e.move_cursor(5, 5, &d);
        assert_eq!(e.cursor_col, 11);
        assert_eq!(e.cursor_row, 7);
    }

    #[test]
    fn delete_removes_card() {
        let mut d = make_dash();
        let mut e = EditorState::new(0, None);
        e.selected_card = Some(1);
        e.delete_selected(&mut d);
        assert_eq!(d.card_count(), 1);
    }

    #[test]
    fn undo_restores() {
        let mut d = make_dash();
        let mut e = EditorState::new(0, None);
        e.snapshot(&d);
        d.remove_card_at_free(d.card_count() - 1);
        assert_eq!(d.card_count(), 1);
        e.undo(&mut d);
        assert_eq!(d.card_count(), 2);
    }
}
