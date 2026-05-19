use ratatui::layout::Rect;
use ratatui::widgets::{Block, Paragraph, Wrap};
use ratatui::Frame;

use crate::ha::EntityState;

pub fn render(f: &mut Frame, area: Rect, sel: Option<&EntityState>) {
    let text = match sel {
        None => "no selection".to_string(),
        Some(s) => {
            let attrs = serde_json::to_string_pretty(&s.attributes)
                .unwrap_or_else(|_| "<unserializable>".into());
            format!(
                "{}\nstate: {}\nlast_changed: {}\nlast_updated: {}\n\nattributes:\n{}",
                s.entity_id,
                s.state,
                s.last_changed.as_deref().unwrap_or("-"),
                s.last_updated.as_deref().unwrap_or("-"),
                attrs
            )
        }
    };
    f.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .block(Block::bordered().title(" Detail ")),
        area,
    );
}
