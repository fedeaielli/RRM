use crate::config::Config;
use ratatui::style::Color;

/// Editable palette slots, in the order shown in the Theme tab.
pub const SLOT_NAMES: [&str; 7] = ["accent", "good", "warn", "crit", "rx", "tx", "banner"];

/// Colors chosen in the Theme tab. `dim` and the gauge track stay fixed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub accent: Color,
    pub good: Color,
    pub warn: Color,
    pub crit: Color,
    pub rx: Color,
    pub tx: Color,
    pub banner: Color,
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            accent: Color::Rgb(0, 191, 255),
            good: Color::Rgb(100, 210, 130),
            warn: Color::Rgb(240, 180, 50),
            crit: Color::Rgb(220, 70, 70),
            rx: Color::Rgb(80, 180, 220),
            tx: Color::Rgb(180, 90, 220),
            banner: Color::Rgb(180, 90, 220),
        }
    }
}

impl Palette {
    /// Build the palette from config, falling back to the defaults per slot.
    pub fn from_config(config: &Config) -> Self {
        let defaults = Self::default();
        let (r, g, b) = config.accent_color_rgb();
        Self {
            accent: config
                .colors
                .accent
                .as_deref()
                .and_then(from_hex)
                .unwrap_or(Color::Rgb(r, g, b)),
            good: config
                .colors
                .good
                .as_deref()
                .and_then(from_hex)
                .unwrap_or(defaults.good),
            warn: config
                .colors
                .warn
                .as_deref()
                .and_then(from_hex)
                .unwrap_or(defaults.warn),
            crit: config
                .colors
                .crit
                .as_deref()
                .and_then(from_hex)
                .unwrap_or(defaults.crit),
            rx: config
                .colors
                .rx
                .as_deref()
                .and_then(from_hex)
                .unwrap_or(defaults.rx),
            tx: config
                .colors
                .tx
                .as_deref()
                .and_then(from_hex)
                .unwrap_or(defaults.tx),
            banner: config
                .colors
                .banner
                .as_deref()
                .and_then(from_hex)
                .unwrap_or(defaults.banner),
        }
    }

    pub fn slot(&self, index: usize) -> Color {
        match index {
            0 => self.accent,
            1 => self.good,
            2 => self.warn,
            3 => self.crit,
            4 => self.rx,
            5 => self.tx,
            _ => self.banner,
        }
    }

    pub fn set_slot(&mut self, index: usize, color: Color) {
        match index {
            0 => self.accent = color,
            1 => self.good = color,
            2 => self.warn = color,
            3 => self.crit = color,
            4 => self.rx = color,
            5 => self.tx = color,
            _ => self.banner = color,
        }
    }

    /// Compact load threshold: good below 60%, warning to 85%, critical above.
    pub fn threshold(&self, pct: f32) -> Color {
        if pct < 60.0 {
            self.good
        } else if pct <= 85.0 {
            self.warn
        } else {
            self.crit
        }
    }

    /// Disk usage threshold: good below 70%, warning to 85%, critical above.
    pub fn disk_usage(&self, pct: f32) -> Color {
        if pct < 70.0 {
            self.good
        } else if pct <= 85.0 {
            self.warn
        } else {
            self.crit
        }
    }

    /// Write the palette back into the config, ready to be saved.
    pub fn apply_to(&self, config: &mut Config) {
        config.display.accent_color = to_hex(self.accent);
        config.colors.accent = Some(to_hex(self.accent));
        config.colors.good = Some(to_hex(self.good));
        config.colors.warn = Some(to_hex(self.warn));
        config.colors.crit = Some(to_hex(self.crit));
        config.colors.rx = Some(to_hex(self.rx));
        config.colors.tx = Some(to_hex(self.tx));
        config.colors.banner = Some(to_hex(self.banner));
    }
}

/// Colors offered when cycling a slot with left/right.
pub const PRESETS: [Color; 12] = [
    Color::Rgb(0, 191, 255),
    Color::Rgb(100, 210, 130),
    Color::Rgb(240, 180, 50),
    Color::Rgb(220, 70, 70),
    Color::Rgb(80, 180, 220),
    Color::Rgb(180, 90, 220),
    Color::Rgb(255, 120, 180),
    Color::Rgb(120, 140, 255),
    Color::Rgb(200, 200, 200),
    Color::Rgb(255, 140, 60),
    Color::Rgb(80, 220, 200),
    Color::Rgb(240, 240, 120),
];

/// Next preset from `current`, wrapping around in both directions.
pub fn cycle(current: Color, delta: i32) -> Color {
    let len = PRESETS.len() as i32;
    let index = PRESETS
        .iter()
        .position(|color| *color == current)
        .unwrap_or(0) as i32;
    PRESETS[(index + delta).rem_euclid(len) as usize]
}

pub fn to_hex(color: Color) -> String {
    match color {
        Color::Rgb(r, g, b) => format!("#{r:02X}{g:02X}{b:02X}"),
        _ => "#FFFFFF".to_string(),
    }
}

fn from_hex(value: &str) -> Option<Color> {
    let hex = value.trim().strip_prefix('#').unwrap_or(value.trim());
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}

#[cfg(test)]
mod tests {
    use super::{PRESETS, Palette, cycle, to_hex};
    use ratatui::style::Color;

    #[test]
    fn cycle_wraps_and_matches_presets() {
        assert_eq!(cycle(PRESETS[0], 1), PRESETS[1]);
        assert_eq!(cycle(PRESETS[0], -1), PRESETS[PRESETS.len() - 1]);
        assert_eq!(cycle(Color::Rgb(1, 2, 3), 1), PRESETS[1]);
    }

    #[test]
    fn slots_roundtrip_through_hex() {
        let mut palette = Palette::default();
        let color = Color::Rgb(12, 34, 56);
        palette.set_slot(2, color);
        assert_eq!(palette.slot(2), color);
        assert_eq!(to_hex(color), "#0C2238");
    }
}
