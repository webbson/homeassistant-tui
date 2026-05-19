use indexmap::IndexMap;
use ratatui::style::Color;

use crate::config::{Alias, Config};

/// Per-instance + semantic colors.
pub struct Theme {
    instance_colors: IndexMap<Alias, Color>,
}

impl Theme {
    /// Build theme from config: explicit `color: ...` overrides; otherwise cycle palette.
    pub fn from_config(cfg: &Config) -> Self {
        let palette: &[Color] = &[
            Color::Cyan,
            Color::Magenta,
            Color::Yellow,
            Color::Green,
            Color::Blue,
            Color::LightRed,
            Color::LightMagenta,
            Color::LightCyan,
        ];
        let mut map = IndexMap::new();
        let mut cycle = palette.iter().cycle();
        for inst in &cfg.instances {
            let color = inst
                .color
                .as_deref()
                .and_then(parse_color)
                .unwrap_or_else(|| *cycle.next().unwrap());
            map.insert(inst.alias.clone(), color);
        }
        Self {
            instance_colors: map,
        }
    }

    pub fn instance_color(&self, alias: &str) -> Color {
        self.instance_colors
            .get(alias)
            .copied()
            .unwrap_or(Color::White)
    }

    pub fn empty() -> Self {
        Self {
            instance_colors: IndexMap::new(),
        }
    }
}

fn parse_color(s: &str) -> Option<Color> {
    match s.to_ascii_lowercase().as_str() {
        "black" => Some(Color::Black),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "magenta" => Some(Color::Magenta),
        "cyan" => Some(Color::Cyan),
        "gray" | "grey" | "white" => Some(Color::Gray),
        "dark_gray" | "dark_grey" => Some(Color::DarkGray),
        "light_red" => Some(Color::LightRed),
        "light_green" => Some(Color::LightGreen),
        "light_yellow" => Some(Color::LightYellow),
        "light_blue" => Some(Color::LightBlue),
        "light_magenta" => Some(Color::LightMagenta),
        "light_cyan" => Some(Color::LightCyan),
        hex if hex.starts_with('#') && hex.len() == 7 => {
            let r = u8::from_str_radix(&hex[1..3], 16).ok()?;
            let g = u8::from_str_radix(&hex[3..5], 16).ok()?;
            let b = u8::from_str_radix(&hex[5..7], 16).ok()?;
            Some(Color::Rgb(r, g, b))
        }
        _ => None,
    }
}
