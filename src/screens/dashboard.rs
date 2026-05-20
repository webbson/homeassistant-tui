use ratatui::layout::Rect;
#[allow(unused_imports)]
use ratatui::style::{Style, Stylize};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::dashboard::layout::cell_to_rect;
use crate::dashboard::{CardKind, Dashboard};
use crate::ui::widgets;

pub fn draw(
    f: &mut Frame,
    area: Rect,
    app: &App,
    idx: usize,
    selected_card: usize,
    sub_index: Option<usize>,
) {
    let Some(dash) = app.dashboards.get(idx) else {
        f.render_widget(
            Paragraph::new("no dashboard").block(Block::bordered()),
            area,
        );
        return;
    };
    draw_title(f, area, dash);
    let inner = Rect {
        x: area.x,
        y: area.y + 1,
        width: area.width,
        height: area.height.saturating_sub(1),
    };
    for (i, card) in dash.cards.iter().enumerate() {
        let rect = cell_to_rect(inner, dash.grid, card.pos);
        if rect.width < 3 || rect.height < 3 {
            continue;
        }
        let sel = i == selected_card;
        let sub = if sel { sub_index } else { None };
        render_card(f, rect, card, app, sel, sub);
    }
}

fn draw_title(f: &mut Frame, area: Rect, dash: &Dashboard) {
    let bar = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: 1,
    };
    f.render_widget(
        Paragraph::new(format!("◆ {}", dash.name)).style(Style::new().bold().dim()),
        bar,
    );
}

fn render_card(
    f: &mut Frame,
    rect: Rect,
    card: &crate::dashboard::Card,
    app: &App,
    selected: bool,
    sub_index: Option<usize>,
) {
    let title = card.title().to_string();
    match &card.kind {
        CardKind::Entity {
            instance,
            entity,
            ticker,
            ..
        } => {
            let s = app
                .instances
                .runtimes
                .get(instance)
                .and_then(|rt| rt.states.get(entity));
            widgets::card_entity::render(
                f,
                rect,
                &title,
                instance,
                s,
                card.color.as_deref(),
                &app.theme,
                selected,
                *ticker,
                app.ticker_offset,
                card.size,
            );
        }
        CardKind::Toggle {
            instance, entity, ..
        } => {
            let s = app
                .instances
                .runtimes
                .get(instance)
                .and_then(|rt| rt.states.get(entity));
            widgets::card_toggle::render(
                f,
                rect,
                &title,
                instance,
                s,
                card.color.as_deref(),
                &app.theme,
                selected,
                card.size,
            );
        }
        CardKind::Gauge {
            instance,
            entity,
            min,
            max,
            unit,
            ..
        } => {
            let s = app
                .instances
                .runtimes
                .get(instance)
                .and_then(|rt| rt.states.get(entity));
            widgets::card_gauge::render(
                f,
                rect,
                &title,
                instance,
                s,
                *min,
                *max,
                unit.as_deref(),
                card.color.as_deref(),
                &app.theme,
                selected,
            );
        }
        CardKind::Sparkline {
            instance,
            entity,
            window,
            ..
        } => {
            let key = (instance.clone(), entity.clone());
            let h = app.history.get(&key);
            widgets::card_sparkline::render(
                f,
                rect,
                &title,
                instance,
                h,
                window,
                card.color.as_deref(),
                &app.theme,
                selected,
            );
        }
        CardKind::Text { markdown, .. } => {
            widgets::card_text::render(f, rect, &title, markdown, card.color.as_deref(), selected);
        }
        CardKind::EntityList {
            instance, entities, ..
        } => {
            let rt = app.instances.runtimes.get(instance);
            widgets::card_entity_list::render(
                f,
                rect,
                &title,
                instance,
                entities,
                rt,
                card.color.as_deref(),
                &app.theme,
                selected,
                sub_index,
                false,
            );
        }
        CardKind::FilteredEntityList {
            instance,
            query,
            hide_state,
            ..
        } => {
            let rt = app.instances.runtimes.get(instance);
            let entities = crate::dashboard::query::resolve(rt, query);
            widgets::card_entity_list::render(
                f,
                rect,
                &title,
                instance,
                &entities,
                rt,
                card.color.as_deref(),
                &app.theme,
                selected,
                sub_index,
                *hide_state,
            );
        }
    }
}
