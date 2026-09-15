use std::cell::{Cell, RefCell};
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use ratatui::style::{Color, Modifier, Style};

/// Available themes inspired by Deezer's official app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeId {
    Omarchy,
    Crimson,
    Emerald,
    Amber,
    Magenta,
    Halloween,
    DarkPurple,
    DarkOrange,
    DarkPink,
    DarkRed,
    DarkYellow,
    DarkBlue,
}

impl ThemeId {
    pub const ALL: &[ThemeId] = &[
        ThemeId::Omarchy,
        ThemeId::Crimson,
        ThemeId::Emerald,
        ThemeId::Amber,
        ThemeId::Magenta,
        ThemeId::Halloween,
        ThemeId::DarkPurple,
        ThemeId::DarkOrange,
        ThemeId::DarkPink,
        ThemeId::DarkRed,
        ThemeId::DarkYellow,
        ThemeId::DarkBlue,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ThemeId::Omarchy => "System (Omarchy)",
            ThemeId::Crimson => "Crimson",
            ThemeId::Emerald => "Emerald",
            ThemeId::Amber => "Amber",
            ThemeId::Magenta => "Magenta",
            ThemeId::Halloween => "Halloween",
            ThemeId::DarkPurple => "Dark Purple",
            ThemeId::DarkOrange => "Dark Orange",
            ThemeId::DarkPink => "Dark Pink",
            ThemeId::DarkRed => "Dark Red",
            ThemeId::DarkYellow => "Dark Yellow",
            ThemeId::DarkBlue => "Dark Blue",
        }
    }

    /// Serialization key for config persistence.
    pub fn as_str(self) -> &'static str {
        match self {
            ThemeId::Omarchy => "omarchy",
            ThemeId::Crimson => "crimson",
            ThemeId::Emerald => "emerald",
            ThemeId::Amber => "amber",
            ThemeId::Magenta => "magenta",
            ThemeId::Halloween => "halloween",
            ThemeId::DarkPurple => "dark_purple",
            ThemeId::DarkOrange => "dark_orange",
            ThemeId::DarkPink => "dark_pink",
            ThemeId::DarkRed => "dark_red",
            ThemeId::DarkYellow => "dark_yellow",
            ThemeId::DarkBlue => "dark_blue",
        }
    }

    /// Parse from config string. Returns None for unknown values.
    pub fn from_str(s: &str) -> Option<ThemeId> {
        match s {
            "omarchy" => Some(ThemeId::Omarchy),
            "crimson" => Some(ThemeId::Crimson),
            "emerald" => Some(ThemeId::Emerald),
            "amber" => Some(ThemeId::Amber),
            "magenta" => Some(ThemeId::Magenta),
            "halloween" => Some(ThemeId::Halloween),
            "dark_purple" => Some(ThemeId::DarkPurple),
            "dark_orange" => Some(ThemeId::DarkOrange),
            "dark_pink" => Some(ThemeId::DarkPink),
            "dark_red" => Some(ThemeId::DarkRed),
            "dark_yellow" => Some(ThemeId::DarkYellow),
            "dark_blue" => Some(ThemeId::DarkBlue),
            _ => None,
        }
    }

    /// Themes offered in the picker: `ALL`, minus Omarchy when no usable
    /// Omarchy palette was found by [`Theme::detect_omarchy`].
    pub fn available() -> &'static [ThemeId] {
        if OMARCHY_AVAILABLE.with(Cell::get) {
            Self::ALL
        } else {
            // Omarchy is the first entry of `ALL`.
            &Self::ALL[1..]
        }
    }
}

thread_local! {
    static CURRENT_THEME: Cell<ThemeId> = const { Cell::new(ThemeId::Omarchy) };
    /// Whether a usable Omarchy `colors.toml` was found at the last detection.
    static OMARCHY_AVAILABLE: Cell<bool> = const { Cell::new(false) };
    static OMARCHY_PALETTE: RefCell<Option<Palette>> = const { RefCell::new(None) };
    static OMARCHY_MODIFIED: RefCell<Option<SystemTime>> = const { RefCell::new(None) };
    /// Background transparency: 0 = fully opaque, 100 = fully transparent (terminal default bg).
    static BG_TRANSPARENCY: Cell<u8> = const { Cell::new(0) };
}

#[derive(Clone, Copy)]
struct Palette {
    primary: Color,
    secondary: Color,
    bg: Color,
    surface: Color,
    border: Color,
    text: Color,
    dim: Color,
    /// Text drawn on a `primary` fill (selected rows, notifications).
    on_primary: Color,
    /// Text drawn on a `secondary` fill (Flow chip).
    on_secondary: Color,
}

fn omarchy_colors_path() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".local/state/omarchy/current/theme/colors.toml"))
}

fn parse_hex_color(value: &str) -> Option<Color> {
    let hex = value.trim().trim_matches('"').trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}

/// WCAG AA minimum contrast ratio for normal-size text.
const MIN_TEXT_CONTRAST: f64 = 4.5;

fn load_omarchy_palette() -> Option<Palette> {
    let path = omarchy_colors_path()?;
    palette_from_colors_toml(&fs::read_to_string(path).ok()?)
}

/// Map an Omarchy `colors.toml` onto the app palette. Omarchy themes are tuned for
/// other apps, so every text color is nudged until it stays readable here.
fn palette_from_colors_toml(contents: &str) -> Option<Palette> {
    let color = |name: &str| {
        contents.lines().find_map(|line| {
            let (key, value) = line.split_once('=')?;
            (key.trim() == name)
                .then(|| parse_hex_color(value))
                .flatten()
        })
    };
    let bg = color("background")?;
    let surface = color("lighter_background")?;
    // Text sits on both the main background and surface panels (player bar, popups).
    let readable = |fg: Color| ensure_contrast(fg, &[bg, surface], MIN_TEXT_CONTRAST);
    let text = readable(color("foreground")?);
    let primary = readable(color("accent")?);
    // `selection` is a background shade, too dark for the shortcut keys drawn in
    // `secondary`; `magenta` exists in every Omarchy theme and reads as an accent.
    let secondary = readable(color("magenta")?);
    let fill_text = [bg, text, color("bright_foreground").unwrap_or(text)];
    Some(Palette {
        primary,
        secondary,
        bg,
        surface,
        border: color("muted")?,
        text,
        dim: readable(color("dark_foreground")?),
        on_primary: text_on(primary, &fill_text),
        on_secondary: text_on(secondary, &fill_text),
    })
}

fn relative_luminance(color: Color) -> f64 {
    let Color::Rgb(r, g, b) = color else {
        return 0.0;
    };
    let channel = |value: u8| {
        let c = f64::from(value) / 255.0;
        if c <= 0.039_28 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}

/// WCAG contrast ratio, from 1.0 (identical) to 21.0 (black on white).
fn contrast_ratio(a: Color, b: Color) -> f64 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// Blend `fg` toward white or black, whichever the backgrounds favor, just far
/// enough to reach `min` contrast against every one of them.
fn ensure_contrast(fg: Color, backgrounds: &[Color], min: f64) -> Color {
    let worst = |c: Color| {
        backgrounds
            .iter()
            .map(|bg| contrast_ratio(c, *bg))
            .fold(f64::INFINITY, f64::min)
    };
    let Color::Rgb(r, g, b) = fg else {
        return fg;
    };
    if worst(fg) >= min {
        return fg;
    }
    let target: u8 = if worst(Color::Rgb(255, 255, 255)) >= worst(Color::Rgb(0, 0, 0)) {
        255
    } else {
        0
    };
    let mix = |value: u8, t: f64| {
        (f64::from(value) + (f64::from(target) - f64::from(value)) * t).round() as u8
    };
    (1..=20)
        .map(|step| {
            let t = f64::from(step) / 20.0;
            Color::Rgb(mix(r, t), mix(g, t), mix(b, t))
        })
        .find(|c| worst(*c) >= min)
        .unwrap_or(Color::Rgb(target, target, target))
}

/// The candidate with the most contrast on `fill`, pushed to the text minimum if needed.
fn text_on(fill: Color, candidates: &[Color]) -> Color {
    let best = candidates
        .iter()
        .copied()
        .max_by(|a, b| contrast_ratio(*a, fill).total_cmp(&contrast_ratio(*b, fill)))
        .unwrap_or(fill);
    ensure_contrast(best, &[fill], MIN_TEXT_CONTRAST)
}

/// Deezer-inspired color palette with dynamic theme support.
pub struct Theme;

impl Theme {
    /// Check for a usable Omarchy palette and remember the result for
    /// [`ThemeId::available`]. Reads the file, so call it on startup and when
    /// the theme picker opens rather than every frame.
    pub fn detect_omarchy() -> bool {
        let available = load_omarchy_palette().is_some();
        OMARCHY_AVAILABLE.with(|a| a.set(available));
        available
    }

    /// Set the active theme.
    pub fn set(id: ThemeId) {
        CURRENT_THEME.with(|c| c.set(id));
        if id == ThemeId::Omarchy {
            OMARCHY_MODIFIED.with(|modified| *modified.borrow_mut() = None);
            Self::refresh_omarchy();
        }
    }

    /// Refresh the generated Omarchy palette when `omarchy theme set` swaps it.
    /// Calling this every render is cheap: the file is only read after its mtime changes.
    pub fn refresh_omarchy() {
        if Self::current() != ThemeId::Omarchy {
            return;
        }
        let Some(path) = omarchy_colors_path() else {
            return;
        };
        let modified = fs::metadata(&path)
            .ok()
            .and_then(|meta| meta.modified().ok());
        let unchanged = OMARCHY_MODIFIED.with(|known| *known.borrow() == modified);
        if unchanged {
            return;
        }
        if let Some(palette) = load_omarchy_palette() {
            OMARCHY_PALETTE.with(|stored| *stored.borrow_mut() = Some(palette));
            OMARCHY_MODIFIED.with(|known| *known.borrow_mut() = modified);
        }
    }

    fn omarchy_palette() -> Option<Palette> {
        (Self::current() == ThemeId::Omarchy)
            .then(|| OMARCHY_PALETTE.with(|palette| *palette.borrow()))
            .flatten()
    }

    /// Get the active theme id.
    pub fn current() -> ThemeId {
        CURRENT_THEME.with(|c| c.get())
    }

    /// Set the stored transparency value (the toggle writes 0 or 100).
    /// Any 0–100 is accepted so configs saved by older (stepped) versions keep
    /// deserializing; the value is interpreted as a toggle by `is_transparent`.
    pub fn set_transparency(transparency: u8) {
        BG_TRANSPARENCY.with(|c| c.set(transparency.min(100)));
    }

    /// Get the stored transparency value (0–100).
    pub fn transparency() -> u8 {
        BG_TRANSPARENCY.with(|c| c.get())
    }

    /// Whether background transparency is on. Transparency is a toggle: only a
    /// fully transparent background (`Color::Reset`) actually reveals the
    /// terminal — intermediate values just dim, so they aren't offered.
    /// Any stored value ≥ 50 counts as on, mapping older stepped configs onto
    /// the toggle.
    pub fn is_transparent() -> bool {
        Self::transparency() >= 50
    }

    /// Effective background color for the transparency toggle.
    /// Off: solid `bg()`. On: `Color::Reset` (terminal transparent background).
    pub fn bg_with_opacity() -> Color {
        if Self::is_transparent() {
            Color::Reset
        } else {
            Self::bg()
        }
    }

    // ── Color accessors ──────────────────────────────────────────

    pub fn primary() -> Color {
        if let Some(palette) = Self::omarchy_palette() {
            return palette.primary;
        }
        match Self::current() {
            ThemeId::Omarchy => Color::Rgb(162, 0, 255),
            ThemeId::Crimson => Color::Rgb(220, 40, 60),
            ThemeId::Emerald => Color::Rgb(46, 204, 113),
            ThemeId::Amber => Color::Rgb(240, 165, 0),
            ThemeId::Magenta => Color::Rgb(255, 0, 200),
            // Halloween: orange-amber primary on purple background
            ThemeId::Halloween => Color::Rgb(255, 140, 0),
            ThemeId::DarkPurple => Color::Rgb(162, 0, 255),
            ThemeId::DarkOrange => Color::Rgb(255, 140, 0),
            ThemeId::DarkPink => Color::Rgb(255, 20, 147),
            ThemeId::DarkRed => Color::Rgb(220, 20, 60),
            ThemeId::DarkYellow => Color::Rgb(240, 200, 0),
            ThemeId::DarkBlue => Color::Rgb(60, 120, 255),
        }
    }

    pub fn secondary() -> Color {
        if let Some(palette) = Self::omarchy_palette() {
            return palette.secondary;
        }
        match Self::current() {
            ThemeId::Omarchy => Color::Rgb(239, 84, 105),
            ThemeId::Crimson => Color::Rgb(255, 107, 107),
            ThemeId::Emerald => Color::Rgb(0, 210, 255),
            ThemeId::Amber => Color::Rgb(255, 209, 102),
            ThemeId::Magenta => Color::Rgb(255, 105, 180),
            // Halloween: purple secondary to complement the orange
            ThemeId::Halloween => Color::Rgb(162, 0, 255),
            ThemeId::DarkPurple => Color::Rgb(239, 84, 105),
            ThemeId::DarkOrange => Color::Rgb(255, 165, 0),
            ThemeId::DarkPink => Color::Rgb(255, 105, 180),
            ThemeId::DarkRed => Color::Rgb(255, 68, 68),
            ThemeId::DarkYellow => Color::Rgb(255, 230, 100),
            ThemeId::DarkBlue => Color::Rgb(100, 160, 255),
        }
    }

    pub fn success() -> Color {
        Color::Rgb(0, 204, 0)
    }

    pub fn bg() -> Color {
        if let Some(palette) = Self::omarchy_palette() {
            return palette.bg;
        }
        match Self::current() {
            ThemeId::Omarchy => Color::Rgb(18, 18, 18),
            ThemeId::Crimson => Color::Rgb(46, 10, 10),
            ThemeId::Emerald => Color::Rgb(10, 36, 24),
            ThemeId::Amber => Color::Rgb(36, 22, 8),
            ThemeId::Magenta => Color::Rgb(36, 10, 30),
            // Halloween: deep dark purple background
            ThemeId::Halloween => Color::Rgb(30, 10, 50),
            ThemeId::DarkPurple => Color::Rgb(18, 18, 18),
            ThemeId::DarkOrange => Color::Rgb(18, 18, 18),
            ThemeId::DarkPink => Color::Rgb(18, 18, 18),
            ThemeId::DarkRed => Color::Rgb(18, 18, 18),
            ThemeId::DarkYellow => Color::Rgb(18, 18, 18),
            ThemeId::DarkBlue => Color::Rgb(14, 16, 22),
        }
    }

    pub fn surface() -> Color {
        if let Some(palette) = Self::omarchy_palette() {
            return palette.surface;
        }
        match Self::current() {
            ThemeId::Omarchy => Color::Rgb(30, 30, 30),
            ThemeId::Crimson => Color::Rgb(58, 20, 20),
            ThemeId::Emerald => Color::Rgb(20, 48, 34),
            ThemeId::Amber => Color::Rgb(50, 34, 16),
            ThemeId::Magenta => Color::Rgb(50, 20, 42),
            // Halloween: slightly lighter purple surface
            ThemeId::Halloween => Color::Rgb(42, 18, 65),
            ThemeId::DarkPurple => Color::Rgb(30, 30, 30),
            ThemeId::DarkOrange => Color::Rgb(30, 30, 30),
            ThemeId::DarkPink => Color::Rgb(30, 30, 30),
            ThemeId::DarkRed => Color::Rgb(30, 30, 30),
            ThemeId::DarkYellow => Color::Rgb(30, 30, 30),
            ThemeId::DarkBlue => Color::Rgb(22, 26, 36),
        }
    }

    pub fn border_color() -> Color {
        if let Some(palette) = Self::omarchy_palette() {
            return palette.border;
        }
        match Self::current() {
            ThemeId::Omarchy => Color::Rgb(60, 60, 60),
            ThemeId::Crimson => Color::Rgb(90, 58, 58),
            ThemeId::Emerald => Color::Rgb(58, 90, 74),
            ThemeId::Amber => Color::Rgb(90, 74, 58),
            ThemeId::Magenta => Color::Rgb(90, 58, 80),
            ThemeId::Halloween => Color::Rgb(80, 50, 90),
            ThemeId::DarkPurple
            | ThemeId::DarkOrange
            | ThemeId::DarkPink
            | ThemeId::DarkRed
            | ThemeId::DarkYellow => Color::Rgb(60, 60, 60),
            ThemeId::DarkBlue => Color::Rgb(50, 55, 75),
        }
    }

    pub fn border_focused_color() -> Color {
        Self::primary()
    }

    pub fn text_color() -> Color {
        if let Some(palette) = Self::omarchy_palette() {
            return palette.text;
        }
        Color::Rgb(230, 230, 230)
    }

    pub fn text_dim_color() -> Color {
        if let Some(palette) = Self::omarchy_palette() {
            return palette.dim;
        }
        Color::Rgb(140, 140, 140)
    }

    pub fn progress_fill() -> Color {
        Self::primary()
    }

    pub fn progress_bg() -> Color {
        Color::Rgb(50, 50, 50)
    }

    /// Color along the visualizer's theme gradient: secondary at the base,
    /// primary at the peak. Every built-in and Omarchy palette uses RGB;
    /// retaining an endpoint is a safe fallback for terminal indexed colors.
    pub fn visualizer_gradient(position: f32) -> Color {
        let position = position.clamp(0.0, 1.0);
        match (Self::secondary(), Self::primary()) {
            (Color::Rgb(ar, ag, ab), Color::Rgb(br, bg, bb)) => {
                let mix = |a: u8, b: u8| {
                    (f32::from(a) + (f32::from(b) - f32::from(a)) * position).round() as u8
                };
                Color::Rgb(mix(ar, br), mix(ag, bg), mix(ab, bb))
            }
            (secondary, primary) => {
                if position < 0.5 {
                    secondary
                } else {
                    primary
                }
            }
        }
    }

    pub fn tab_active_color() -> Color {
        Self::primary()
    }

    pub fn tab_inactive_color() -> Color {
        Self::text_color()
    }

    // ── Style helpers (unchanged API) ────────────────────────────

    pub fn title() -> Style {
        Style::default()
            .fg(Self::text_color())
            .add_modifier(Modifier::BOLD)
    }

    pub fn text() -> Style {
        Style::default().fg(Self::text_color())
    }

    pub fn dim() -> Style {
        Style::default().fg(Self::text_dim_color())
    }

    pub fn highlight() -> Style {
        Style::default()
            .fg(Self::omarchy_palette().map_or_else(Self::text_color, |p| p.on_primary))
            .bg(Self::primary())
            .add_modifier(Modifier::BOLD)
    }

    pub fn border() -> Style {
        Style::default().fg(Self::border_color())
    }

    pub fn border_focused() -> Style {
        Style::default().fg(Self::border_focused_color())
    }

    pub fn tab_active() -> Style {
        Style::default()
            .fg(Self::tab_active_color())
            .add_modifier(Modifier::BOLD)
    }

    pub fn tab_inactive() -> Style {
        Style::default().fg(Self::tab_inactive_color())
    }

    /// Shortcut-key "chip": secondary-colored text on a white pad (no brackets).
    pub fn shortcut_key() -> Style {
        Style::default()
            .fg(Self::secondary())
            // .bg(Color::Rgb(10, 10, 10))
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    }

    /// Transient status notification: white text on the theme's primary color,
    /// rendered on the last row of the content area, above the player bar.
    pub fn notification() -> Style {
        Style::default()
            .fg(Self::omarchy_palette().map_or(Color::White, |p| p.on_primary))
            .bg(Self::primary())
            .add_modifier(Modifier::BOLD)
    }

    /// Flow's chip: white text on a secondary background, covering both the key
    /// and its label, so the feature stands out.
    pub fn shortcut_flow_chip() -> Style {
        // Black text on themes whose secondary (the chip bg) is light; white
        // otherwise, for readable contrast.
        let fg = if let Some(palette) = Self::omarchy_palette() {
            palette.on_secondary
        } else {
            match Self::current() {
                ThemeId::Omarchy
                | ThemeId::Emerald
                | ThemeId::Amber
                | ThemeId::DarkOrange
                | ThemeId::DarkYellow => Color::Rgb(30, 30, 30),
                _ => Self::text_color(),
            }
        };
        Style::default()
            .fg(fg)
            .bg(Self::secondary())
            .add_modifier(Modifier::BOLD)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKYO_NIGHT: &str = r##"
accent = "#7aa2f7"
muted = "#414868"
background = "#1a1b26"
lighter_background = "#24283b"
foreground = "#a9b1d6"
dark_foreground = "#565f89"
bright_foreground = "#c0caf5"
magenta = "#ad8ee6"
"##;

    const CATPPUCCIN_LATTE: &str = r##"
accent = "#1e66f5"
muted = "#acb0be"
background = "#eff1f5"
lighter_background = "#dce0e8"
foreground = "#4c4f69"
dark_foreground = "#9ca0b0"
bright_foreground = "#4c4f69"
magenta = "#ea76cb"
"##;

    fn contrast_failures(contents: &str) -> Vec<String> {
        let p = palette_from_colors_toml(contents).expect("palette parses");
        [
            ("text", p.text, p.bg),
            ("text on surface", p.text, p.surface),
            ("dim", p.dim, p.bg),
            ("dim on surface", p.dim, p.surface),
            ("primary", p.primary, p.bg),
            ("secondary", p.secondary, p.bg),
            ("secondary on surface", p.secondary, p.surface),
            ("on_primary", p.on_primary, p.primary),
            ("on_secondary", p.on_secondary, p.secondary),
        ]
        .into_iter()
        .filter_map(|(name, fg, bg)| {
            let ratio = contrast_ratio(fg, bg);
            (ratio < MIN_TEXT_CONTRAST).then(|| format!("{name}: {ratio:.2}"))
        })
        .collect()
    }

    #[test]
    fn dark_omarchy_theme_is_readable() {
        assert_eq!(contrast_failures(TOKYO_NIGHT), Vec::<String>::new());
    }

    #[test]
    fn light_omarchy_theme_is_readable() {
        assert_eq!(contrast_failures(CATPPUCCIN_LATTE), Vec::<String>::new());
    }

    #[test]
    fn omarchy_hidden_unless_detected() {
        OMARCHY_AVAILABLE.with(|a| a.set(false));
        assert!(!ThemeId::available().contains(&ThemeId::Omarchy));
        assert_eq!(ThemeId::available().len(), ThemeId::ALL.len() - 1);
        OMARCHY_AVAILABLE.with(|a| a.set(true));
        assert_eq!(ThemeId::available(), ThemeId::ALL);
    }

    #[test]
    fn readable_color_is_left_unchanged() {
        let fg = Color::Rgb(122, 162, 247);
        assert_eq!(
            ensure_contrast(fg, &[Color::Rgb(26, 27, 38)], MIN_TEXT_CONTRAST),
            fg
        );
    }
}
