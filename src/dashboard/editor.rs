use crate::dashboard::{Card, CardKind, Dashboard, Pos};

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
}

#[derive(Debug, Clone)]
pub enum EditorMode {
    Browse,
    /// User pressed `a` — picking a card type.
    PickingType,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    RenameCard,
    ChangeEntity,
    EditWindow,
    EditQuery,
    ToggleHideState,
    ToggleTicker,
    DeleteCard,
    RenameDashboard,
    ResizeGrid,
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
    let entity_change_label = match &card.kind {
        CardKind::EntityList { .. } => Some("Change entities"),
        CardKind::FilteredEntityList { .. } => None,
        CardKind::Text { .. } => None,
        _ => Some("Change entity"),
    };
    if let Some(label) = entity_change_label {
        items.push(MenuItem {
            action: MenuAction::ChangeEntity,
            label,
        });
    }
    if matches!(card.kind, CardKind::Sparkline { .. }) {
        items.push(MenuItem {
            action: MenuAction::EditWindow,
            label: "Set history window",
        });
    }
    if matches!(card.kind, CardKind::FilteredEntityList { .. }) {
        items.push(MenuItem {
            action: MenuAction::EditQuery,
            label: "Edit filter query",
        });
        items.push(MenuItem {
            action: MenuAction::ToggleHideState,
            label: "Toggle hide state column",
        });
    }
    if matches!(card.kind, CardKind::Entity { .. }) {
        items.push(MenuItem {
            action: MenuAction::ToggleTicker,
            label: "Toggle ticker mode",
        });
    }
    items.push(MenuItem {
        action: MenuAction::DeleteCard,
        label: "Delete card",
    });
    items
}

pub fn dashboard_menu_items() -> Vec<MenuItem> {
    vec![
        MenuItem {
            action: MenuAction::RenameDashboard,
            label: "Rename dashboard",
        },
        MenuItem {
            action: MenuAction::ResizeGrid,
            label: "Grid size (cols × rows)",
        },
    ]
}

#[derive(Debug, Clone, Copy)]
pub enum CardTypeStub {
    Entity,
    Toggle,
    Gauge,
    Sparkline,
    Text,
    EntityList,
    FilteredEntityList,
}

impl CardTypeStub {
    pub const ALL: &'static [CardTypeStub] = &[
        CardTypeStub::Entity,
        CardTypeStub::Toggle,
        CardTypeStub::Gauge,
        CardTypeStub::Sparkline,
        CardTypeStub::Text,
        CardTypeStub::EntityList,
        CardTypeStub::FilteredEntityList,
    ];
    pub fn label(self) -> &'static str {
        match self {
            CardTypeStub::Entity => "entity",
            CardTypeStub::Toggle => "toggle",
            CardTypeStub::Gauge => "gauge",
            CardTypeStub::Sparkline => "sparkline",
            CardTypeStub::Text => "text",
            CardTypeStub::EntityList => "entity list (multi)",
            CardTypeStub::FilteredEntityList => "filtered list (glob + filters)",
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
        self.cursor_col = clamp_add(self.cursor_col, dx, dash.grid.cols.saturating_sub(1));
        self.cursor_row = clamp_add(self.cursor_row, dy, dash.grid.rows.saturating_sub(1));
    }

    pub fn select_at_cursor(&mut self, dash: &Dashboard) {
        self.selected_card = card_at(dash, self.cursor_col, self.cursor_row);
    }

    pub fn resize_selected(&mut self, dw: i32, dh: i32, dash: &mut Dashboard) {
        let Some(i) = self.selected_card else { return };
        let Some(card) = dash.cards.get_mut(i) else {
            return;
        };
        self.snapshot_inner(card);
        let new_w = clamp_dim(card.pos.w, dw, dash.grid.cols - card.pos.col);
        let new_h = clamp_dim(card.pos.h, dh, dash.grid.rows - card.pos.row);
        card.pos.w = new_w.max(1);
        card.pos.h = new_h.max(1);
        self.dirty = true;
    }

    pub fn move_selected_to_cursor(&mut self, dash: &mut Dashboard) {
        let Some(i) = self.selected_card else { return };
        let target_col = self.cursor_col;
        let target_row = self.cursor_row;
        let Some(card) = dash.cards.get_mut(i) else {
            return;
        };
        let new_col = target_col.min(dash.grid.cols.saturating_sub(card.pos.w));
        let new_row = target_row.min(dash.grid.rows.saturating_sub(card.pos.h));
        if card.pos.col != new_col || card.pos.row != new_row {
            card.pos.col = new_col;
            card.pos.row = new_row;
            self.dirty = true;
        }
    }

    pub fn delete_selected(&mut self, dash: &mut Dashboard) {
        let Some(i) = self.selected_card.take() else {
            return;
        };
        if i < dash.cards.len() {
            dash.cards.remove(i);
            self.dirty = true;
        }
    }

    pub fn add_card(&mut self, dash: &mut Dashboard, kind: CardKind) {
        if let Some(idx) = self.edit_target.take() {
            if let Some(card) = dash.cards.get_mut(idx) {
                card.kind = kind;
                self.dirty = true;
                self.selected_card = Some(idx);
                return;
            }
        }
        let card = Card {
            pos: Pos {
                col: self.cursor_col,
                row: self.cursor_row,
                w: 3.min(dash.grid.cols.saturating_sub(self.cursor_col).max(1)),
                h: 2.min(dash.grid.rows.saturating_sub(self.cursor_row).max(1)),
            },
            kind,
        };
        dash.cards.push(card);
        self.selected_card = Some(dash.cards.len() - 1);
        self.dirty = true;
    }

    /// Replace just the title of the card at `idx` (keeps everything else).
    pub fn retitle_card(&mut self, dash: &mut Dashboard, idx: usize, new_title: Option<String>) {
        let Some(card) = dash.cards.get_mut(idx) else {
            return;
        };
        match &mut card.kind {
            CardKind::Entity { title, .. }
            | CardKind::Toggle { title, .. }
            | CardKind::Gauge { title, .. }
            | CardKind::Sparkline { title, .. }
            | CardKind::Text { title, .. }
            | CardKind::EntityList { title, .. }
            | CardKind::FilteredEntityList { title, .. } => {
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
    // Iterate in reverse so newest (drawn last) wins.
    for (i, c) in dash.cards.iter().enumerate().rev() {
        if col >= c.pos.col
            && col < c.pos.col + c.pos.w
            && row >= c.pos.row
            && row < c.pos.row + c.pos.h
        {
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dashboard::{Grid, Pos};

    fn make_dash() -> Dashboard {
        Dashboard {
            name: "t".into(),
            grid: Grid { cols: 12, rows: 8 },
            cards: vec![
                Card {
                    pos: Pos {
                        col: 0,
                        row: 0,
                        w: 3,
                        h: 2,
                    },
                    kind: CardKind::Text {
                        markdown: "a".into(),
                        title: None,
                    },
                },
                Card {
                    pos: Pos {
                        col: 4,
                        row: 0,
                        w: 2,
                        h: 2,
                    },
                    kind: CardKind::Text {
                        markdown: "b".into(),
                        title: None,
                    },
                },
            ],
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
        assert_eq!(d.cards.len(), 1);
    }

    #[test]
    fn undo_restores() {
        let mut d = make_dash();
        let mut e = EditorState::new(0, None);
        e.snapshot(&d);
        d.cards.pop();
        assert_eq!(d.cards.len(), 1);
        e.undo(&mut d);
        assert_eq!(d.cards.len(), 2);
    }
}
