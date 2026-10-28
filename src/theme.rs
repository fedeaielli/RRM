use crate::config::Config;
use ratatui::style::Color;

/// Editable palette slots, in the order shown in the Theme tab.
pub const SLOT_NAMES: [&str; 8] = [
    "accent",
    "good",
    "warn",
    "crit",
    "rx",
    "tx",
    "banner",
    "background",
];

/// Slots in the order the Theme tab shows them: background first.
pub const SLOT_ORDER: [usize; 8] = [7, 0, 1, 2, 3, 4, 5, 6];

/// Index of the `background` slot in `SLOT_NAMES`.
pub const BACKGROUND_INDEX: usize = 7;

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
    pub background: Color,
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
            background: NEAR_BLACK,
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
                .and_then(parse_hex)
                .unwrap_or(Color::Rgb(r, g, b)),
            good: config
                .colors
                .good
                .as_deref()
                .and_then(parse_hex)
                .unwrap_or(defaults.good),
            warn: config
                .colors
                .warn
                .as_deref()
                .and_then(parse_hex)
                .unwrap_or(defaults.warn),
            crit: config
                .colors
                .crit
                .as_deref()
                .and_then(parse_hex)
                .unwrap_or(defaults.crit),
            rx: config
                .colors
                .rx
                .as_deref()
                .and_then(parse_hex)
                .unwrap_or(defaults.rx),
            tx: config
                .colors
                .tx
                .as_deref()
                .and_then(parse_hex)
                .unwrap_or(defaults.tx),
            banner: config
                .colors
                .banner
                .as_deref()
                .and_then(parse_hex)
                .unwrap_or(defaults.banner),
            // Migrate the old pure-black default, transparent on kitty.
            background: config
                .colors
                .background
                .as_deref()
                .and_then(parse_hex)
                .map(|color| {
                    if color == Color::Rgb(0, 0, 0) {
                        NEAR_BLACK
                    } else {
                        color
                    }
                })
                .unwrap_or(defaults.background),
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
            6 => self.banner,
            _ => self.background,
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
            6 => self.banner = color,
            7 => self.background = color,
            _ => {}
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
        config.colors.background = Some(to_hex(self.background));
    }
}

/// Colors offered when cycling a slot with left/right.
pub const PRESETS: [Color; 13] = [
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
    NEAR_BLACK,
];

/// Pure black matches kitty's default background, which `background_opacity`
/// makes transparent; one step off keeps the panel opaque.
pub const NEAR_BLACK: Color = Color::Rgb(10, 10, 10);

/// Transparent background: nothing is painted, the terminal shows through.
pub const TRANSPARENT: Color = Color::Reset;

/// Next preset from `current`, wrapping around in both directions.
pub fn cycle(current: Color, delta: i32) -> Color {
    let len = PRESETS.len() as i32;
    let index = PRESETS
        .iter()
        .position(|color| *color == current)
        .unwrap_or(0) as i32;
    PRESETS[(index + delta).rem_euclid(len) as usize]
}

/// Background-only cycle: same presets, plus transparent past either end.
pub fn cycle_background(current: Color, delta: i32) -> Color {
    let last = PRESETS[PRESETS.len() - 1];
    if current == TRANSPARENT {
        return if delta > 0 {
            PRESETS[0]
        } else {
            PRESETS[PRESETS.len() - 1]
        };
    }
    if delta > 0 && current == last {
        return TRANSPARENT;
    }
    if delta < 0 && current == PRESETS[0] {
        return TRANSPARENT;
    }
    cycle(current, delta)
}

pub fn to_hex(color: Color) -> String {
    match color {
        Color::Rgb(r, g, b) => format!("#{r:02X}{g:02X}{b:02X}"),
        TRANSPARENT => "transparent".to_string(),
        _ => "#FFFFFF".to_string(),
    }
}

pub fn parse_hex(value: &str) -> Option<Color> {
    if value.trim() == "transparent" {
        return Some(TRANSPARENT);
    }
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
    use super::{
        NEAR_BLACK, PRESETS, Palette, TRANSPARENT, cycle, cycle_background, parse_hex, to_hex,
    };
    use ratatui::style::Color;

    #[test]
    fn cycle_wraps_and_matches_presets() {
        assert_eq!(cycle(PRESETS[0], 1), PRESETS[1]);
        assert_eq!(cycle(PRESETS[0], -1), PRESETS[PRESETS.len() - 1]);
        assert_eq!(cycle(Color::Rgb(1, 2, 3), 1), PRESETS[1]);
    }

    #[test]
    fn parse_hex_accepts_colors_and_transparent() {
        assert_eq!(parse_hex("#00BFFF"), Some(Color::Rgb(0, 191, 255)));
        assert_eq!(parse_hex("00bfff"), Some(Color::Rgb(0, 191, 255)));
        assert_eq!(parse_hex("transparent"), Some(TRANSPARENT));
        assert_eq!(parse_hex("#00BFF"), None);
        assert_eq!(parse_hex("#GGGGGG"), None);
        assert_eq!(parse_hex(""), None);
    }

    #[test]
    fn background_cycle_reaches_transparent() {
        let last = PRESETS[PRESETS.len() - 1];
        assert_eq!(cycle_background(last, 1), TRANSPARENT);
        assert_eq!(cycle_background(TRANSPARENT, 1), PRESETS[0]);
        assert_eq!(cycle_background(PRESETS[0], -1), TRANSPARENT);
        assert_eq!(cycle_background(TRANSPARENT, -1), last);
        assert_eq!(cycle_background(PRESETS[0], 1), PRESETS[1]);
    }

    #[test]
    fn transparent_roundtrips_through_config_text() {
        assert_eq!(to_hex(TRANSPARENT), "transparent");
        let mut config = crate::config::Config::default();
        config.colors.background = Some("transparent".to_string());
        assert_eq!(Palette::from_config(&config).background, TRANSPARENT);
    }

    #[test]
    fn pure_black_migrates_to_near_black() {
        let mut config = crate::config::Config::default();
        config.colors.background = Some("#000000".to_string());
        assert_eq!(Palette::from_config(&config).background, NEAR_BLACK);

        config.colors.background = Some("#123456".to_string());
        assert_eq!(
            Palette::from_config(&config).background,
            ratatui::style::Color::Rgb(0x12, 0x34, 0x56)
        );
    }

    #[test]
    fn slots_roundtrip_through_hex() {
        let color = Color::Rgb(12, 34, 56);
        for index in 0..super::SLOT_NAMES.len() {
            let mut palette = Palette::default();
            palette.set_slot(index, color);
            assert_eq!(palette.slot(index), color, "slot {index} was not set");
            for other in 0..super::SLOT_NAMES.len() {
                if other != index {
                    assert_eq!(
                        palette.slot(other),
                        Palette::default().slot(other),
                        "slot {index} overwrote slot {other}"
                    );
                }
            }
        }
        assert_eq!(to_hex(color), "#0C2238");
    }
}
