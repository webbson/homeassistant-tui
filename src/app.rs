use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use color_eyre::Result;
use crossterm::event::{
    Event as CtEvent, EventStream, KeyCode, KeyEvent, KeyEventKind, MouseButton, MouseEvent,
    MouseEventKind,
};
use futures::StreamExt;
use ratatui::layout::Rect;
use ratatui::DefaultTerminal;
use tokio::sync::mpsc;
use tracing::{error, info};

use crate::config::{self, Alias, Config};
use crate::dashboard::editor::{CardTypeStub, EditorMode, EditorState};
use crate::dashboard::{self, CardKind, Dashboard};
use crate::event::AppEvent;
use crate::ha::{client, EntityId, InstanceRegistry, InstanceRuntime};
use crate::screens::Screen;
use crate::ui;
use crate::ui::theme::Theme;
use crate::util::history::RingBuf;

const HISTORY_CAP: usize = 512;

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
    pub last_error: Option<String>,
    #[allow(dead_code)]
    pub tx: mpsc::UnboundedSender<AppEvent>,
}

#[derive(Debug, Clone, Copy)]
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
            last_error: None,
            tx,
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
                            editor.mode = EditorMode::Entering {
                                card_type: *kind,
                                buffer: String::new(),
                            };
                            return;
                        }
                    }
                }
                if matches!(k.code, KeyCode::Esc) {
                    editor.mode = EditorMode::Browse;
                }
                return;
            }
            EditorMode::Entering { card_type, buffer } => {
                match k.code {
                    KeyCode::Esc => editor.mode = EditorMode::Browse,
                    KeyCode::Backspace => {
                        buffer.pop();
                    }
                    KeyCode::Char(c) => buffer.push(c),
                    KeyCode::Enter => {
                        let kind = *card_type;
                        let buf = buffer.clone();
                        editor.mode = EditorMode::Browse;
                        let default_alias = self.instances.runtimes.keys().next().cloned();
                        let kind_opt = build_card_kind(kind, &buf, default_alias.as_deref());
                        let Some(dash) = self.dashboards.get_mut(dash_idx) else {
                            return;
                        };
                        let Some(editor2) = self.editor.as_mut() else {
                            return;
                        };
                        editor2.snapshot(dash);
                        if let Some(card_kind) = kind_opt {
                            editor2.add_card(dash, card_kind);
                        } else {
                            self.last_error = Some("invalid card spec".into());
                        }
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
            KeyCode::Char('d') => {
                if editor.selected_card.is_some() {
                    editor.mode = EditorMode::ConfirmDelete;
                }
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
                            self.last_error = Some(format!("saved {}", p.display()));
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

    fn create_new_dashboard(&mut self) {
        let n = self.dashboards.len() + 1;
        let dash = crate::dashboard::Dashboard {
            name: format!("Dashboard {n}"),
            grid: crate::dashboard::Grid { cols: 12, rows: 8 },
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
        self.last_error = Some(format!("new dashboard #{n} — press 's' to save"));
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
            MouseEventKind::Drag(MouseButton::Left) => {
                if editor.selected_card.is_some() {
                    editor.snapshot(dash);
                    editor.move_selected_to_cursor(dash);
                }
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
            Screen::Dashboard { idx, selected_card } => {
                let Some(dash) = self.dashboards.get(*idx) else {
                    return;
                };
                let Some(card) = dash.cards.get(*selected_card) else {
                    return;
                };
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
        let Screen::Dashboard { idx, selected_card } = &mut self.screen else {
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
            }
            AppEvent::HaEntityUpdated { instance, state } => {
                if let Some(rt) = self.instances.get_mut(&instance) {
                    rt.states.insert(state.entity_id.clone(), state.clone());
                    rt.last_update = Some(std::time::Instant::now());
                }
                self.record_history(&instance, &state.entity_id, &state.state);
            }
            AppEvent::HaServiceError { instance, error } => {
                self.last_error = Some(format!("{instance}: {error}"));
            }
        }
    }
}

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
            title: None,
        },
        CardTypeStub::Sparkline => CardKind::Sparkline {
            instance,
            entity,
            window: "1h".into(),
            title: None,
        },
        CardTypeStub::Text => unreachable!(),
    })
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
    mut terminal: DefaultTerminal,
    config_path: Option<PathBuf>,
    dashboards_path: Option<PathBuf>,
) -> Result<()> {
    info!("starting ha-tui");
    // Enable mouse capture (crossterm)
    crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture)?;

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

    let initial = terminal.size().unwrap_or_default();
    app.last_terminal_size = (initial.width, initial.height);
    terminal.draw(|f| ui::draw(f, &app))?;

    let result: Result<()> = async {
        loop {
            tokio::select! {
                Some(Ok(ev)) = term_events.next() => app.handle_term(ev),
                Some(ev) = rx.recv()              => app.handle_app(ev),
                _ = tick.tick()                   => {}
            }
            if app.should_quit {
                break;
            }
            terminal.draw(|f| ui::draw(f, &app))?;
        }
        Ok(())
    }
    .await;

    let _ = crossterm::execute!(std::io::stdout(), crossterm::event::DisableMouseCapture);
    info!("ha-tui exiting cleanly");
    result
}
