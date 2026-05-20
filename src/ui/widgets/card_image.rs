use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::StatefulImage;

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
    let border_style = if selected {
        Style::new().fg(color).bold()
    } else {
        Style::new().fg(color)
    };
    let block = Block::bordered()
        .title(format!(" {title} "))
        .border_style(border_style);
    let inner = block.inner(area);

    if let Some(err) = error {
        f.render_widget(block, area);
        f.render_widget(
            Paragraph::new(format!("Image unavailable\n{err}")).style(Style::new().fg(Color::Red)),
            inner,
        );
        return;
    }
    if let Some(p) = protocol {
        // Render image FIRST so the kitty/iterm2 placeholder cells aren't
        // touched by the block's border draw. Then draw the block on top —
        // Block::bordered only writes the edge cells, so the inner image
        // cells stay intact.
        f.render_stateful_widget(StatefulImage::default(), inner, p);
        f.render_widget(block, area);
    } else {
        f.render_widget(block, area);
        f.render_widget(
            Paragraph::new("(loading…)").style(Style::new().fg(Color::DarkGray)),
            inner,
        );
    }
}
