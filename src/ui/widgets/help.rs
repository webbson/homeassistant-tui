use ratatui::layout::Rect;
#[allow(unused_imports)]
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};
use ratatui::Frame;

pub fn render(f: &mut Frame, area: Rect) {
    let w = 64u16.min(area.width.saturating_sub(4));
    let h = 22u16.min(area.height.saturating_sub(2));
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let r = Rect {
        x,
        y,
        width: w,
        height: h,
    };
    f.render_widget(Clear, r);

    let entries: Vec<(&str, &str)> = vec![
        ("q / Esc", "quit"),
        ("?", "toggle this help"),
        ("E", "entity browser"),
        ("i", "instances screen"),
        ("1..9", "jump to dashboard N"),
        ("n", "new dashboard (opens editor)"),
        ("e", "edit current dashboard"),
        ("", ""),
        ("Entity browser", ""),
        ("j/k ↑/↓", "navigate"),
        ("PgUp/PgDn", "jump 10"),
        ("f", "cycle instance filter"),
        ("Enter", "default action (toggle, etc.)"),
        ("", ""),
        ("Editor", ""),
        ("hjkl", "move cursor"),
        ("HJKL", "resize selected"),
        ("Enter", "select / place"),
        ("a / d / u / s", "add / delete / undo / save"),
        ("m", "menu (card or dashboard settings)"),
    ];
    let lines: Vec<Line<'_>> = entries
        .into_iter()
        .map(|(k, v)| {
            if v.is_empty() && k.is_empty() {
                Line::raw("")
            } else if v.is_empty() {
                Line::from(Span::styled(
                    k.to_string(),
                    Style::new().bold().underlined(),
                ))
            } else {
                Line::from(vec![
                    Span::styled(format!("  {k:<16}"), Style::new().bold()),
                    Span::raw(v.to_string()),
                ])
            }
        })
        .collect();
    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(" Help ")),
        r,
    );
}
