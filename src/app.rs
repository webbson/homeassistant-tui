use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Duration;

use color_eyre::Result;
use crossterm::event::{
    Event as CtEvent, EventStream, KeyCode, KeyEvent, KeyEventKind, MouseButton, MouseEvent,
    MouseEventKind,
};
use futures::StreamExt;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tokio::sync::mpsc;
use tracing::{error, info};

use crate::config::{self, Alias, Config};
use crate::dashboard::editor::{CardTypeStub, EditorMode, EditorState};
use crate::dashboard::{self, CardKind, Dashboard};
use crate::event::AppEvent;
use crate::ha::{
    client, EntityId, ForecastDay, ForecastKind, ImageFetchKind, InstanceRegistry, InstanceRuntime,
};
use crate::screens::Screen;
use crate::ui;
use crate::ui::theme::Theme;
use crate::util::history::RingBuf;

const HISTORY_CAP: usize = 8192;

pub struct App {
    pub should_quit: bool,
    pub config: Option<Config>,
    pub instances: InstanceRegistry,
    pub screen: Screen,
    pub theme: Theme,
    pub dashboards: Vec<Dashboard>,
    pub history: HashMap<(Alias, EntityId), RingBuf>,
    pub editor: Option<EditorState>,
    pub dashboards_path: Option<PathBuf>,
    pub last_terminal_size: (u16, u16),
    pub mouse_drag: Option<MouseDrag>,
    pub show_help: bool,
    pub ticker_offset: usize,
    pub status_msg: Option<String>,
    pub last_error: Option<String>,
    #[allow(dead_code)]
    pub tx: mpsc::UnboundedSender<AppEvent>,
    /// Weather forecast cache keyed by (instance, entity).
    pub weather_forecasts: HashMap<(Alias, EntityId), Vec<ForecastDay>>,
    /// Decoded image cache keyed by (instance, entity).
    pub image_cache: HashMap<(Alias, EntityId), ImageCacheEntry>,
    /// Entities currently being fetched — prevents duplicate in-flight requests.
    pub image_inflight: HashSet<(Alias, EntityId)>,
    /// ratatui-image protocol picker; None when terminal doesn't support graphics.
    pub image_picker: Option<ratatui_image::picker::Picker>,
}

pub struct ImageCacheEntry {
    pub protocol: ratatui_image::protocol::StatefulProtocol,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct MouseDrag {
    pub origin_col: u16,
    pub origin_row: u16,
}

impl App {
    fn new(tx: mpsc::UnboundedSender<AppEvent>) -> Self {
        Self {
            should_quit: false,
            config: None,
            instances: InstanceRegistry::new(),
            screen: Screen::default(),
            theme: Theme::empty(),
            dashboards: Vec::new(),
            history: HashMap::new(),
            editor: None,
            dashboards_path: None,
            last_terminal_size: (0, 0),
            mouse_drag: None,
            show_help: false,
            ticker_offset: 0,
            status_msg: None,
            last_error: None,
            tx,
            weather_forecasts: HashMap::new(),
            image_cache: HashMap::new(),
            image_inflight: HashSet::new(),
            image_picker: None,
        }
    }

    fn handle_term(&mut self, ev: CtEvent) {
        match ev {
            CtEvent::Key(k) if k.kind == KeyEventKind::Press => self.handle_key(k),
            CtEvent::Mouse(m) => self.handle_mouse(m),
            CtEvent::Resize(w, h) => self.last_terminal_size = (w, h),
            _ => {}
        }
    }

    fn handle_key(&mut self, k: KeyEvent) {
        // Clear transient status on next key (so "saved" doesn't linger).
        self.status_msg = None;
        if self.show_help {
            self.show_help = false;
            return;
        }
        if matches!(k.code, KeyCode::Char('?')) {
            self.show_help = true;
            return;
        }
        if matches!(self.screen, Screen::Editor) {
            self.handle_key_editor(k);
            return;
        }
        // MediaPlayer-scoped keys: intercept before generic dispatch so 'n', 'm', etc.
        // don't fall through to create_new_dashboard or other global bindings.
        if let KeyCode::Char(ch) = k.code {
            if self.handle_key_media_player(ch) {
                return;
            }
        }
        match k.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::PageUp => self.move_selection(-10),
            KeyCode::PageDown => self.move_selection(10),
            KeyCode::Left | KeyCode::Char('h') => self.move_card_selection(-1),
            KeyCode::Right | KeyCode::Char('l') => self.move_card_selection(1),
            KeyCode::Home => self.set_selection(0),
            KeyCode::End => self.set_selection(usize::MAX),
            KeyCode::Char('i') => self.screen = Screen::Instances { selected: 0 },
            KeyCode::Char('E') => {
                self.screen = Screen::Entities {
                    instance_filter: None,
                    search: String::new(),
                    selected: 0,
                }
            }
            KeyCode::Char('f') => self.cycle_instance_filter(),
            KeyCode::Char('e') => self.enter_editor(),
            KeyCode::Char('n') => self.create_new_dashboard(),
            KeyCode::Char(c) if ('1'..='9').contains(&c) => {
                let idx = (c as u8 - b'1') as usize;
                if idx < self.dashboards.len() {
                    self.screen = Screen::Dashboard {
                        idx,
                        selected_card: 0,
                        sub_index: 0,
                    };
                }
            }
            KeyCode::Enter => self.trigger_default_action(),
            _ => {}
        }
    }

    fn handle_key_editor(&mut self, k: KeyEvent) {
        let Some(editor) = self.editor.as_mut() else {
            return;
        };
        let dash_idx = editor.dash_idx;

        // Modal modes have their own handling
        match &mut editor.mode {
            EditorMode::PickingType => {
                if let KeyCode::Char(c) = k.code {
                    if let Some(d) = c.to_digit(10) {
                        if let Some(kind) = CardTypeStub::ALL.get((d as usize).wrapping_sub(1)) {
                            self.start_card_after_type(*kind);
                            return;
                        }
                    }
                }
                if matches!(k.code, KeyCode::Esc) {
                    editor.mode = EditorMode::Browse;
                }
                return;
            }
            EditorMode::PickingInstance {
                card_type,
                selected,
            } => {
                let aliases: Vec<String> = self.instances.runtimes.keys().cloned().collect();
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Up | KeyCode::Char('k') if *selected > 0 => {
                        *selected -= 1;
                    }
                    KeyCode::Down | KeyCode::Char('j') if *selected + 1 < aliases.len() => {
                        *selected += 1;
                    }
                    KeyCode::Enter => {
                        if let Some(inst) = aliases.get(*selected) {
                            let ct = *card_type;
                            let inst = inst.clone();
                            editor.mode = picker_mode_for(ct, inst);
                        }
                    }
                    KeyCode::Char(c) if c.is_ascii_digit() => {
                        let i = c.to_digit(10).unwrap() as usize;
                        if i >= 1 && i <= aliases.len() {
                            let ct = *card_type;
                            let inst = aliases[i - 1].clone();
                            editor.mode = picker_mode_for(ct, inst);
                        }
                    }
                    _ => {}
                }
                return;
            }
            EditorMode::PickingEntity {
                card_type,
                instance,
                query,
                selected,
            } => {
                let domain_prefix = domain_prefix_for_type(*card_type);
                let rows = entity_search_filtered(&self.instances, instance, query, domain_prefix);
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Backspace => {
                        query.pop();
                        *selected = 0;
                    }
                    KeyCode::Up if *selected > 0 => {
                        *selected -= 1;
                    }
                    KeyCode::Down if *selected + 1 < rows.len() => {
                        *selected += 1;
                    }
                    KeyCode::PageUp => {
                        *selected = selected.saturating_sub(10);
                    }
                    KeyCode::PageDown => {
                        *selected = (*selected + 10).min(rows.len().saturating_sub(1));
                    }
                    KeyCode::Char(c) => {
                        query.push(c);
                        *selected = 0;
                    }
                    KeyCode::Enter => {
                        if let Some(pick) = rows.get(*selected) {
                            let ct = *card_type;
                            let inst = instance.clone();
                            let eid = pick.entity_id.clone();
                            let fname = pick.friendly_name.clone();
                            if ct == CardTypeStub::Statistics {
                                editor.mode = EditorMode::StatsPickMetric {
                                    instance: inst,
                                    entity: eid,
                                    friendly_name: fname,
                                    selected: 0,
                                };
                            } else if ct == CardTypeStub::Image {
                                let is_camera = eid.starts_with("camera.");
                                if is_camera {
                                    editor.mode = EditorMode::ImageEditRefreshSeconds {
                                        instance: inst,
                                        entity: eid,
                                        friendly_name: fname,
                                        is_camera: true,
                                        buf: String::new(),
                                    };
                                } else {
                                    // image entity — skip refresh_seconds step
                                    editor.mode = EditorMode::ImageEditTitleAdd {
                                        instance: inst,
                                        entity: eid,
                                        friendly_name: fname,
                                        is_camera: false,
                                        refresh_seconds: None,
                                        buf: String::new(),
                                    };
                                }
                            } else {
                                editor.mode = EditorMode::EditingTitle {
                                    card_type: ct,
                                    instance: inst,
                                    entity: eid,
                                    friendly_name: fname,
                                    title_buffer: String::new(),
                                };
                            }
                        }
                    }
                    _ => {}
                }
                return;
            }
            EditorMode::EditingTitle {
                card_type,
                instance,
                entity,
                friendly_name,
                title_buffer,
            } => {
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Backspace => {
                        title_buffer.pop();
                    }
                    KeyCode::Char(c) => title_buffer.push(c),
                    KeyCode::Enter => {
                        let ct = *card_type;
                        let inst = instance.clone();
                        let ent = entity.clone();
                        let title = if title_buffer.trim().is_empty() {
                            if friendly_name.is_empty() {
                                None
                            } else {
                                Some(friendly_name.clone())
                            }
                        } else {
                            Some(title_buffer.trim().to_string())
                        };
                        editor.mode = EditorMode::Browse;
                        let kind = build_typed_card(ct, inst, ent, title);
                        let Some(dash) = self.dashboards.get_mut(dash_idx) else {
                            return;
                        };
                        let Some(editor2) = self.editor.as_mut() else {
                            return;
                        };
                        editor2.snapshot(dash);
                        editor2.add_card(dash, kind);
                    }
                    _ => {}
                }
                return;
            }
            EditorMode::PickingMulti {
                instance,
                query,
                selected,
                picked,
            } => {
                let rows = entity_search(&self.instances, instance, query);
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Backspace => {
                        query.pop();
                        *selected = 0;
                    }
                    KeyCode::Up if *selected > 0 => {
                        *selected -= 1;
                    }
                    KeyCode::Down if *selected + 1 < rows.len() => {
                        *selected += 1;
                    }
                    KeyCode::Char(' ') => {
                        if let Some(pick) = rows.get(*selected) {
                            let pair = (pick.entity_id.clone(), pick.friendly_name.clone());
                            if let Some(pos) = picked.iter().position(|p| p.0 == pair.0) {
                                picked.remove(pos);
                            } else {
                                picked.push(pair);
                            }
                        }
                    }
                    KeyCode::Char(c) => {
                        query.push(c);
                        *selected = 0;
                    }
                    KeyCode::Enter => {
                        if picked.is_empty() {
                            // Treat Enter as toggle when no picks yet, then continue.
                            if let Some(pick) = rows.get(*selected) {
                                picked.push((pick.entity_id.clone(), pick.friendly_name.clone()));
                            }
                            return;
                        }
                        let inst = instance.clone();
                        let p = picked.clone();
                        editor.mode = EditorMode::EditingEntityListTitle {
                            instance: inst,
                            picked: p,
                            title_buffer: String::new(),
                        };
                    }
                    _ => {}
                }
                return;
            }
            EditorMode::EditingEntityListTitle {
                instance,
                picked,
                title_buffer,
            } => {
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Backspace => {
                        title_buffer.pop();
                    }
                    KeyCode::Char(c) => title_buffer.push(c),
                    KeyCode::Enter => {
                        let title = if title_buffer.trim().is_empty() {
                            None
                        } else {
                            Some(title_buffer.trim().to_string())
                        };
                        let inst = instance.clone();
                        let entities: Vec<String> =
                            picked.iter().map(|(eid, _)| eid.clone()).collect();
                        editor.mode = EditorMode::Browse;
                        let kind = CardKind::EntityList {
                            instance: inst,
                            entities,
                            title,
                        };
                        let Some(dash) = self.dashboards.get_mut(dash_idx) else {
                            return;
                        };
                        let Some(editor2) = self.editor.as_mut() else {
                            return;
                        };
                        editor2.snapshot(dash);
                        editor2.add_card(dash, kind);
                    }
                    _ => {}
                }
                return;
            }
            EditorMode::EditingTextBody {
                title_buffer,
                body_buffer,
                focus_body,
            } => {
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Tab => *focus_body = !*focus_body,
                    KeyCode::Backspace => {
                        if *focus_body {
                            body_buffer.pop();
                        } else {
                            title_buffer.pop();
                        }
                    }
                    KeyCode::Char(c) => {
                        if *focus_body {
                            body_buffer.push(c);
                        } else {
                            title_buffer.push(c);
                        }
                    }
                    KeyCode::Enter => {
                        if *focus_body {
                            body_buffer.push('\n');
                        } else {
                            *focus_body = true;
                        }
                    }
                    KeyCode::F(2) => {
                        let title = if title_buffer.trim().is_empty() {
                            None
                        } else {
                            Some(title_buffer.trim().to_string())
                        };
                        let body = body_buffer.clone();
                        editor.mode = EditorMode::Browse;
                        let kind = CardKind::Text {
                            markdown: body,
                            title,
                        };
                        let Some(dash) = self.dashboards.get_mut(dash_idx) else {
                            return;
                        };
                        let Some(editor2) = self.editor.as_mut() else {
                            return;
                        };
                        editor2.snapshot(dash);
                        editor2.add_card(dash, kind);
                    }
                    _ => {}
                }
                return;
            }
            EditorMode::ConfirmExit => {
                match k.code {
                    KeyCode::Char('y') | KeyCode::Char('Y') => {
                        self.editor = None;
                        self.screen = Screen::Dashboard {
                            idx: dash_idx,
                            selected_card: 0,
                            sub_index: 0,
                        };
                    }
                    KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                        editor.mode = EditorMode::Browse;
                    }
                    _ => {}
                }
                return;
            }
            EditorMode::ConfirmDelete => {
                match k.code {
                    KeyCode::Char('y') | KeyCode::Char('Y') => {
                        let dash = match self.dashboards.get_mut(dash_idx) {
                            Some(d) => d,
                            None => return,
                        };
                        editor.snapshot(dash);
                        editor.delete_selected(dash);
                        editor.mode = EditorMode::Browse;
                    }
                    KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                        editor.mode = EditorMode::Browse;
                    }
                    _ => {}
                }
                return;
            }
            EditorMode::Renaming { buffer } => {
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Backspace => {
                        buffer.pop();
                    }
                    KeyCode::Char(c) => buffer.push(c),
                    KeyCode::Enter => {
                        let new_name = buffer.trim().to_string();
                        editor.mode = EditorMode::Browse;
                        if !new_name.is_empty() {
                            if let Some(dash) = self.dashboards.get_mut(dash_idx) {
                                if let Some(ed) = self.editor.as_mut() {
                                    ed.snapshot(dash);
                                    dash.name = new_name;
                                    ed.dirty = true;
                                }
                            }
                        }
                    }
                    _ => {}
                }
                return;
            }
            EditorMode::ResizingGrid {
                cols_buffer,
                rows_buffer,
                focus_rows,
            } => {
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Tab => *focus_rows = !*focus_rows,
                    KeyCode::Backspace => {
                        if *focus_rows {
                            rows_buffer.pop();
                        } else {
                            cols_buffer.pop();
                        }
                    }
                    KeyCode::Char(c) if c.is_ascii_digit() => {
                        if *focus_rows {
                            rows_buffer.push(c);
                        } else {
                            cols_buffer.push(c);
                        }
                    }
                    KeyCode::Enter => {
                        let cols: u16 = cols_buffer.parse().unwrap_or(0);
                        let rows: u16 = rows_buffer.parse().unwrap_or(0);
                        editor.mode = EditorMode::Browse;
                        if cols > 0 && rows > 0 {
                            if let Some(dash) = self.dashboards.get_mut(dash_idx) {
                                if let Some(ed) = self.editor.as_mut() {
                                    ed.snapshot(dash);
                                    dash.grid.cols = cols;
                                    dash.grid.rows = rows;
                                    ed.dirty = true;
                                }
                            }
                        }
                    }
                    _ => {}
                }
                return;
            }
            EditorMode::EditingFilterQuery {
                instance,
                query_buffer,
                title_buffer,
                hide_state,
                focus,
            } => {
                use crate::dashboard::editor::FilterFocus as F;
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Tab => {
                        *focus = match *focus {
                            F::Query => F::Title,
                            F::Title => F::HideToggle,
                            F::HideToggle => F::Query,
                        };
                    }
                    KeyCode::Backspace => match *focus {
                        F::Query => {
                            query_buffer.pop();
                        }
                        F::Title => {
                            title_buffer.pop();
                        }
                        F::HideToggle => {}
                    },
                    KeyCode::Char(' ') if matches!(*focus, F::HideToggle) => {
                        *hide_state = !*hide_state;
                    }
                    KeyCode::Char(c) => match *focus {
                        F::Query => query_buffer.push(c),
                        F::Title => title_buffer.push(c),
                        F::HideToggle => {}
                    },
                    KeyCode::F(2) => {
                        let inst = instance.clone();
                        let q = query_buffer.trim().to_string();
                        let title = if title_buffer.trim().is_empty() {
                            None
                        } else {
                            Some(title_buffer.trim().to_string())
                        };
                        let hide = *hide_state;
                        editor.mode = EditorMode::Browse;
                        if q.is_empty() {
                            self.last_error = Some("query cannot be empty".into());
                            return;
                        }
                        if let Err(e) = crate::dashboard::query::EntityQuery::parse(&q) {
                            self.last_error = Some(format!("invalid query: {e}"));
                            return;
                        }
                        let kind = CardKind::FilteredEntityList {
                            instance: inst,
                            query: q,
                            hide_state: hide,
                            hide_when_empty: false,
                            title,
                        };
                        let Some(dash) = self.dashboards.get_mut(dash_idx) else {
                            return;
                        };
                        let Some(editor2) = self.editor.as_mut() else {
                            return;
                        };
                        editor2.snapshot(dash);
                        editor2.add_card(dash, kind);
                    }
                    _ => {}
                }
                return;
            }
            EditorMode::Menu {
                items, selected, ..
            } => {
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Up | KeyCode::Char('k') if *selected > 0 => {
                        *selected -= 1;
                    }
                    KeyCode::Down | KeyCode::Char('j') if *selected + 1 < items.len() => {
                        *selected += 1;
                    }
                    KeyCode::Enter => {
                        let Some(item) = items.get(*selected).cloned() else {
                            return;
                        };
                        let ctx = match &editor.mode {
                            EditorMode::Menu { context, .. } => *context,
                            _ => return,
                        };
                        // Drop the &mut editor borrow before dispatching.
                        editor.mode = EditorMode::Browse;
                        self.dispatch_menu_action(item.action, ctx);
                    }
                    _ => {}
                }
                return;
            }
            EditorMode::EditingWindow { card_idx, buffer } => {
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Backspace => {
                        buffer.pop();
                    }
                    KeyCode::Char(c) => buffer.push(c),
                    KeyCode::Enter => {
                        let idx = *card_idx;
                        let new_window = buffer.trim().to_string();
                        editor.mode = EditorMode::Browse;
                        if !new_window.is_empty() {
                            let mut entity_to_refetch: Option<(String, String, u32)> = None;
                            if let Some(dash) = self.dashboards.get_mut(dash_idx) {
                                if let Some(card) = dash.cards.get_mut(idx) {
                                    if let crate::dashboard::CardKind::Graph {
                                        instance,
                                        entities,
                                        entity,
                                        window,
                                        ..
                                    } = &mut card.kind
                                    {
                                        *window = new_window.clone();
                                        let eid = entities
                                            .first()
                                            .map(|s| s.entity.clone())
                                            .or_else(|| entity.clone());
                                        if let Some(eid) = eid {
                                            entity_to_refetch = Some((
                                                instance.clone(),
                                                eid,
                                                parse_window_hours(&new_window),
                                            ));
                                        }
                                    }
                                }
                            }
                            if let Some(ed) = self.editor.as_mut() {
                                ed.dirty = true;
                            }
                            if let Some((inst, eid, hours)) = entity_to_refetch {
                                let _ = self.instances.send(
                                    &inst,
                                    crate::ha::HaCommand::FetchHistory {
                                        entity_id: eid,
                                        hours,
                                    },
                                );
                            }
                        }
                    }
                    _ => {}
                }
                return;
            }
            EditorMode::RenamingCard { card_idx, buffer } => {
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Backspace => {
                        buffer.pop();
                    }
                    KeyCode::Char(c) => buffer.push(c),
                    KeyCode::Enter => {
                        let idx = *card_idx;
                        let new_title = if buffer.trim().is_empty() {
                            None
                        } else {
                            Some(buffer.trim().to_string())
                        };
                        editor.mode = EditorMode::Browse;
                        if let Some(dash) = self.dashboards.get_mut(dash_idx) {
                            if let Some(ed) = self.editor.as_mut() {
                                ed.snapshot(dash);
                                ed.retitle_card(dash, idx, new_title);
                            }
                        }
                    }
                    _ => {}
                }
                return;
            }
            EditorMode::EnterColorOverride { card_idx, buf } => {
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Backspace => {
                        buf.pop();
                    }
                    KeyCode::Char(c) => buf.push(c),
                    KeyCode::Enter => {
                        let idx = *card_idx;
                        let trimmed = buf.trim().to_string();
                        if trimmed.is_empty() {
                            // Clear override
                            editor.mode = EditorMode::Browse;
                            if let Some(dash) = self.dashboards.get_mut(dash_idx) {
                                if let Some(ed) = self.editor.as_mut() {
                                    ed.snapshot(dash);
                                }
                                if let Some(card) = self
                                    .dashboards
                                    .get_mut(dash_idx)
                                    .and_then(|d| d.cards.get_mut(idx))
                                {
                                    card.color = None;
                                }
                                if let Some(ed) = self.editor.as_mut() {
                                    ed.dirty = true;
                                }
                            }
                        } else if crate::ui::theme::parse_color(&trimmed).is_none() {
                            self.last_error = Some(format!(
                                "invalid color \"{trimmed}\" — use a named color or #rrggbb"
                            ));
                        } else {
                            editor.mode = EditorMode::Browse;
                            if let Some(dash) = self.dashboards.get_mut(dash_idx) {
                                if let Some(ed) = self.editor.as_mut() {
                                    ed.snapshot(dash);
                                }
                                if let Some(card) = self
                                    .dashboards
                                    .get_mut(dash_idx)
                                    .and_then(|d| d.cards.get_mut(idx))
                                {
                                    card.color = Some(trimmed);
                                }
                                if let Some(ed) = self.editor.as_mut() {
                                    ed.dirty = true;
                                }
                            }
                        }
                    }
                    _ => {}
                }
                return;
            }
            EditorMode::PickCardSize { card_idx, current } => {
                use crate::dashboard::CardSize;
                const SIZES: [CardSize; 3] = [CardSize::Small, CardSize::Normal, CardSize::Large];
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Up | KeyCode::Char('k') => {
                        let pos = SIZES.iter().position(|s| s == current).unwrap_or(1);
                        if pos > 0 {
                            *current = SIZES[pos - 1];
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        let pos = SIZES.iter().position(|s| s == current).unwrap_or(1);
                        if pos + 1 < SIZES.len() {
                            *current = SIZES[pos + 1];
                        }
                    }
                    KeyCode::Enter => {
                        let idx = *card_idx;
                        let chosen = *current;
                        editor.mode = EditorMode::Browse;
                        if let Some(dash) = self.dashboards.get_mut(dash_idx) {
                            if let Some(ed) = self.editor.as_mut() {
                                ed.snapshot(dash);
                            }
                            if let Some(card) = self
                                .dashboards
                                .get_mut(dash_idx)
                                .and_then(|d| d.cards.get_mut(idx))
                            {
                                card.size = chosen;
                            }
                            if let Some(ed) = self.editor.as_mut() {
                                ed.dirty = true;
                            }
                        }
                    }
                    _ => {}
                }
                return;
            }
            // ── Graph add-flow ────────────────────────────────────────────────────
            EditorMode::GraphPickType => {
                match k.code {
                    KeyCode::Char('1') => {
                        self.start_graph_after_type(crate::dashboard::GraphType::Line);
                    }
                    KeyCode::Char('2') => {
                        self.start_graph_after_type(crate::dashboard::GraphType::Bar);
                    }
                    KeyCode::Char('3') => {
                        self.start_graph_after_type(crate::dashboard::GraphType::Pie);
                    }
                    KeyCode::Esc => {
                        editor.mode = EditorMode::Browse;
                    }
                    _ => {}
                }
                return;
            }
            EditorMode::GraphPickInstance {
                graph_type,
                selected,
            } => {
                let aliases: Vec<String> = self.instances.runtimes.keys().cloned().collect();
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Up | KeyCode::Char('k') if *selected > 0 => *selected -= 1,
                    KeyCode::Down | KeyCode::Char('j') if *selected + 1 < aliases.len() => {
                        *selected += 1;
                    }
                    KeyCode::Enter => {
                        if let Some(inst) = aliases.get(*selected) {
                            let gt = *graph_type;
                            let inst = inst.clone();
                            editor.mode = EditorMode::GraphAddEntities {
                                instance: inst,
                                graph_type: gt,
                                accumulated: Vec::new(),
                                query: String::new(),
                                selected: 0,
                                asking_more: false,
                            };
                        }
                    }
                    KeyCode::Char(c) if c.is_ascii_digit() => {
                        let i = c.to_digit(10).unwrap() as usize;
                        if i >= 1 && i <= aliases.len() {
                            let gt = *graph_type;
                            let inst = aliases[i - 1].clone();
                            editor.mode = EditorMode::GraphAddEntities {
                                instance: inst,
                                graph_type: gt,
                                accumulated: Vec::new(),
                                query: String::new(),
                                selected: 0,
                                asking_more: false,
                            };
                        }
                    }
                    _ => {}
                }
                return;
            }
            EditorMode::GraphAddEntities {
                instance,
                graph_type,
                accumulated,
                query,
                selected,
                asking_more,
            } => {
                if *asking_more {
                    match k.code {
                        KeyCode::Char('y') | KeyCode::Char('Y') => {
                            *asking_more = false;
                            *query = String::new();
                            *selected = 0;
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                            if !accumulated.is_empty() {
                                let inst = instance.clone();
                                let gt = *graph_type;
                                let series = accumulated.clone();
                                self.advance_graph_to_config(inst, gt, series);
                            } else {
                                if let Some(ed) = self.editor.as_mut() {
                                    ed.mode = EditorMode::Browse;
                                }
                            }
                        }
                        _ => {}
                    }
                    return;
                }
                let rows = entity_search(&self.instances, instance, query);
                match k.code {
                    KeyCode::Esc => {
                        // Esc with at least one entity = done
                        if !accumulated.is_empty() {
                            let inst = instance.clone();
                            let gt = *graph_type;
                            let series = accumulated.clone();
                            self.advance_graph_to_config(inst, gt, series);
                        } else {
                            editor.mode = EditorMode::Browse;
                        }
                    }
                    KeyCode::Backspace => {
                        query.pop();
                        *selected = 0;
                    }
                    KeyCode::Up | KeyCode::Char('k') if *selected > 0 => *selected -= 1,
                    KeyCode::Down | KeyCode::Char('j') if *selected + 1 < rows.len() => {
                        *selected += 1;
                    }
                    KeyCode::Char(c) => {
                        query.push(c);
                        *selected = 0;
                    }
                    KeyCode::Enter => {
                        if let Some(pick) = rows.get(*selected) {
                            let eid = pick.entity_id.clone();
                            accumulated.push(crate::dashboard::GraphSeries {
                                entity: eid,
                                label: None,
                                color: None,
                            });
                            *asking_more = true;
                        }
                    }
                    _ => {}
                }
                return;
            }
            EditorMode::GraphEditWindowAdd {
                instance,
                graph_type,
                series,
                window_buf,
                title_buf,
                title_stage,
            } => {
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Backspace => {
                        if *title_stage {
                            title_buf.pop();
                        } else {
                            window_buf.pop();
                        }
                    }
                    KeyCode::Char(c) => {
                        if *title_stage {
                            title_buf.push(c);
                        } else {
                            window_buf.push(c);
                        }
                    }
                    KeyCode::Enter => {
                        if *title_stage {
                            let inst = instance.clone();
                            let gt = *graph_type;
                            let s = series.clone();
                            let w = if window_buf.trim().is_empty() {
                                "1h".to_string()
                            } else {
                                window_buf.trim().to_string()
                            };
                            let title = if title_buf.trim().is_empty() {
                                None
                            } else {
                                Some(title_buf.trim().to_string())
                            };
                            editor.mode = EditorMode::Browse;
                            let kind = crate::dashboard::CardKind::Graph {
                                instance: inst,
                                entity: None,
                                entities: s,
                                graph_type: gt,
                                window: w,
                                orientation: crate::dashboard::BarOrientation::default(),
                                title,
                            };
                            let Some(dash) = self.dashboards.get_mut(dash_idx) else {
                                return;
                            };
                            let Some(ed) = self.editor.as_mut() else {
                                return;
                            };
                            ed.snapshot(dash);
                            ed.add_card(dash, kind);
                        } else {
                            *title_stage = true;
                        }
                    }
                    _ => {}
                }
                return;
            }
            EditorMode::GraphPickOrientationAdd {
                instance,
                series,
                current,
                title_buf,
                title_stage,
            } => {
                const OPTS: [crate::dashboard::BarOrientation; 2] = [
                    crate::dashboard::BarOrientation::Vertical,
                    crate::dashboard::BarOrientation::Horizontal,
                ];
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Up | KeyCode::Char('k') if !*title_stage => {
                        let pos = OPTS.iter().position(|o| o == current).unwrap_or(0);
                        if pos > 0 {
                            *current = OPTS[pos - 1];
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') if !*title_stage => {
                        let pos = OPTS.iter().position(|o| o == current).unwrap_or(0);
                        if pos + 1 < OPTS.len() {
                            *current = OPTS[pos + 1];
                        }
                    }
                    KeyCode::Backspace => {
                        title_buf.pop();
                    }
                    KeyCode::Char(c) if *title_stage => {
                        title_buf.push(c);
                    }
                    KeyCode::Enter => {
                        if *title_stage {
                            let inst = instance.clone();
                            let s = series.clone();
                            let ori = *current;
                            let title = if title_buf.trim().is_empty() {
                                None
                            } else {
                                Some(title_buf.trim().to_string())
                            };
                            editor.mode = EditorMode::Browse;
                            let kind = crate::dashboard::CardKind::Graph {
                                instance: inst,
                                entity: None,
                                entities: s,
                                graph_type: crate::dashboard::GraphType::Bar,
                                window: "1h".to_string(),
                                orientation: ori,
                                title,
                            };
                            let Some(dash) = self.dashboards.get_mut(dash_idx) else {
                                return;
                            };
                            let Some(ed) = self.editor.as_mut() else {
                                return;
                            };
                            ed.snapshot(dash);
                            ed.add_card(dash, kind);
                        } else {
                            *title_stage = true;
                        }
                    }
                    _ => {}
                }
                return;
            }
            // ── Graph context-menu handlers ───────────────────────────────────────
            EditorMode::GraphAddOneSeries {
                card_idx,
                query,
                selected,
            } => {
                let instance = self
                    .dashboards
                    .get(dash_idx)
                    .and_then(|d| d.cards.get(*card_idx))
                    .and_then(|c| {
                        if let crate::dashboard::CardKind::Graph { instance, .. } = &c.kind {
                            Some(instance.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default();
                let rows = entity_search(&self.instances, &instance, query);
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Backspace => {
                        query.pop();
                        *selected = 0;
                    }
                    KeyCode::Up | KeyCode::Char('k') if *selected > 0 => *selected -= 1,
                    KeyCode::Down | KeyCode::Char('j') if *selected + 1 < rows.len() => {
                        *selected += 1
                    }
                    KeyCode::Char(c) => {
                        query.push(c);
                        *selected = 0;
                    }
                    KeyCode::Enter => {
                        if let Some(pick) = rows.get(*selected) {
                            let idx = *card_idx;
                            let eid = pick.entity_id.clone();
                            editor.mode = EditorMode::Browse;
                            if let Some(dash) = self.dashboards.get_mut(dash_idx) {
                                if let Some(ed) = self.editor.as_mut() {
                                    ed.snapshot(dash);
                                }
                                if let Some(card) = self
                                    .dashboards
                                    .get_mut(dash_idx)
                                    .and_then(|d| d.cards.get_mut(idx))
                                {
                                    if let crate::dashboard::CardKind::Graph { entities, .. } =
                                        &mut card.kind
                                    {
                                        entities.push(crate::dashboard::GraphSeries {
                                            entity: eid,
                                            label: None,
                                            color: None,
                                        });
                                    }
                                }
                                if let Some(ed) = self.editor.as_mut() {
                                    ed.dirty = true;
                                }
                            }
                        }
                    }
                    _ => {}
                }
                return;
            }
            EditorMode::GraphPickSeriesIndex {
                card_idx,
                op,
                selected,
            } => {
                let count = self
                    .dashboards
                    .get(dash_idx)
                    .and_then(|d| d.cards.get(*card_idx))
                    .map(|c| {
                        if let crate::dashboard::CardKind::Graph { entities, .. } = &c.kind {
                            entities.len()
                        } else {
                            0
                        }
                    })
                    .unwrap_or(0);
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Up | KeyCode::Char('k') if *selected > 0 => *selected -= 1,
                    KeyCode::Down | KeyCode::Char('j') if *selected + 1 < count => *selected += 1,
                    KeyCode::Enter => {
                        let idx = *card_idx;
                        let sidx = *selected;
                        let op = *op;
                        match op {
                            crate::dashboard::editor::SeriesIndexOp::Remove => {
                                editor.mode = EditorMode::Browse;
                                if let Some(dash) = self.dashboards.get_mut(dash_idx) {
                                    if let Some(ed) = self.editor.as_mut() {
                                        ed.snapshot(dash);
                                    }
                                    if let Some(card) = self
                                        .dashboards
                                        .get_mut(dash_idx)
                                        .and_then(|d| d.cards.get_mut(idx))
                                    {
                                        if let crate::dashboard::CardKind::Graph {
                                            entities, ..
                                        } = &mut card.kind
                                        {
                                            if sidx < entities.len() {
                                                entities.remove(sidx);
                                            }
                                        }
                                    }
                                    if let Some(ed) = self.editor.as_mut() {
                                        ed.dirty = true;
                                    }
                                }
                            }
                            crate::dashboard::editor::SeriesIndexOp::SetColor => {
                                let cur = self
                                    .dashboards
                                    .get(dash_idx)
                                    .and_then(|d| d.cards.get(idx))
                                    .and_then(|c| {
                                        if let crate::dashboard::CardKind::Graph {
                                            entities, ..
                                        } = &c.kind
                                        {
                                            entities.get(sidx).and_then(|s| s.color.clone())
                                        } else {
                                            None
                                        }
                                    })
                                    .unwrap_or_default();
                                editor.mode = EditorMode::GraphEditSeriesColor {
                                    card_idx: idx,
                                    series_idx: sidx,
                                    buf: cur,
                                };
                            }
                            crate::dashboard::editor::SeriesIndexOp::SetLabel => {
                                let cur = self
                                    .dashboards
                                    .get(dash_idx)
                                    .and_then(|d| d.cards.get(idx))
                                    .and_then(|c| {
                                        if let crate::dashboard::CardKind::Graph {
                                            entities, ..
                                        } = &c.kind
                                        {
                                            entities.get(sidx).and_then(|s| s.label.clone())
                                        } else {
                                            None
                                        }
                                    })
                                    .unwrap_or_default();
                                editor.mode = EditorMode::GraphEditSeriesLabel {
                                    card_idx: idx,
                                    series_idx: sidx,
                                    buf: cur,
                                };
                            }
                        }
                    }
                    _ => {}
                }
                return;
            }
            EditorMode::GraphEditSeriesColor {
                card_idx,
                series_idx,
                buf,
            } => {
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Backspace => {
                        buf.pop();
                    }
                    KeyCode::Char(c) => buf.push(c),
                    KeyCode::Enter => {
                        let idx = *card_idx;
                        let sidx = *series_idx;
                        let trimmed = buf.trim().to_string();
                        if !trimmed.is_empty() && crate::ui::theme::parse_color(&trimmed).is_none()
                        {
                            self.last_error = Some(format!(
                                "invalid color \"{trimmed}\" — use a named color or #rrggbb"
                            ));
                            return;
                        }
                        editor.mode = EditorMode::Browse;
                        let color_val = if trimmed.is_empty() {
                            None
                        } else {
                            Some(trimmed)
                        };
                        if let Some(dash) = self.dashboards.get_mut(dash_idx) {
                            if let Some(ed) = self.editor.as_mut() {
                                ed.snapshot(dash);
                            }
                            if let Some(card) = self
                                .dashboards
                                .get_mut(dash_idx)
                                .and_then(|d| d.cards.get_mut(idx))
                            {
                                if let crate::dashboard::CardKind::Graph { entities, .. } =
                                    &mut card.kind
                                {
                                    if let Some(s) = entities.get_mut(sidx) {
                                        s.color = color_val;
                                    }
                                }
                            }
                            if let Some(ed) = self.editor.as_mut() {
                                ed.dirty = true;
                            }
                        }
                    }
                    _ => {}
                }
                return;
            }
            EditorMode::GraphEditSeriesLabel {
                card_idx,
                series_idx,
                buf,
            } => {
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Backspace => {
                        buf.pop();
                    }
                    KeyCode::Char(c) => buf.push(c),
                    KeyCode::Enter => {
                        let idx = *card_idx;
                        let sidx = *series_idx;
                        let trimmed = buf.trim().to_string();
                        editor.mode = EditorMode::Browse;
                        let label_val = if trimmed.is_empty() {
                            None
                        } else {
                            Some(trimmed)
                        };
                        if let Some(dash) = self.dashboards.get_mut(dash_idx) {
                            if let Some(ed) = self.editor.as_mut() {
                                ed.snapshot(dash);
                            }
                            if let Some(card) = self
                                .dashboards
                                .get_mut(dash_idx)
                                .and_then(|d| d.cards.get_mut(idx))
                            {
                                if let crate::dashboard::CardKind::Graph { entities, .. } =
                                    &mut card.kind
                                {
                                    if let Some(s) = entities.get_mut(sidx) {
                                        s.label = label_val;
                                    }
                                }
                            }
                            if let Some(ed) = self.editor.as_mut() {
                                ed.dirty = true;
                            }
                        }
                    }
                    _ => {}
                }
                return;
            }
            EditorMode::GraphEditWindow { card_idx, buf } => {
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Backspace => {
                        buf.pop();
                    }
                    KeyCode::Char(c) => buf.push(c),
                    KeyCode::Enter => {
                        let idx = *card_idx;
                        let new_window = buf.trim().to_string();
                        editor.mode = EditorMode::Browse;
                        if !new_window.is_empty() {
                            if let Some(dash) = self.dashboards.get_mut(dash_idx) {
                                if let Some(ed) = self.editor.as_mut() {
                                    ed.snapshot(dash);
                                }
                                if let Some(card) = self
                                    .dashboards
                                    .get_mut(dash_idx)
                                    .and_then(|d| d.cards.get_mut(idx))
                                {
                                    if let crate::dashboard::CardKind::Graph { window, .. } =
                                        &mut card.kind
                                    {
                                        *window = new_window;
                                    }
                                }
                                if let Some(ed) = self.editor.as_mut() {
                                    ed.dirty = true;
                                }
                            }
                        }
                    }
                    _ => {}
                }
                return;
            }
            EditorMode::GraphPickOrientation { card_idx, current } => {
                use crate::dashboard::BarOrientation;
                const OPTS: [BarOrientation; 2] =
                    [BarOrientation::Vertical, BarOrientation::Horizontal];
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Up | KeyCode::Char('k') => {
                        let pos = OPTS.iter().position(|o| o == current).unwrap_or(0);
                        if pos > 0 {
                            *current = OPTS[pos - 1];
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        let pos = OPTS.iter().position(|o| o == current).unwrap_or(0);
                        if pos + 1 < OPTS.len() {
                            *current = OPTS[pos + 1];
                        }
                    }
                    KeyCode::Enter => {
                        let idx = *card_idx;
                        let ori = *current;
                        editor.mode = EditorMode::Browse;
                        if let Some(dash) = self.dashboards.get_mut(dash_idx) {
                            if let Some(ed) = self.editor.as_mut() {
                                ed.snapshot(dash);
                            }
                            if let Some(card) = self
                                .dashboards
                                .get_mut(dash_idx)
                                .and_then(|d| d.cards.get_mut(idx))
                            {
                                if let crate::dashboard::CardKind::Graph { orientation, .. } =
                                    &mut card.kind
                                {
                                    *orientation = ori;
                                }
                            }
                            if let Some(ed) = self.editor.as_mut() {
                                ed.dirty = true;
                            }
                        }
                    }
                    _ => {}
                }
                return;
            }
            // ── Gauge severity flow ──────────────────────────────────────────
            EditorMode::EditSeverityGreen {
                card_idx,
                buf,
                accum,
            } => {
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Backspace => {
                        buf.pop();
                    }
                    KeyCode::Char(c) => buf.push(c),
                    KeyCode::Enter => match buf.trim().parse::<f64>() {
                        Ok(v) => {
                            let idx = *card_idx;
                            let new_accum = crate::dashboard::editor::SeverityAccum {
                                green: v,
                                yellow: accum.yellow,
                            };
                            let cur_sev = self
                                .dashboards
                                .get(dash_idx)
                                .and_then(|d| d.cards.get(idx))
                                .and_then(|c| {
                                    if let CardKind::Gauge { severity, .. } = &c.kind {
                                        severity.as_ref().map(|s| s.yellow.to_string())
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or_default();
                            editor.mode = EditorMode::EditSeverityYellow {
                                card_idx: idx,
                                buf: cur_sev,
                                accum: new_accum,
                            };
                        }
                        Err(_) => {
                            self.last_error = Some("invalid number — enter a numeric value".into());
                        }
                    },
                    _ => {}
                }
                return;
            }
            EditorMode::EditSeverityYellow {
                card_idx,
                buf,
                accum,
            } => {
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Backspace => {
                        buf.pop();
                    }
                    KeyCode::Char(c) => buf.push(c),
                    KeyCode::Enter => match buf.trim().parse::<f64>() {
                        Ok(v) => {
                            let idx = *card_idx;
                            let new_accum = crate::dashboard::editor::SeverityAccum {
                                green: accum.green,
                                yellow: v,
                            };
                            let cur_red = self
                                .dashboards
                                .get(dash_idx)
                                .and_then(|d| d.cards.get(idx))
                                .and_then(|c| {
                                    if let CardKind::Gauge { severity, .. } = &c.kind {
                                        severity.as_ref().map(|s| s.red.to_string())
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or_default();
                            editor.mode = EditorMode::EditSeverityRed {
                                card_idx: idx,
                                buf: cur_red,
                                accum: new_accum,
                            };
                        }
                        Err(_) => {
                            self.last_error = Some("invalid number — enter a numeric value".into());
                        }
                    },
                    _ => {}
                }
                return;
            }
            EditorMode::EditSeverityRed {
                card_idx,
                buf,
                accum,
            } => {
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Backspace => {
                        buf.pop();
                    }
                    KeyCode::Char(c) => buf.push(c),
                    KeyCode::Enter => match buf.trim().parse::<f64>() {
                        Ok(red) => {
                            let idx = *card_idx;
                            let new_sev = crate::dashboard::Severity {
                                green: accum.green,
                                yellow: accum.yellow,
                                red,
                            };
                            editor.mode = EditorMode::Browse;
                            if let Some(dash) = self.dashboards.get_mut(dash_idx) {
                                if let Some(ed) = self.editor.as_mut() {
                                    ed.snapshot(dash);
                                }
                                if let Some(card) = self
                                    .dashboards
                                    .get_mut(dash_idx)
                                    .and_then(|d| d.cards.get_mut(idx))
                                {
                                    if let CardKind::Gauge { severity, .. } = &mut card.kind {
                                        *severity = Some(new_sev);
                                    }
                                }
                                if let Some(ed) = self.editor.as_mut() {
                                    ed.dirty = true;
                                }
                            }
                        }
                        Err(_) => {
                            self.last_error = Some("invalid number — enter a numeric value".into());
                        }
                    },
                    _ => {}
                }
                return;
            }
            // ---- Statistics add-flow ----
            EditorMode::StatsPickMetric {
                instance,
                entity,
                friendly_name,
                selected,
            } => {
                const METRICS: [crate::dashboard::StatsMetric; 5] = [
                    crate::dashboard::StatsMetric::Avg,
                    crate::dashboard::StatsMetric::Min,
                    crate::dashboard::StatsMetric::Max,
                    crate::dashboard::StatsMetric::Sum,
                    crate::dashboard::StatsMetric::Count,
                ];
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Up if *selected > 0 => *selected -= 1,
                    KeyCode::Down if *selected + 1 < METRICS.len() => *selected += 1,
                    KeyCode::Char('1') => *selected = 0,
                    KeyCode::Char('2') => *selected = 1,
                    KeyCode::Char('3') => *selected = 2,
                    KeyCode::Char('4') => *selected = 3,
                    KeyCode::Char('5') => *selected = 4,
                    KeyCode::Enter => {
                        let metric = METRICS[*selected];
                        let inst = instance.clone();
                        let ent = entity.clone();
                        let fname = friendly_name.clone();
                        editor.mode = EditorMode::StatsEditWindowAdd {
                            instance: inst,
                            entity: ent,
                            friendly_name: fname,
                            metric,
                            buf: "1h".into(),
                        };
                    }
                    _ => {}
                }
                return;
            }
            EditorMode::StatsEditWindowAdd {
                instance,
                entity,
                friendly_name,
                metric,
                buf,
            } => {
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Backspace => {
                        buf.pop();
                    }
                    KeyCode::Char(c) => buf.push(c),
                    KeyCode::Enter => {
                        let window = {
                            let s = buf.trim().to_string();
                            if s.is_empty() {
                                "1h".into()
                            } else {
                                s
                            }
                        };
                        let inst = instance.clone();
                        let ent = entity.clone();
                        let fname = friendly_name.clone();
                        let m = *metric;
                        editor.mode = EditorMode::StatsEditUnitAdd {
                            instance: inst,
                            entity: ent,
                            friendly_name: fname,
                            metric: m,
                            window,
                            buf: String::new(),
                        };
                    }
                    _ => {}
                }
                return;
            }
            EditorMode::StatsEditUnitAdd {
                instance,
                entity,
                friendly_name,
                metric,
                window,
                buf,
            } => {
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Backspace => {
                        buf.pop();
                    }
                    KeyCode::Char(c) => buf.push(c),
                    KeyCode::Enter => {
                        let unit_raw = buf.trim().to_string();
                        let unit = if unit_raw.is_empty() {
                            None
                        } else {
                            Some(unit_raw)
                        };
                        let inst = instance.clone();
                        let ent = entity.clone();
                        let fname = friendly_name.clone();
                        let m = *metric;
                        let win = window.clone();
                        editor.mode = EditorMode::StatsEditTitleAdd {
                            instance: inst,
                            entity: ent,
                            friendly_name: fname,
                            metric: m,
                            window: win,
                            unit,
                            buf: String::new(),
                        };
                    }
                    _ => {}
                }
                return;
            }
            EditorMode::StatsEditTitleAdd {
                instance,
                entity,
                friendly_name,
                metric,
                window,
                unit,
                buf,
            } => {
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Backspace => {
                        buf.pop();
                    }
                    KeyCode::Char(c) => buf.push(c),
                    KeyCode::Enter => {
                        let title_raw = buf.trim().to_string();
                        let title = if title_raw.is_empty() {
                            if friendly_name.is_empty() {
                                None
                            } else {
                                Some(friendly_name.clone())
                            }
                        } else {
                            Some(title_raw)
                        };
                        let kind = CardKind::Statistics {
                            instance: instance.clone(),
                            entity: entity.clone(),
                            window: window.clone(),
                            metric: *metric,
                            unit: unit.clone(),
                            title,
                        };
                        editor.mode = EditorMode::Browse;
                        if let Some(dash) = self.dashboards.get_mut(dash_idx) {
                            if let Some(ed) = self.editor.as_mut() {
                                ed.snapshot(dash);
                                ed.add_card(dash, kind);
                            }
                        }
                    }
                    _ => {}
                }
                return;
            }
            // ---- Statistics context-menu flows ----
            EditorMode::StatsEditMetric { card_idx, selected } => {
                const METRICS: [crate::dashboard::StatsMetric; 5] = [
                    crate::dashboard::StatsMetric::Avg,
                    crate::dashboard::StatsMetric::Min,
                    crate::dashboard::StatsMetric::Max,
                    crate::dashboard::StatsMetric::Sum,
                    crate::dashboard::StatsMetric::Count,
                ];
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Up if *selected > 0 => *selected -= 1,
                    KeyCode::Down if *selected + 1 < METRICS.len() => *selected += 1,
                    KeyCode::Char('1') => *selected = 0,
                    KeyCode::Char('2') => *selected = 1,
                    KeyCode::Char('3') => *selected = 2,
                    KeyCode::Char('4') => *selected = 3,
                    KeyCode::Char('5') => *selected = 4,
                    KeyCode::Enter => {
                        let idx = *card_idx;
                        let new_metric = METRICS[*selected];
                        editor.mode = EditorMode::Browse;
                        if let Some(dash) = self.dashboards.get_mut(dash_idx) {
                            if let Some(ed) = self.editor.as_mut() {
                                ed.snapshot(dash);
                            }
                            if let Some(card) = self
                                .dashboards
                                .get_mut(dash_idx)
                                .and_then(|d| d.cards.get_mut(idx))
                            {
                                if let CardKind::Statistics { metric, .. } = &mut card.kind {
                                    *metric = new_metric;
                                }
                            }
                            if let Some(ed) = self.editor.as_mut() {
                                ed.dirty = true;
                            }
                        }
                    }
                    _ => {}
                }
                return;
            }
            EditorMode::StatsEditWindow { card_idx, buf } => {
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Backspace => {
                        buf.pop();
                    }
                    KeyCode::Char(c) => buf.push(c),
                    KeyCode::Enter => {
                        let idx = *card_idx;
                        let new_window = {
                            let s = buf.trim().to_string();
                            if s.is_empty() {
                                "1h".into()
                            } else {
                                s
                            }
                        };
                        editor.mode = EditorMode::Browse;
                        if let Some(dash) = self.dashboards.get_mut(dash_idx) {
                            if let Some(ed) = self.editor.as_mut() {
                                ed.snapshot(dash);
                            }
                            if let Some(card) = self
                                .dashboards
                                .get_mut(dash_idx)
                                .and_then(|d| d.cards.get_mut(idx))
                            {
                                if let CardKind::Statistics { window, .. } = &mut card.kind {
                                    *window = new_window;
                                }
                            }
                            if let Some(ed) = self.editor.as_mut() {
                                ed.dirty = true;
                            }
                        }
                    }
                    _ => {}
                }
                return;
            }
            EditorMode::StatsEditUnit { card_idx, buf } => {
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Backspace => {
                        buf.pop();
                    }
                    KeyCode::Char(c) => buf.push(c),
                    KeyCode::Enter => {
                        let idx = *card_idx;
                        let unit_raw = buf.trim().to_string();
                        let new_unit = if unit_raw.is_empty() {
                            None
                        } else {
                            Some(unit_raw)
                        };
                        editor.mode = EditorMode::Browse;
                        if let Some(dash) = self.dashboards.get_mut(dash_idx) {
                            if let Some(ed) = self.editor.as_mut() {
                                ed.snapshot(dash);
                            }
                            if let Some(card) = self
                                .dashboards
                                .get_mut(dash_idx)
                                .and_then(|d| d.cards.get_mut(idx))
                            {
                                if let CardKind::Statistics { unit, .. } = &mut card.kind {
                                    *unit = new_unit;
                                }
                            }
                            if let Some(ed) = self.editor.as_mut() {
                                ed.dirty = true;
                            }
                        }
                    }
                    _ => {}
                }
                return;
            }
            // ---- Clock add-flow ----
            EditorMode::ClockAddTitle { title_buffer } => {
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Backspace => {
                        title_buffer.pop();
                    }
                    KeyCode::Char(c) => title_buffer.push(c),
                    KeyCode::Enter => {
                        let title = {
                            let t = title_buffer.trim().to_string();
                            if t.is_empty() {
                                None
                            } else {
                                Some(t)
                            }
                        };
                        editor.mode = EditorMode::ClockAddFormat {
                            title,
                            format_buffer: "%H:%M:%S".into(),
                        };
                    }
                    _ => {}
                }
                return;
            }
            EditorMode::ClockAddFormat {
                title,
                format_buffer,
            } => {
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Backspace => {
                        format_buffer.pop();
                    }
                    KeyCode::Char(c) => format_buffer.push(c),
                    KeyCode::Enter => {
                        let fmt = format_buffer.trim().to_string();
                        let fmt = if fmt.is_empty() {
                            "%H:%M:%S".into()
                        } else {
                            fmt
                        };
                        editor.mode = EditorMode::ClockAddTimezone {
                            title: title.clone(),
                            format: fmt,
                            tz_buffer: String::new(),
                        };
                    }
                    _ => {}
                }
                return;
            }
            EditorMode::ClockAddTimezone {
                title,
                format,
                tz_buffer,
            } => {
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Backspace => {
                        tz_buffer.pop();
                    }
                    KeyCode::Char(c) => tz_buffer.push(c),
                    KeyCode::Enter => {
                        let tz_raw = tz_buffer.trim().to_string();
                        let timezone = if tz_raw.is_empty() {
                            None
                        } else {
                            Some(tz_raw)
                        };
                        let kind = CardKind::Clock {
                            format: format.clone(),
                            timezone,
                            title: title.clone(),
                        };
                        editor.mode = EditorMode::Browse;
                        if let Some(dash) = self.dashboards.get_mut(dash_idx) {
                            if let Some(ed) = self.editor.as_mut() {
                                ed.snapshot(dash);
                                ed.add_card(dash, kind);
                            }
                        }
                    }
                    _ => {}
                }
                return;
            }
            // ---- Clock context-menu flows ----
            EditorMode::ClockEditFormat { card_idx, buf } => {
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Backspace => {
                        buf.pop();
                    }
                    KeyCode::Char(c) => buf.push(c),
                    KeyCode::Enter => {
                        let idx = *card_idx;
                        let new_fmt = {
                            let s = buf.trim().to_string();
                            if s.is_empty() {
                                "%H:%M:%S".into()
                            } else {
                                s
                            }
                        };
                        editor.mode = EditorMode::Browse;
                        if let Some(dash) = self.dashboards.get_mut(dash_idx) {
                            if let Some(ed) = self.editor.as_mut() {
                                ed.snapshot(dash);
                            }
                            if let Some(card) = self
                                .dashboards
                                .get_mut(dash_idx)
                                .and_then(|d| d.cards.get_mut(idx))
                            {
                                if let CardKind::Clock { format, .. } = &mut card.kind {
                                    *format = new_fmt;
                                }
                            }
                            if let Some(ed) = self.editor.as_mut() {
                                ed.dirty = true;
                            }
                        }
                    }
                    _ => {}
                }
                return;
            }
            EditorMode::ClockEditTimezone { card_idx, buf } => {
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Backspace => {
                        buf.pop();
                    }
                    KeyCode::Char(c) => buf.push(c),
                    KeyCode::Enter => {
                        let idx = *card_idx;
                        let tz_raw = buf.trim().to_string();
                        let new_tz = if tz_raw.is_empty() {
                            None
                        } else {
                            Some(tz_raw)
                        };
                        editor.mode = EditorMode::Browse;
                        if let Some(dash) = self.dashboards.get_mut(dash_idx) {
                            if let Some(ed) = self.editor.as_mut() {
                                ed.snapshot(dash);
                            }
                            if let Some(card) = self
                                .dashboards
                                .get_mut(dash_idx)
                                .and_then(|d| d.cards.get_mut(idx))
                            {
                                if let CardKind::Clock { timezone, .. } = &mut card.kind {
                                    *timezone = new_tz;
                                }
                            }
                            if let Some(ed) = self.editor.as_mut() {
                                ed.dirty = true;
                            }
                        }
                    }
                    _ => {}
                }
                return;
            }
            // ---- Image add-flow ----
            EditorMode::ImagePickSourceKind { selected } => {
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Char('1') => {
                        *selected = 0;
                        // image entity — go directly to instance picker
                        editor.mode = EditorMode::PickingInstance {
                            card_type: CardTypeStub::Image,
                            selected: 0,
                        };
                    }
                    KeyCode::Char('2') => {
                        *selected = 1;
                        // camera — go to instance picker
                        editor.mode = EditorMode::PickingInstance {
                            card_type: CardTypeStub::Image,
                            selected: 0,
                        };
                    }
                    _ => {}
                }
                return;
            }
            EditorMode::ImageEditRefreshSeconds {
                instance,
                entity,
                friendly_name,
                is_camera,
                buf,
            } => {
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Backspace => {
                        buf.pop();
                    }
                    KeyCode::Char(c) if c.is_ascii_digit() => buf.push(c),
                    KeyCode::Enter => {
                        let secs_raw = buf.trim().to_string();
                        let refresh_seconds = secs_raw.parse::<u32>().ok().filter(|&s| s > 0);
                        editor.mode = EditorMode::ImageEditTitleAdd {
                            instance: instance.clone(),
                            entity: entity.clone(),
                            friendly_name: friendly_name.clone(),
                            is_camera: *is_camera,
                            refresh_seconds,
                            buf: String::new(),
                        };
                    }
                    _ => {}
                }
                return;
            }
            EditorMode::ImageEditTitleAdd {
                instance,
                entity,
                friendly_name,
                is_camera,
                refresh_seconds,
                buf,
            } => {
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Backspace => {
                        buf.pop();
                    }
                    KeyCode::Char(c) => buf.push(c),
                    KeyCode::Enter => {
                        let title_raw = buf.trim().to_string();
                        let title = if title_raw.is_empty() {
                            if friendly_name.is_empty() {
                                None
                            } else {
                                Some(friendly_name.clone())
                            }
                        } else {
                            Some(title_raw)
                        };
                        let source = if *is_camera {
                            crate::dashboard::ImageSource::Camera {
                                entity: entity.clone(),
                            }
                        } else {
                            crate::dashboard::ImageSource::ImageEntity {
                                entity: entity.clone(),
                            }
                        };
                        let kind = CardKind::Image {
                            instance: instance.clone(),
                            source,
                            refresh_seconds: *refresh_seconds,
                            title,
                        };
                        editor.mode = EditorMode::Browse;
                        if let Some(dash) = self.dashboards.get_mut(dash_idx) {
                            if let Some(ed) = self.editor.as_mut() {
                                ed.snapshot(dash);
                                ed.add_card(dash, kind);
                            }
                        }
                    }
                    _ => {}
                }
                return;
            }
            EditorMode::Browse => {}
        }

        // Browse mode keys
        let Some(dash) = self.dashboards.get_mut(dash_idx) else {
            return;
        };
        match k.code {
            KeyCode::Esc => {
                if editor.dirty {
                    editor.mode = EditorMode::ConfirmExit;
                } else {
                    self.editor = None;
                    self.screen = Screen::Dashboard {
                        idx: dash_idx,
                        selected_card: 0,
                        sub_index: 0,
                    };
                }
            }
            KeyCode::Char('h') | KeyCode::Left => editor.move_cursor(-1, 0, dash),
            KeyCode::Char('l') | KeyCode::Right => editor.move_cursor(1, 0, dash),
            KeyCode::Char('k') | KeyCode::Up => editor.move_cursor(0, -1, dash),
            KeyCode::Char('j') | KeyCode::Down => editor.move_cursor(0, 1, dash),
            KeyCode::Char('H') => {
                editor.snapshot(dash);
                editor.resize_selected(-1, 0, dash);
            }
            KeyCode::Char('L') => {
                editor.snapshot(dash);
                editor.resize_selected(1, 0, dash);
            }
            KeyCode::Char('K') => {
                editor.snapshot(dash);
                editor.resize_selected(0, -1, dash);
            }
            KeyCode::Char('J') => {
                editor.snapshot(dash);
                editor.resize_selected(0, 1, dash);
            }
            KeyCode::Enter => {
                if editor.selected_card.is_some() {
                    editor.snapshot(dash);
                    editor.move_selected_to_cursor(dash);
                    editor.selected_card = None;
                } else {
                    editor.select_at_cursor(dash);
                }
            }
            KeyCode::Char(' ') => editor.select_at_cursor(dash),
            KeyCode::Char('a') => editor.mode = EditorMode::PickingType,
            KeyCode::Char('m') | KeyCode::Char('M') => {
                self.open_menu();
            }
            KeyCode::Char('d') if editor.selected_card.is_some() => {
                editor.mode = EditorMode::ConfirmDelete;
            }
            KeyCode::Char('u') => {
                editor.undo(dash);
            }
            KeyCode::Char('s') => {
                let path = editor
                    .source_path
                    .clone()
                    .or_else(|| self.dashboards_path.clone())
                    .or_else(crate::dashboard::persist::default_path);
                if let Some(p) = path {
                    let file = crate::dashboard::DashboardFile {
                        dashboards: self.dashboards.clone(),
                    };
                    match crate::dashboard::persist::save(&file, &p) {
                        Ok(()) => {
                            if let Some(ed) = self.editor.as_mut() {
                                ed.dirty = false;
                            }
                            self.last_error = None;
                            self.status_msg = Some(format!("saved {}", p.display()));
                        }
                        Err(e) => self.last_error = Some(format!("save failed: {e}")),
                    }
                } else {
                    self.last_error = Some("no dashboards path".into());
                }
            }
            _ => {}
        }
    }

    fn enter_editor(&mut self) {
        let Screen::Dashboard { idx, .. } = self.screen else {
            return;
        };
        if self.dashboards.get(idx).is_none() {
            return;
        }
        let path = self
            .dashboards_path
            .clone()
            .or_else(crate::dashboard::persist::default_path);
        self.editor = Some(EditorState::new(idx, path));
        self.screen = Screen::Editor;
    }

    fn open_menu(&mut self) {
        let Some(editor) = self.editor.as_mut() else {
            return;
        };
        let dash_idx = editor.dash_idx;
        let selected = editor.selected_card;
        let Some(dash) = self.dashboards.get(dash_idx) else {
            return;
        };
        let (context, items) = match selected.and_then(|i| dash.cards.get(i).map(|c| (i, c))) {
            Some((idx, card)) => (
                crate::dashboard::editor::MenuContext::Card(idx),
                crate::dashboard::editor::card_menu_items(card),
            ),
            None => (
                crate::dashboard::editor::MenuContext::Dashboard,
                crate::dashboard::editor::dashboard_menu_items(),
            ),
        };
        if let Some(ed) = self.editor.as_mut() {
            ed.mode = EditorMode::Menu {
                context,
                items,
                selected: 0,
            };
        }
    }

    fn dispatch_menu_action(
        &mut self,
        action: crate::dashboard::editor::MenuAction,
        context: crate::dashboard::editor::MenuContext,
    ) {
        use crate::dashboard::editor::{MenuAction as A, MenuContext as C};
        let editor_dash_idx = self.editor.as_ref().map(|e| e.dash_idx);
        let Some(dash_idx) = editor_dash_idx else {
            return;
        };
        match (action, context) {
            (A::RenameCard, C::Card(idx)) => {
                let current = self
                    .dashboards
                    .get(dash_idx)
                    .and_then(|d| d.cards.get(idx))
                    .and_then(|c| match &c.kind {
                        crate::dashboard::CardKind::Entity { title, .. }
                        | crate::dashboard::CardKind::Toggle { title, .. }
                        | crate::dashboard::CardKind::Gauge { title, .. }
                        | crate::dashboard::CardKind::Graph { title, .. }
                        | crate::dashboard::CardKind::Text { title, .. }
                        | crate::dashboard::CardKind::EntityList { title, .. }
                        | crate::dashboard::CardKind::FilteredEntityList { title, .. }
                        | crate::dashboard::CardKind::Clock { title, .. }
                        | crate::dashboard::CardKind::Statistics { title, .. }
                        | crate::dashboard::CardKind::MediaPlayer { title, .. }
                        | crate::dashboard::CardKind::Image { title, .. }
                        | crate::dashboard::CardKind::Weather { title, .. } => title.clone(),
                    })
                    .unwrap_or_default();
                if let Some(ed) = self.editor.as_mut() {
                    ed.selected_card = Some(idx);
                    ed.mode = EditorMode::RenamingCard {
                        card_idx: idx,
                        buffer: current,
                    };
                }
            }
            (A::ChangeEntity, C::Card(idx)) => {
                if let Some(ed) = self.editor.as_mut() {
                    ed.selected_card = Some(idx);
                }
                self.change_card_entity(idx);
            }
            (A::EditWindow, C::Card(idx)) => {
                let current = self
                    .dashboards
                    .get(dash_idx)
                    .and_then(|d| d.cards.get(idx))
                    .and_then(|c| match &c.kind {
                        crate::dashboard::CardKind::Graph { window, .. } => Some(window.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| "1h".to_string());
                if let Some(ed) = self.editor.as_mut() {
                    ed.selected_card = Some(idx);
                    ed.mode = EditorMode::EditingWindow {
                        card_idx: idx,
                        buffer: current,
                    };
                }
            }
            (A::SetColorOverride, C::Card(idx)) => {
                let current_color = self
                    .dashboards
                    .get(dash_idx)
                    .and_then(|d| d.cards.get(idx))
                    .and_then(|c| c.color.clone())
                    .unwrap_or_default();
                if let Some(ed) = self.editor.as_mut() {
                    ed.selected_card = Some(idx);
                    ed.mode = EditorMode::EnterColorOverride {
                        card_idx: idx,
                        buf: current_color,
                    };
                }
            }
            (A::SetCardSize, C::Card(idx)) => {
                let current_size = self
                    .dashboards
                    .get(dash_idx)
                    .and_then(|d| d.cards.get(idx))
                    .map(|c| c.size)
                    .unwrap_or_default();
                if let Some(ed) = self.editor.as_mut() {
                    ed.selected_card = Some(idx);
                    ed.mode = EditorMode::PickCardSize {
                        card_idx: idx,
                        current: current_size,
                    };
                }
            }
            (A::DeleteCard, C::Card(idx)) => {
                if let Some(ed) = self.editor.as_mut() {
                    ed.selected_card = Some(idx);
                    ed.mode = EditorMode::ConfirmDelete;
                }
            }
            (A::EditQuery, C::Card(idx)) => {
                let mut existing: Option<(String, String, bool, String)> = None;
                if let Some(card) = self.dashboards.get(dash_idx).and_then(|d| d.cards.get(idx)) {
                    if let CardKind::FilteredEntityList {
                        instance,
                        query,
                        hide_state,
                        title,
                        ..
                    } = &card.kind
                    {
                        existing = Some((
                            instance.clone(),
                            query.clone(),
                            *hide_state,
                            title.clone().unwrap_or_default(),
                        ));
                    }
                }
                if let Some((inst, q, hs, t)) = existing {
                    if let Some(ed) = self.editor.as_mut() {
                        ed.selected_card = Some(idx);
                        ed.edit_target = Some(idx);
                        ed.mode = EditorMode::EditingFilterQuery {
                            instance: inst,
                            query_buffer: q,
                            title_buffer: t,
                            hide_state: hs,
                            focus: crate::dashboard::editor::FilterFocus::Query,
                        };
                    }
                }
            }
            (A::ToggleHideState, C::Card(idx)) => {
                if let Some(dash) = self.dashboards.get_mut(dash_idx) {
                    if let Some(ed) = self.editor.as_mut() {
                        ed.snapshot(dash);
                    }
                    if let Some(card) = dash.cards.get_mut(idx) {
                        if let CardKind::FilteredEntityList { hide_state, .. } = &mut card.kind {
                            *hide_state = !*hide_state;
                            if let Some(ed) = self.editor.as_mut() {
                                ed.dirty = true;
                            }
                        }
                    }
                }
            }
            (A::ToggleHideWhenEmpty, C::Card(idx)) => {
                if let Some(dash) = self.dashboards.get_mut(dash_idx) {
                    if let Some(ed) = self.editor.as_mut() {
                        ed.snapshot(dash);
                    }
                    if let Some(card) = dash.cards.get_mut(idx) {
                        if let CardKind::FilteredEntityList {
                            hide_when_empty, ..
                        } = &mut card.kind
                        {
                            *hide_when_empty = !*hide_when_empty;
                            if let Some(ed) = self.editor.as_mut() {
                                ed.dirty = true;
                            }
                        }
                    }
                }
            }
            (A::ToggleTicker, C::Card(idx)) => {
                if let Some(dash) = self.dashboards.get_mut(dash_idx) {
                    if let Some(ed) = self.editor.as_mut() {
                        ed.snapshot(dash);
                    }
                    if let Some(card) = dash.cards.get_mut(idx) {
                        if let CardKind::Entity { ticker, .. } = &mut card.kind {
                            *ticker = !*ticker;
                            if let Some(ed) = self.editor.as_mut() {
                                ed.dirty = true;
                            }
                        }
                    }
                }
            }
            (A::RenameDashboard, C::Dashboard) => {
                let name = self
                    .dashboards
                    .get(dash_idx)
                    .map(|d| d.name.clone())
                    .unwrap_or_default();
                if let Some(ed) = self.editor.as_mut() {
                    ed.mode = EditorMode::Renaming { buffer: name };
                }
            }
            (A::ResizeGrid, C::Dashboard) => {
                let (cols, rows) = self
                    .dashboards
                    .get(dash_idx)
                    .map(|d| (d.grid.cols.to_string(), d.grid.rows.to_string()))
                    .unwrap_or_else(|| ("12".into(), "24".into()));
                if let Some(ed) = self.editor.as_mut() {
                    ed.mode = EditorMode::ResizingGrid {
                        cols_buffer: cols,
                        rows_buffer: rows,
                        focus_rows: false,
                    };
                }
            }
            (A::AddGraphSeries, C::Card(idx)) => {
                if let Some(ed) = self.editor.as_mut() {
                    ed.selected_card = Some(idx);
                    ed.mode = EditorMode::GraphAddOneSeries {
                        card_idx: idx,
                        query: String::new(),
                        selected: 0,
                    };
                }
            }
            (A::RemoveGraphSeries, C::Card(idx)) => {
                if let Some(ed) = self.editor.as_mut() {
                    ed.selected_card = Some(idx);
                    ed.mode = EditorMode::GraphPickSeriesIndex {
                        card_idx: idx,
                        op: crate::dashboard::editor::SeriesIndexOp::Remove,
                        selected: 0,
                    };
                }
            }
            (A::SetGraphSeriesColor, C::Card(idx)) => {
                if let Some(ed) = self.editor.as_mut() {
                    ed.selected_card = Some(idx);
                    ed.mode = EditorMode::GraphPickSeriesIndex {
                        card_idx: idx,
                        op: crate::dashboard::editor::SeriesIndexOp::SetColor,
                        selected: 0,
                    };
                }
            }
            (A::SetGraphSeriesLabel, C::Card(idx)) => {
                if let Some(ed) = self.editor.as_mut() {
                    ed.selected_card = Some(idx);
                    ed.mode = EditorMode::GraphPickSeriesIndex {
                        card_idx: idx,
                        op: crate::dashboard::editor::SeriesIndexOp::SetLabel,
                        selected: 0,
                    };
                }
            }
            (A::CycleGraphType, C::Card(idx)) => {
                if let Some(dash) = self.dashboards.get_mut(dash_idx) {
                    if let Some(ed) = self.editor.as_mut() {
                        ed.snapshot(dash);
                        ed.selected_card = Some(idx);
                    }
                    if let Some(card) = dash.cards.get_mut(idx) {
                        if let crate::dashboard::CardKind::Graph { graph_type, .. } = &mut card.kind
                        {
                            *graph_type = match graph_type {
                                crate::dashboard::GraphType::Line => {
                                    crate::dashboard::GraphType::Bar
                                }
                                crate::dashboard::GraphType::Bar => {
                                    crate::dashboard::GraphType::Pie
                                }
                                crate::dashboard::GraphType::Pie => {
                                    crate::dashboard::GraphType::Line
                                }
                            };
                        }
                    }
                    if let Some(ed) = self.editor.as_mut() {
                        ed.dirty = true;
                    }
                }
            }
            (A::EditGraphWindow, C::Card(idx)) => {
                let current = self
                    .dashboards
                    .get(dash_idx)
                    .and_then(|d| d.cards.get(idx))
                    .and_then(|c| match &c.kind {
                        crate::dashboard::CardKind::Graph { window, .. } => Some(window.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| "1h".to_string());
                if let Some(ed) = self.editor.as_mut() {
                    ed.selected_card = Some(idx);
                    ed.mode = EditorMode::GraphEditWindow {
                        card_idx: idx,
                        buf: current,
                    };
                }
            }
            (A::EditGraphOrientation, C::Card(idx)) => {
                let current_ori = self
                    .dashboards
                    .get(dash_idx)
                    .and_then(|d| d.cards.get(idx))
                    .and_then(|c| match &c.kind {
                        crate::dashboard::CardKind::Graph { orientation, .. } => Some(*orientation),
                        _ => None,
                    })
                    .unwrap_or_default();
                if let Some(ed) = self.editor.as_mut() {
                    ed.selected_card = Some(idx);
                    ed.mode = EditorMode::GraphPickOrientation {
                        card_idx: idx,
                        current: current_ori,
                    };
                }
            }
            (A::EditSeverityThresholds, C::Card(idx)) => {
                // Pre-populate buffer with existing green threshold if set.
                let existing = self
                    .dashboards
                    .get(dash_idx)
                    .and_then(|d| d.cards.get(idx))
                    .and_then(|c| {
                        if let CardKind::Gauge { severity, .. } = &c.kind {
                            severity.as_ref().map(|s| (s.green, s.yellow, s.red))
                        } else {
                            None
                        }
                    });
                let (green_str, accum) = match existing {
                    Some((g, y, _)) => (
                        g.to_string(),
                        crate::dashboard::editor::SeverityAccum {
                            green: g,
                            yellow: y,
                        },
                    ),
                    None => (
                        String::new(),
                        crate::dashboard::editor::SeverityAccum::default(),
                    ),
                };
                if let Some(ed) = self.editor.as_mut() {
                    ed.selected_card = Some(idx);
                    ed.mode = EditorMode::EditSeverityGreen {
                        card_idx: idx,
                        buf: green_str,
                        accum,
                    };
                }
            }
            (A::ToggleGaugeNeedle, C::Card(idx)) => {
                if let Some(dash) = self.dashboards.get_mut(dash_idx) {
                    if let Some(ed) = self.editor.as_mut() {
                        ed.snapshot(dash);
                    }
                    if let Some(card) = dash.cards.get_mut(idx) {
                        if let CardKind::Gauge { needle, .. } = &mut card.kind {
                            *needle = !*needle;
                            if let Some(ed) = self.editor.as_mut() {
                                ed.dirty = true;
                            }
                        }
                    }
                }
            }
            (A::ClockEditFormat, C::Card(idx)) => {
                let current = self
                    .dashboards
                    .get(dash_idx)
                    .and_then(|d| d.cards.get(idx))
                    .and_then(|c| {
                        if let CardKind::Clock { format, .. } = &c.kind {
                            Some(format.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| "%H:%M:%S".into());
                if let Some(ed) = self.editor.as_mut() {
                    ed.selected_card = Some(idx);
                    ed.mode = EditorMode::ClockEditFormat {
                        card_idx: idx,
                        buf: current,
                    };
                }
            }
            (A::ClockEditTimezone, C::Card(idx)) => {
                let current = self
                    .dashboards
                    .get(dash_idx)
                    .and_then(|d| d.cards.get(idx))
                    .and_then(|c| {
                        if let CardKind::Clock { timezone, .. } = &c.kind {
                            timezone.clone()
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default();
                if let Some(ed) = self.editor.as_mut() {
                    ed.selected_card = Some(idx);
                    ed.mode = EditorMode::ClockEditTimezone {
                        card_idx: idx,
                        buf: current,
                    };
                }
            }
            (A::StatsEditMetric, C::Card(idx)) => {
                // Find the current metric index to pre-select it.
                let current_selected = self
                    .dashboards
                    .get(dash_idx)
                    .and_then(|d| d.cards.get(idx))
                    .and_then(|c| {
                        if let CardKind::Statistics { metric, .. } = &c.kind {
                            use crate::dashboard::StatsMetric::*;
                            Some(match metric {
                                Avg => 0,
                                Min => 1,
                                Max => 2,
                                Sum => 3,
                                Count => 4,
                            })
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0);
                if let Some(ed) = self.editor.as_mut() {
                    ed.selected_card = Some(idx);
                    ed.mode = EditorMode::StatsEditMetric {
                        card_idx: idx,
                        selected: current_selected,
                    };
                }
            }
            (A::StatsEditWindow, C::Card(idx)) => {
                let current = self
                    .dashboards
                    .get(dash_idx)
                    .and_then(|d| d.cards.get(idx))
                    .and_then(|c| {
                        if let CardKind::Statistics { window, .. } = &c.kind {
                            Some(window.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| "1h".into());
                if let Some(ed) = self.editor.as_mut() {
                    ed.selected_card = Some(idx);
                    ed.mode = EditorMode::StatsEditWindow {
                        card_idx: idx,
                        buf: current,
                    };
                }
            }
            (A::StatsEditUnit, C::Card(idx)) => {
                let current = self
                    .dashboards
                    .get(dash_idx)
                    .and_then(|d| d.cards.get(idx))
                    .and_then(|c| {
                        if let CardKind::Statistics { unit, .. } = &c.kind {
                            unit.clone()
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default();
                if let Some(ed) = self.editor.as_mut() {
                    ed.selected_card = Some(idx);
                    ed.mode = EditorMode::StatsEditUnit {
                        card_idx: idx,
                        buf: current,
                    };
                }
            }
            _ => {
                self.last_error = Some("menu action not valid in this context".into());
            }
        }
    }

    fn change_card_entity(&mut self, idx: usize) {
        // Determine entry point from current card kind; prefill picker state.
        let Some(editor) = self.editor.as_mut() else {
            return;
        };
        let Some(dash) = self.dashboards.get(editor.dash_idx) else {
            return;
        };
        let Some(card) = dash.cards.get(idx) else {
            return;
        };
        use crate::dashboard::CardKind;
        let (kind, instance, prefill): (CardTypeStub, String, _) = match &card.kind {
            CardKind::Entity { instance, .. } => (CardTypeStub::Entity, instance.clone(), None),
            CardKind::Toggle { instance, .. } => (CardTypeStub::Toggle, instance.clone(), None),
            CardKind::Gauge { instance, .. } => (CardTypeStub::Gauge, instance.clone(), None),
            CardKind::Graph { instance, .. } => (CardTypeStub::Graph, instance.clone(), None),
            CardKind::EntityList {
                instance, entities, ..
            } => {
                let picked: Vec<(String, String)> = entities
                    .iter()
                    .map(|eid| {
                        let friendly = self
                            .instances
                            .runtimes
                            .get(instance)
                            .and_then(|rt| rt.states.get(eid))
                            .and_then(|s| s.attributes.get("friendly_name"))
                            .and_then(|v| v.as_str())
                            .map(str::to_string)
                            .unwrap_or_default();
                        (eid.clone(), friendly)
                    })
                    .collect();
                (CardTypeStub::EntityList, instance.clone(), Some(picked))
            }
            CardKind::Text { .. } => {
                self.last_error = Some("text cards have no entity to change".into());
                return;
            }
            CardKind::FilteredEntityList { .. } => {
                // Filtered lists are edited via the query editor, not the entity picker.
                self.last_error = Some(
                    "filtered list uses query syntax — open menu (m) → Edit filter query".into(),
                );
                return;
            }
            CardKind::Clock { .. } => {
                self.last_error = Some("clock cards have no entity to change".into());
                return;
            }
            CardKind::Statistics { instance, .. } => {
                (CardTypeStub::Statistics, instance.clone(), None)
            }
            CardKind::MediaPlayer { instance, .. } => {
                (CardTypeStub::MediaPlayer, instance.clone(), None)
            }
            CardKind::Image { .. } => {
                self.last_error = Some("image cards: use the YAML editor to change entity".into());
                return;
            }
            CardKind::Weather { instance, .. } => (CardTypeStub::Weather, instance.clone(), None),
        };
        editor.edit_target = Some(idx);
        editor.mode = if let Some(picked) = prefill {
            EditorMode::PickingMulti {
                instance,
                query: String::new(),
                selected: 0,
                picked,
            }
        } else {
            EditorMode::PickingEntity {
                card_type: kind,
                instance,
                query: String::new(),
                selected: 0,
            }
        };
    }

    fn start_card_after_type(&mut self, kind: CardTypeStub) {
        let Some(editor) = self.editor.as_mut() else {
            return;
        };
        if matches!(kind, CardTypeStub::Text) {
            editor.mode = EditorMode::EditingTextBody {
                title_buffer: String::new(),
                body_buffer: String::new(),
                focus_body: false,
            };
            return;
        }
        // Clock has no instance or entity — go straight to title input.
        if matches!(kind, CardTypeStub::Clock) {
            editor.mode = EditorMode::ClockAddTitle {
                title_buffer: String::new(),
            };
            return;
        }
        // Statistics: has instance + entity, then extra metric/window/unit steps.
        // Falls through to the normal PickingInstance / PickingEntity flow below,
        // but branches at PickingEntity::Enter to StatsPickMetric instead of EditingTitle.
        // Graph has its own multi-step flow: type → instance → entities → config.
        // Always route through GraphPickType first, regardless of instance count.
        if matches!(kind, CardTypeStub::Graph) {
            editor.mode = EditorMode::GraphPickType;
            return;
        }
        // Image: pick source kind (image entity vs camera) first, then instance + entity.
        if matches!(kind, CardTypeStub::Image) {
            editor.mode = EditorMode::ImagePickSourceKind { selected: 0 };
            return;
        }
        let aliases: Vec<String> = self.instances.runtimes.keys().cloned().collect();
        match aliases.len() {
            0 => {
                self.last_error = Some("no instances connected".into());
                if let Some(e) = self.editor.as_mut() {
                    e.mode = EditorMode::Browse;
                }
            }
            1 => {
                editor.mode = picker_mode_for(kind, aliases.into_iter().next().unwrap());
            }
            _ => {
                editor.mode = EditorMode::PickingInstance {
                    card_type: kind,
                    selected: 0,
                };
            }
        }
    }

    fn start_graph_after_type(&mut self, graph_type: crate::dashboard::GraphType) {
        let aliases: Vec<String> = self.instances.runtimes.keys().cloned().collect();
        let Some(editor) = self.editor.as_mut() else {
            return;
        };
        match aliases.len() {
            0 => {
                self.last_error = Some("no instances connected".into());
                editor.mode = EditorMode::Browse;
            }
            1 => {
                let inst = aliases.into_iter().next().unwrap();
                editor.mode = EditorMode::GraphAddEntities {
                    instance: inst,
                    graph_type,
                    accumulated: Vec::new(),
                    query: String::new(),
                    selected: 0,
                    asking_more: false,
                };
            }
            _ => {
                editor.mode = EditorMode::GraphPickInstance {
                    graph_type,
                    selected: 0,
                };
            }
        }
    }

    fn advance_graph_to_config(
        &mut self,
        instance: String,
        graph_type: crate::dashboard::GraphType,
        series: Vec<crate::dashboard::GraphSeries>,
    ) {
        let Some(editor) = self.editor.as_mut() else {
            return;
        };
        match graph_type {
            crate::dashboard::GraphType::Line => {
                editor.mode = EditorMode::GraphEditWindowAdd {
                    instance,
                    graph_type,
                    series,
                    window_buf: String::new(),
                    title_buf: String::new(),
                    title_stage: false,
                };
            }
            crate::dashboard::GraphType::Bar => {
                editor.mode = EditorMode::GraphPickOrientationAdd {
                    instance,
                    series,
                    current: crate::dashboard::BarOrientation::default(),
                    title_buf: String::new(),
                    title_stage: false,
                };
            }
            crate::dashboard::GraphType::Pie => {
                // Pie: no extra config — just prompt for title then commit.
                // Reuse GraphEditWindowAdd with empty window (we won't show a window prompt,
                // but this avoids another mode variant). Instead use a dedicated commit path.
                // Immediately go to title stage with a dummy window.
                editor.mode = EditorMode::GraphEditWindowAdd {
                    instance,
                    graph_type,
                    series,
                    window_buf: "1h".to_string(),
                    title_buf: String::new(),
                    title_stage: true, // skip window prompt, go straight to title
                };
            }
        }
    }

    fn create_new_dashboard(&mut self) {
        let n = self.dashboards.len() + 1;
        let dash = crate::dashboard::Dashboard {
            name: format!("Dashboard {n}"),
            grid: crate::dashboard::Grid { cols: 12, rows: 24 },
            cards: Vec::new(),
        };
        self.dashboards.push(dash);
        let idx = self.dashboards.len() - 1;
        let path = self
            .dashboards_path
            .clone()
            .or_else(crate::dashboard::persist::default_path);
        let mut ed = EditorState::new(idx, path);
        ed.dirty = true;
        self.editor = Some(ed);
        self.screen = Screen::Editor;
        self.status_msg = Some(format!(
            "new dashboard #{n} — press 'R' to rename, 's' to save"
        ));
    }

    fn handle_mouse(&mut self, m: MouseEvent) {
        // Only the editor uses mouse for card manipulation in this milestone.
        if !matches!(self.screen, Screen::Editor) {
            return;
        }
        let Some(editor) = self.editor.as_mut() else {
            return;
        };
        let Some(dash) = self.dashboards.get_mut(editor.dash_idx) else {
            return;
        };

        let (w, h) = self.last_terminal_size;
        if w == 0 || h == 0 {
            return;
        }
        let body = Rect {
            x: 0,
            y: 2,
            width: w,
            height: h.saturating_sub(3),
        };
        let Some((col, row)) = mouse_to_cell(body, dash, m.column, m.row) else {
            return;
        };
        editor.cursor_col = col;
        editor.cursor_row = row;

        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                editor.selected_card = crate::dashboard::editor::card_at(dash, col, row);
                self.mouse_drag = Some(MouseDrag {
                    origin_col: col,
                    origin_row: row,
                });
            }
            MouseEventKind::Drag(MouseButton::Left) if editor.selected_card.is_some() => {
                editor.snapshot(dash);
                editor.move_selected_to_cursor(dash);
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.mouse_drag = None;
            }
            _ => {}
        }
    }

    fn cycle_instance_filter(&mut self) {
        let Screen::Entities {
            instance_filter, ..
        } = &mut self.screen
        else {
            return;
        };
        let aliases: Vec<Alias> = self.instances.runtimes.keys().cloned().collect();
        *instance_filter = match instance_filter.take() {
            None => aliases.first().cloned(),
            Some(cur) => {
                let pos = aliases.iter().position(|a| a == &cur);
                match pos {
                    Some(i) if i + 1 < aliases.len() => Some(aliases[i + 1].clone()),
                    _ => None,
                }
            }
        };
    }

    /// Returns `true` if the key was consumed by a MediaPlayer card action.
    fn handle_key_media_player(&mut self, ch: char) -> bool {
        let Screen::Dashboard {
            idx, selected_card, ..
        } = &self.screen
        else {
            return false;
        };
        let (idx, selected_card) = (*idx, *selected_card);
        let Some(card) = self
            .dashboards
            .get(idx)
            .and_then(|d| d.cards.get(selected_card))
        else {
            return false;
        };
        let CardKind::MediaPlayer {
            instance, entity, ..
        } = &card.kind
        else {
            return false;
        };
        let Some(service) = crate::actions::media_service_for_key(ch) else {
            return false;
        };
        let instance = instance.clone();
        let entity = entity.clone();
        let service_data = if service == "volume_mute" {
            let current_muted = self
                .instances
                .runtimes
                .get(&instance)
                .and_then(|rt| rt.states.get(&entity))
                .and_then(|s| s.attributes.get("is_volume_muted"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            serde_json::json!({ "is_volume_muted": !current_muted })
        } else {
            serde_json::Value::Null
        };
        let cmd = crate::ha::HaCommand::CallService {
            domain: "media_player".to_string(),
            service: service.to_string(),
            service_data,
            target: serde_json::json!({ "entity_id": entity }),
        };
        if !self.instances.send(&instance, cmd) {
            self.last_error = Some(format!("{instance}: no command channel"));
        } else {
            tracing::info!(%instance, %entity, %service, "media service call dispatched");
        }
        true
    }

    fn trigger_default_action(&mut self) {
        match &self.screen {
            Screen::Entities {
                instance_filter,
                search,
                selected,
            } => {
                let rows = crate::screens::entities::build_rows(
                    self.instances.runtimes.values(),
                    instance_filter.as_ref(),
                    search,
                );
                let Some(row) = rows.get(*selected) else {
                    return;
                };
                let alias = row.instance.clone();
                let entity_id = row.state.entity_id.clone();
                self.dispatch_default(&alias, &entity_id);
            }
            Screen::Dashboard {
                idx,
                selected_card,
                sub_index,
            } => {
                let Some(dash) = self.dashboards.get(*idx) else {
                    return;
                };
                let Some(card) = dash.cards.get(*selected_card) else {
                    return;
                };
                // EntityList / FilteredEntityList: act on sub-selected entity.
                if let Some((alias, entities)) = list_entities(card, &self.instances) {
                    let Some(eid) = entities.get(*sub_index).cloned() else {
                        return;
                    };
                    self.dispatch_default(&alias, &eid);
                    return;
                }
                let Some((alias, entity)) = card.entity_ref() else {
                    return;
                };
                let alias = alias.clone();
                let entity = entity.clone();
                self.dispatch_default(&alias, &entity);
            }
            Screen::Instances { .. } | Screen::Editor => {}
        }
    }

    fn dispatch_default(&mut self, alias: &Alias, entity_id: &EntityId) {
        match crate::actions::default_action(entity_id) {
            Some(cmd) => {
                if !self.instances.send(alias, cmd) {
                    self.last_error = Some(format!("{alias}: no command channel"));
                } else {
                    tracing::info!(%alias, %entity_id, "service call dispatched");
                }
            }
            None => {
                self.last_error = Some(format!(
                    "no default action for {entity_id} (free-form dialog: TBD)"
                ));
            }
        }
    }

    fn move_selection(&mut self, delta: i32) {
        // Special-case Dashboard: if the selected card is an EntityList, j/k
        // navigates rows within that card; otherwise it moves between cards.
        if let Screen::Dashboard {
            idx,
            selected_card,
            sub_index,
        } = &mut self.screen
        {
            if let Some(dash) = self.dashboards.get(*idx) {
                if let Some(card) = dash.cards.get(*selected_card) {
                    if let Some((_, entities)) = list_entities(card, &self.instances) {
                        if entities.is_empty() {
                            return;
                        }
                        let cur = i64::try_from(*sub_index).unwrap_or(0);
                        let new = (cur + i64::from(delta)).clamp(0, entities.len() as i64 - 1);
                        *sub_index = new as usize;
                        return;
                    }
                }
            }
        }
        let total = self.current_row_count();
        if total == 0 {
            return;
        }
        let selected = match &mut self.screen {
            Screen::Entities { selected, .. } => selected,
            Screen::Instances { selected } => selected,
            Screen::Dashboard { selected_card, .. } => selected_card,
            Screen::Editor => return,
        };
        let cur = i64::try_from(*selected).unwrap_or(0);
        let new = (cur + i64::from(delta)).clamp(0, total as i64 - 1);
        *selected = new as usize;
    }

    fn move_card_selection(&mut self, delta: i32) {
        let Screen::Dashboard {
            idx,
            selected_card,
            sub_index,
        } = &mut self.screen
        else {
            return;
        };
        let Some(dash) = self.dashboards.get(*idx) else {
            return;
        };
        if dash.cards.is_empty() {
            return;
        }
        let total = dash.cards.len() as i64;
        let cur = i64::try_from(*selected_card).unwrap_or(0);
        let new = (cur + i64::from(delta)).rem_euclid(total);
        *selected_card = new as usize;
        *sub_index = 0;
    }

    fn set_selection(&mut self, n: usize) {
        let total = self.current_row_count();
        if total == 0 {
            return;
        }
        let selected = match &mut self.screen {
            Screen::Entities { selected, .. } => selected,
            Screen::Instances { selected } => selected,
            Screen::Dashboard { selected_card, .. } => selected_card,
            Screen::Editor => return,
        };
        *selected = n.min(total - 1);
    }

    fn current_row_count(&self) -> usize {
        match &self.screen {
            Screen::Entities {
                instance_filter,
                search,
                ..
            } => crate::screens::entities::build_rows(
                self.instances.runtimes.values(),
                instance_filter.as_ref(),
                search,
            )
            .len(),
            Screen::Instances { .. } => self.instances.runtimes.len(),
            Screen::Dashboard { idx, .. } => self.dashboards.get(*idx).map_or(0, |d| d.cards.len()),
            Screen::Editor => 0,
        }
    }

    fn record_history(&mut self, alias: &Alias, entity: &EntityId, state_str: &str) {
        let Ok(v) = state_str.parse::<f64>() else {
            return;
        };
        let key = (alias.clone(), entity.clone());
        let buf = self
            .history
            .entry(key)
            .or_insert_with(|| RingBuf::new(HISTORY_CAP));
        buf.push(std::time::Instant::now(), v);
    }

    fn handle_app(&mut self, ev: AppEvent) {
        match ev {
            AppEvent::Quit => self.should_quit = true,
            AppEvent::Tick => {}
            AppEvent::HaConnStatus {
                instance,
                status,
                error,
            } => {
                if let Some(rt) = self.instances.get_mut(&instance) {
                    rt.status = status;
                    rt.last_error.clone_from(&error);
                }
                if let Some(e) = error {
                    self.last_error = Some(format!("{instance}: {e}"));
                }
            }
            AppEvent::HaInitialStates { instance, states } => {
                if let Some(rt) = self.instances.get_mut(&instance) {
                    rt.states.clear();
                    for s in &states {
                        rt.states.insert(s.entity_id.clone(), s.clone());
                    }
                    rt.states.sort_by(|a, _, b, _| a.cmp(b));
                    rt.last_update = Some(std::time::Instant::now());
                }
                for s in states {
                    self.record_history(&instance, &s.entity_id, &s.state);
                }
                self.last_error = None;
                self.fetch_sparkline_history(&instance);
                self.fetch_image_cards(&instance);
                self.fetch_weather_forecasts(&instance);
            }
            AppEvent::HaEntityUpdated { instance, state } => {
                if let Some(rt) = self.instances.get_mut(&instance) {
                    rt.states.insert(state.entity_id.clone(), state.clone());
                    rt.last_update = Some(std::time::Instant::now());
                }
                self.record_history(&instance, &state.entity_id, &state.state);
                // Re-fetch image/camera cards when their entity state changes.
                self.refresh_image_card_if_needed(&instance, &state.entity_id.clone());
            }
            AppEvent::HaServiceError { instance, error } => {
                self.last_error = Some(format!("{instance}: {error}"));
            }
            AppEvent::HaHistory {
                instance,
                entity_id,
                samples,
            } => {
                let key = (instance.clone(), entity_id);
                let buf = self
                    .history
                    .entry(key)
                    .or_insert_with(|| RingBuf::new(HISTORY_CAP));
                buf.fill_from(samples.into_iter().map(|(_, v)| v));
            }
            AppEvent::HaImageBytes {
                instance,
                entity,
                result,
            } => {
                let key = (instance.clone(), entity.clone());
                self.image_inflight.remove(&key);
                match result {
                    Ok(bytes) => {
                        tracing::debug!(
                            instance = %instance,
                            entity = %entity,
                            bytes = bytes.len(),
                            "image fetch succeeded"
                        );
                        if let Some(picker) = &mut self.image_picker {
                            match image::load_from_memory(&bytes) {
                                Ok(img) => {
                                    let protocol = picker.new_resize_protocol(img);
                                    self.image_cache.insert(
                                        key,
                                        ImageCacheEntry {
                                            protocol,
                                            error: None,
                                        },
                                    );
                                }
                                Err(e) => {
                                    tracing::warn!(entity = %entity, error = %e, "image decode failed");
                                    // Keep any existing cached frame; only log the error.
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(entity = %entity, error = %e, "image fetch failed");
                        self.last_error = Some(format!("image fetch {entity}: {e}"));
                    }
                }
            }
            AppEvent::RefreshImageCard { instance, entity } => {
                self.send_image_fetch(&instance, &entity.clone());
            }
            AppEvent::HaWeatherForecast {
                instance,
                entity,
                forecast,
            } => {
                self.weather_forecasts.insert((instance, entity), forecast);
            }
            AppEvent::RefreshWeatherForecast { instance, entity } => {
                let _ = self.instances.send(
                    &instance,
                    crate::ha::HaCommand::GetWeatherForecast {
                        entity,
                        kind: ForecastKind::Daily,
                    },
                );
            }
        }
    }

    /// Trigger an image fetch for all Image cards on dashboards matching `instance`,
    /// parallel to `fetch_sparkline_history`.
    fn fetch_image_cards(&mut self, instance: &Alias) {
        let mut entities: Vec<String> = Vec::new();
        for dash in &self.dashboards {
            for card in &dash.cards {
                if let crate::dashboard::CardKind::Image {
                    instance: card_inst,
                    source,
                    ..
                } = &card.kind
                {
                    if card_inst == instance {
                        let entity = match source {
                            crate::dashboard::ImageSource::ImageEntity { entity } => entity.clone(),
                            crate::dashboard::ImageSource::Camera { entity } => entity.clone(),
                        };
                        entities.push(entity);
                    }
                }
            }
        }
        for entity in entities {
            self.send_image_fetch(instance, &entity);
        }
    }

    /// Request forecast for every Weather card on dashboards matching `instance`.
    fn fetch_weather_forecasts(&mut self, instance: &Alias) {
        let mut entities: Vec<String> = Vec::new();
        for dash in &self.dashboards {
            for card in &dash.cards {
                if let crate::dashboard::CardKind::Weather {
                    instance: card_inst,
                    entity,
                    ..
                } = &card.kind
                {
                    if card_inst == instance {
                        entities.push(entity.clone());
                    }
                }
            }
        }
        for entity in entities {
            let _ = self.instances.send(
                instance,
                crate::ha::HaCommand::GetWeatherForecast {
                    entity,
                    kind: ForecastKind::Daily,
                },
            );
        }
    }

    /// Send a `FetchImageBytes` command if not already in-flight.
    fn send_image_fetch(&mut self, instance: &Alias, entity: &EntityId) {
        let key = (instance.clone(), entity.clone());
        if self.image_inflight.contains(&key) {
            return;
        }
        // Determine kind from the card definition.
        let kind = self.image_fetch_kind_for(instance, entity);
        let Some(kind) = kind else { return };
        self.image_inflight.insert(key);
        let _ = self.instances.send(
            instance,
            crate::ha::HaCommand::FetchImageBytes {
                entity: entity.clone(),
                kind,
            },
        );
    }

    /// Look up whether an entity is an image or camera source across all dashboards.
    fn image_fetch_kind_for(&self, instance: &Alias, entity: &EntityId) -> Option<ImageFetchKind> {
        for dash in &self.dashboards {
            for card in &dash.cards {
                if let crate::dashboard::CardKind::Image {
                    instance: card_inst,
                    source,
                    ..
                } = &card.kind
                {
                    if card_inst != instance {
                        continue;
                    }
                    match source {
                        crate::dashboard::ImageSource::ImageEntity { entity: e } if e == entity => {
                            return Some(ImageFetchKind::Image);
                        }
                        crate::dashboard::ImageSource::Camera { entity: e } if e == entity => {
                            return Some(ImageFetchKind::Camera);
                        }
                        _ => {}
                    }
                }
            }
        }
        None
    }

    /// Re-fetch an image card if the updated entity is an image/camera source.
    fn refresh_image_card_if_needed(&mut self, instance: &Alias, entity: &EntityId) {
        if self.image_fetch_kind_for(instance, entity).is_some() {
            self.send_image_fetch(instance, entity);
        }
    }

    fn fetch_sparkline_history(&mut self, instance: &Alias) {
        // For each graph/statistics card on every dashboard matching this instance, request backfill.
        let mut requests: Vec<(String, u32)> = Vec::new();
        for dash in &self.dashboards {
            for card in &dash.cards {
                match &card.kind {
                    crate::dashboard::CardKind::Graph {
                        instance: card_inst,
                        window,
                        ..
                    } if card_inst == instance => {
                        let hours = parse_window_hours(window);
                        for eid in card.graph_entities() {
                            requests.push((eid.clone(), hours));
                        }
                    }
                    crate::dashboard::CardKind::Statistics {
                        instance: card_inst,
                        entity,
                        window,
                        ..
                    } if card_inst == instance => {
                        let hours = parse_window_hours(window);
                        requests.push((entity.clone(), hours));
                    }
                    _ => {}
                }
            }
        }
        for (eid, hours) in requests {
            let _ = self.instances.send(
                instance,
                crate::ha::HaCommand::FetchHistory {
                    entity_id: eid,
                    hours,
                },
            );
        }
    }
}

/// Row returned by entity search modal.
#[derive(Debug, Clone)]
pub struct EntityPick {
    pub entity_id: String,
    pub friendly_name: String,
}

pub fn entity_search(instances: &InstanceRegistry, instance: &str, query: &str) -> Vec<EntityPick> {
    entity_search_filtered(instances, instance, query, None)
}

pub fn entity_search_filtered(
    instances: &InstanceRegistry,
    instance: &str,
    query: &str,
    domain_prefix: Option<&str>,
) -> Vec<EntityPick> {
    let Some(rt) = instances.runtimes.get(instance) else {
        return Vec::new();
    };
    let q = query.to_ascii_lowercase();
    let mut out: Vec<EntityPick> = rt
        .states
        .values()
        .map(|s| {
            let friendly = s
                .attributes
                .get("friendly_name")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .unwrap_or_default();
            EntityPick {
                entity_id: s.entity_id.clone(),
                friendly_name: friendly,
            }
        })
        .filter(|p| {
            if let Some(prefix) = domain_prefix {
                if !p.entity_id.starts_with(prefix) {
                    return false;
                }
            }
            if q.is_empty() {
                return true;
            }
            p.entity_id.to_ascii_lowercase().contains(&q)
                || p.friendly_name.to_ascii_lowercase().contains(&q)
        })
        .collect();
    out.sort_by(|a, b| a.entity_id.cmp(&b.entity_id));
    out
}

/// Convert a card `window` spec like "1h", "30m", "24h", "1d", "7d" to hours.
fn parse_window_hours(s: &str) -> u32 {
    let s = s.trim();
    if let Some(num) = s.strip_suffix('h') {
        return num.parse().unwrap_or(1);
    }
    if let Some(num) = s.strip_suffix('d') {
        return num.parse::<u32>().unwrap_or(1) * 24;
    }
    if let Some(num) = s.strip_suffix('m') {
        let mins: u32 = num.parse().unwrap_or(60);
        return (mins / 60).max(1);
    }
    1
}

/// Return the (instance, entities) pair for any card that displays a list of entities.
pub fn list_entities(
    card: &crate::dashboard::Card,
    instances: &InstanceRegistry,
) -> Option<(Alias, Vec<String>)> {
    use crate::dashboard::CardKind as CK;
    match &card.kind {
        CK::EntityList {
            instance, entities, ..
        } => Some((instance.clone(), entities.clone())),
        CK::FilteredEntityList {
            instance, query, ..
        } => {
            let rt = instances.runtimes.get(instance);
            Some((
                instance.clone(),
                crate::dashboard::query::resolve(rt, query),
            ))
        }
        _ => None,
    }
}

pub fn domain_prefix_for_type(kind: CardTypeStub) -> Option<&'static str> {
    match kind {
        CardTypeStub::MediaPlayer => Some("media_player."),
        // Image cards accept both `image.` and `camera.` entities — no single prefix filter.
        _ => None,
    }
}

fn picker_mode_for(kind: CardTypeStub, instance: String) -> EditorMode {
    match kind {
        CardTypeStub::EntityList => EditorMode::PickingMulti {
            instance,
            query: String::new(),
            selected: 0,
            picked: Vec::new(),
        },
        CardTypeStub::FilteredEntityList => EditorMode::EditingFilterQuery {
            instance,
            query_buffer: String::new(),
            title_buffer: String::new(),
            hide_state: false,
            focus: crate::dashboard::editor::FilterFocus::Query,
        },
        _ => EditorMode::PickingEntity {
            card_type: kind,
            instance,
            query: String::new(),
            selected: 0,
        },
    }
}

fn build_typed_card(
    kind: CardTypeStub,
    instance: String,
    entity: String,
    title: Option<String>,
) -> CardKind {
    match kind {
        CardTypeStub::Entity => CardKind::Entity {
            instance,
            entity,
            title,
            ticker: false,
        },
        CardTypeStub::Toggle => CardKind::Toggle {
            instance,
            entity,
            title,
        },
        CardTypeStub::Gauge => CardKind::Gauge {
            instance,
            entity,
            min: 0.0,
            max: 100.0,
            unit: None,
            severity: None,
            needle: true,
            title,
        },
        CardTypeStub::Graph => CardKind::Graph {
            instance,
            entity: Some(entity),
            entities: Vec::new(),
            graph_type: crate::dashboard::GraphType::default(),
            window: "1h".into(),
            orientation: crate::dashboard::BarOrientation::default(),
            title,
        },
        CardTypeStub::Text => CardKind::Text {
            markdown: String::new(),
            title,
        },
        CardTypeStub::EntityList => CardKind::EntityList {
            instance,
            entities: vec![entity],
            title,
        },
        CardTypeStub::FilteredEntityList => unreachable!(
            "FilteredEntityList is built via EditingFilterQuery flow, not build_typed_card"
        ),
        CardTypeStub::Clock => {
            unreachable!("Clock is built via ClockAddTitle flow, not build_typed_card")
        }
        CardTypeStub::Statistics => {
            unreachable!("Statistics is built via StatsPickMetric flow, not build_typed_card")
        }
        CardTypeStub::Image => {
            unreachable!("Image is built via ImagePickSourceKind flow, not build_typed_card")
        }
        CardTypeStub::Weather => {
            unreachable!("Weather is built via WxEditShowForecast flow, not build_typed_card")
        }
        CardTypeStub::MediaPlayer => CardKind::MediaPlayer {
            instance,
            entity,
            title,
        },
    }
}

#[allow(dead_code)]
fn build_card_kind(kind: CardTypeStub, buf: &str, default_alias: Option<&str>) -> Option<CardKind> {
    let buf = buf.trim();
    if buf.is_empty() && !matches!(kind, CardTypeStub::Text) {
        return None;
    }
    if matches!(kind, CardTypeStub::Text) {
        return Some(CardKind::Text {
            markdown: buf.to_string(),
            title: None,
        });
    }
    let (instance, entity) = if let Some((a, b)) = buf.split_once('/') {
        (a.to_string(), b.to_string())
    } else {
        (default_alias?.to_string(), buf.to_string())
    };
    Some(match kind {
        CardTypeStub::Entity => CardKind::Entity {
            instance,
            entity,
            title: None,
            ticker: false,
        },
        CardTypeStub::Toggle => CardKind::Toggle {
            instance,
            entity,
            title: None,
        },
        CardTypeStub::Gauge => CardKind::Gauge {
            instance,
            entity,
            min: 0.0,
            max: 100.0,
            unit: None,
            severity: None,
            needle: true,
            title: None,
        },
        CardTypeStub::Graph => CardKind::Graph {
            instance,
            entity: Some(entity),
            entities: Vec::new(),
            graph_type: crate::dashboard::GraphType::default(),
            window: "1h".into(),
            orientation: crate::dashboard::BarOrientation::default(),
            title: None,
        },
        CardTypeStub::MediaPlayer => CardKind::MediaPlayer {
            instance,
            entity,
            title: None,
        },
        CardTypeStub::Text
        | CardTypeStub::EntityList
        | CardTypeStub::FilteredEntityList
        | CardTypeStub::Clock
        | CardTypeStub::Statistics
        | CardTypeStub::Image
        | CardTypeStub::Weather => {
            unreachable!()
        }
    })
}

/// Spawn one tokio task per Camera card that has `refresh_seconds` set.
/// Each task sends `AppEvent::RefreshImageCard` on the given interval.
pub fn spawn_camera_timers(dashboards: &[Dashboard], tx: &mpsc::UnboundedSender<AppEvent>) {
    for dash in dashboards {
        for card in &dash.cards {
            if let crate::dashboard::CardKind::Image {
                instance,
                source: crate::dashboard::ImageSource::Camera { entity },
                refresh_seconds: Some(secs),
                ..
            } = &card.kind
            {
                let instance = instance.clone();
                let entity = entity.clone();
                let secs = *secs;
                let tx = tx.clone();
                tokio::spawn(async move {
                    let mut interval = tokio::time::interval(Duration::from_secs(secs as u64));
                    interval.tick().await; // skip the immediate first tick
                    loop {
                        interval.tick().await;
                        if tx
                            .send(AppEvent::RefreshImageCard {
                                instance: instance.clone(),
                                entity: entity.clone(),
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                });
            }
        }
    }
}

/// Spawn a single tokio task that sends `RefreshWeatherForecast` every 30 minutes
/// for every Weather card found across all dashboards.
pub fn spawn_weather_timer(dashboards: &[Dashboard], tx: &mpsc::UnboundedSender<AppEvent>) {
    // Collect unique (instance, entity) pairs from all Weather cards.
    let mut pairs: Vec<(String, String)> = Vec::new();
    for dash in dashboards {
        for card in &dash.cards {
            if let crate::dashboard::CardKind::Weather {
                instance, entity, ..
            } = &card.kind
            {
                let key = (instance.clone(), entity.clone());
                if !pairs.contains(&key) {
                    pairs.push(key);
                }
            }
        }
    }
    if pairs.is_empty() {
        return;
    }
    let tx = tx.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30 * 60));
        interval.tick().await; // skip the immediate first tick
        loop {
            interval.tick().await;
            for (instance, entity) in &pairs {
                if tx
                    .send(AppEvent::RefreshWeatherForecast {
                        instance: instance.clone(),
                        entity: entity.clone(),
                    })
                    .is_err()
                {
                    return;
                }
            }
        }
    });
}

fn mouse_to_cell(area: Rect, dash: &Dashboard, mx: u16, my: u16) -> Option<(u16, u16)> {
    if mx < area.x || mx >= area.x + area.width || my < area.y || my >= area.y + area.height {
        return None;
    }
    let dx = mx - area.x;
    let dy = my - area.y;
    let cell_w = (area.width as f32 / dash.grid.cols as f32).max(1.0);
    let cell_h = (area.height as f32 / dash.grid.rows as f32).max(1.0);
    let col = ((dx as f32 / cell_w) as u16).min(dash.grid.cols - 1);
    let row = ((dy as f32 / cell_h) as u16).min(dash.grid.rows - 1);
    Some((col, row))
}

pub async fn run(
    mut terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    config_path: Option<PathBuf>,
    dashboards_path: Option<PathBuf>,
) -> Result<()> {
    info!("starting ha-tui");

    let (tx, mut rx) = mpsc::unbounded_channel::<AppEvent>();
    let mut app = App::new(tx.clone());
    app.dashboards_path = dashboards_path.clone();

    match config::load::load(config_path.as_deref()) {
        Ok(cfg) => {
            app.theme = Theme::from_config(&cfg);
            for inst in &cfg.instances {
                let rt = InstanceRuntime::new(inst.alias.clone(), inst.url.clone());
                let tx_cmd = client::spawn(inst.clone(), tx.clone());
                app.instances.add(rt, tx_cmd);
            }
            app.config = Some(cfg);
        }
        Err(e) => {
            error!(error = %e, "config load failed");
            app.last_error = Some(format!("config: {e}"));
        }
    }

    match dashboard::persist::load(dashboards_path.as_deref()) {
        Ok(file) => app.dashboards = file.dashboards,
        Err(e) => {
            error!(error = %e, "dashboards load failed");
            app.last_error = Some(format!("dashboards: {e}"));
        }
    }

    let mut term_events = EventStream::new();
    let mut tick = tokio::time::interval(Duration::from_millis(250));

    // Init ratatui-image picker after terminal setup (must not run in tests).
    app.image_picker = ratatui_image::picker::Picker::from_query_stdio().ok();

    // Spawn interval refresh timers for Camera cards that have refresh_seconds set.
    spawn_camera_timers(&app.dashboards, &tx);
    // Spawn 30-minute refresh timer for Weather cards.
    spawn_weather_timer(&app.dashboards, &tx);

    let initial = terminal.size().unwrap_or_default();
    app.last_terminal_size = (initial.width, initial.height);
    terminal.draw(|f| ui::draw(f, &mut app))?;

    let result: Result<()> = async {
        loop {
            tokio::select! {
                Some(Ok(ev)) = term_events.next() => app.handle_term(ev),
                Some(ev) = rx.recv()              => app.handle_app(ev),
                _ = tick.tick()                   => { app.ticker_offset = app.ticker_offset.wrapping_add(1); }
            }
            if app.should_quit {
                break;
            }
            terminal.draw(|f| ui::draw(f, &mut app))?;
        }
        Ok(())
    }
    .await;

    info!("ha-tui exiting cleanly");
    result
}
