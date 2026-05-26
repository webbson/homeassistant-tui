use ratatui::layout::Rect;
#[allow(unused_imports)]
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};
use ratatui::Frame;

/// `update`: `Some((version, upgrade_cmd))` when a newer release is available.
pub fn render(f: &mut Frame, area: Rect, update: Option<(&str, &str)>) {
    let base_h: u16 = 22;
    let update_h: u16 = if update.is_some() { 4 } else { 0 };
    let w = 64u16.min(area.width.saturating_sub(4));
    let h = (base_h + update_h).min(area.height.saturating_sub(2));
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let r = Rect {
        x,
        y,
        width: w,
        height: h,
    };
    f.render_widget(Clear, r);

    let mut entries: Vec<(&str, &str)> = vec![
        ("Esc", "quit (or close current overlay/editor)"),
        ("?", "toggle this help"),
        ("E", "entity search modal"),
        ("i", "instance list modal"),
        ("1..9", "jump to dashboard N"),
        ("n", "new dashboard (opens editor)"),
        ("e", "edit current dashboard"),
        ("", ""),
        ("Entity search", ""),
        ("type", "fuzzy filter on entity_id"),
        ("↑/↓ PgUp/PgDn", "navigate"),
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

    // Owned strings for the update section so they outlive the match.
    let version_line;
    let cmd_line;
    if let Some((ver, cmd)) = update {
        version_line = format!("↑ v{ver} available");
        cmd_line = format!("run: {cmd}");
        entries.push(("", ""));
        entries.push(("Update", ""));
        entries.push((&version_line, ""));
        entries.push((&cmd_line, ""));
    }

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
