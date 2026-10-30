use crate::app::{App, Tab};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph};

pub mod battery;
pub mod braille;
pub mod cpu;
pub mod disk;
pub mod fans;
pub mod gpu;
pub mod memory;
pub mod network;
pub mod processes;
pub mod system;
pub mod theme;

/// Secondary labels, units, separators and idle rows.
pub const DIM: Color = Color::Rgb(70, 70, 80);
/// Unfilled gauge track, dimmer than regular text.
pub const IDLE: Color = Color::Rgb(30, 30, 40);

/// Resolve the configured accent color.
pub fn accent(app: &App) -> Color {
    app.palette.accent
}

/// One filled cell per percent step, each colored by the level it represents.
pub fn segmented_bar(percent: f32, width: u16, color: impl Fn(f32) -> Color) -> Line<'static> {
    let width = usize::from(width);
    if width == 0 {
        return Line::default();
    }
    let filled = ((percent / 100.0).clamp(0.0, 1.0) * width as f32).round() as usize;
    let mut spans = Vec::new();
    let mut start = 0;
    while start < filled {
        let shade = color(percent_at(start, width));
        let mut end = start + 1;
        while end < filled && color(percent_at(end, width)) == shade {
            end += 1;
        }
        spans.push(Span::styled(
            "█".repeat(end - start),
            Style::default().fg(shade),
        ));
        start = end;
    }
    if filled < width {
        spans.push(Span::styled(
            "█".repeat(width - filled),
            Style::default().fg(IDLE),
        ));
    }
    Line::from(spans)
}

fn percent_at(index: usize, width: usize) -> f32 {
    (index as f32 + 0.5) / width as f32 * 100.0
}

/// Format a byte rate, picking B/s, KB/s or MB/s by magnitude.
/// Wash the area out so an overlay stands out: foreground colors are blended
/// towards the background, which reads as "blurred" behind glass.
pub fn scrim(frame: &mut Frame, area: Rect, background: Color, keep_percent: u8) {
    let buf = frame.buffer_mut();
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let cell = &mut buf[(x, y)];
            let current = cell.style();
            let blended = current
                .fg
                .and_then(|fg| blend(fg, background, keep_percent));
            let mut style = Style::default()
                .fg(blended.unwrap_or(Color::Reset))
                .bg(background);
            if blended.is_none() {
                style = style.add_modifier(Modifier::DIM);
            }
            cell.set_style(style);
        }
    }
}

/// Mix `fg` towards `bg`, keeping `keep_percent` of the original color.
fn blend(fg: Color, bg: Color, keep_percent: u8) -> Option<Color> {
    let (Color::Rgb(fr, fg_, fb), Color::Rgb(br, bg_, bb)) = (fg, bg) else {
        return None;
    };
    let keep = u16::from(keep_percent.min(100));
    let mix = |f: u8, b: u8| ((u16::from(b) * (100 - keep) + u16::from(f) * keep) / 100) as u8;
    Some(Color::Rgb(mix(fr, br), mix(fg_, bg_), mix(fb, bb)))
}

pub fn format_rate(bytes_per_sec: f64) -> String {
    if bytes_per_sec < 1_024.0 {
        format!("{bytes_per_sec:.0} B/s")
    } else if bytes_per_sec < 1_048_576.0 {
        format!("{:.1} KB/s", bytes_per_sec / 1_024.0)
    } else {
        format!("{:.2} MB/s", bytes_per_sec / 1_048_576.0)
    }
}

/// Format a byte count as MiB or GiB, whichever keeps it readable.
pub fn format_bytes(bytes: u64) -> String {
    const MIB: f64 = 1_048_576.0;
    const GIB: f64 = 1_073_741_824.0;
    let value = bytes as f64;
    if value >= GIB {
        format!("{:.1} GiB", value / GIB)
    } else {
        format!("{:.1} MiB", value / MIB)
    }
}

/// Spread the tab titles evenly across `width`, with the dividers centred in
/// the gaps. The last title ends at the right edge.
fn tab_line(titles: &[&str], selected: usize, accent: Color, width: usize) -> Vec<Span<'static>> {
    let dividers = titles.len().saturating_sub(1);
    let text_width: usize = titles.iter().map(|title| title.chars().count()).sum();
    // One space of margin at each end; the rest is split into even gaps so the
    // spaces around every divider are symmetric.
    let spare = width.saturating_sub(text_width + dividers + 2);
    let pairs = spare / 2;
    let (base, extra) = pairs
        .checked_div(dividers)
        .map_or((0, 0), |base| (base, pairs % dividers));
    let mut spans = vec![Span::raw(" ")];
    for (index, title) in titles.iter().enumerate() {
        if index > 0 {
            let half = base + usize::from(index - 1 < extra);
            spans.push(Span::raw(" ".repeat(half)));
            spans.push(dim_text("│"));
            spans.push(Span::raw(" ".repeat(half)));
        }
        let style = if index == selected {
            Style::default()
                .fg(accent)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            Style::default().fg(DIM)
        };
        spans.push(Span::styled((*title).to_string(), style));
    }
    spans.push(Span::raw(" ".repeat(1 + spare % 2)));
    spans
}

/// A bold, accent-colored line for use as a block title.
pub fn styled_title(text: impl Into<String>, accent: Color) -> Line<'static> {
    Line::from(Span::styled(
        text.into(),
        Style::default().fg(accent).add_modifier(Modifier::BOLD),
    ))
}

/// A dim span for units and secondary text.
pub fn dim_text(text: impl Into<String>) -> Span<'static> {
    Span::styled(text.into(), Style::default().fg(DIM))
}

/// A rounded panel with a styled, accent-colored title.
pub fn panel(title: impl Into<String>, accent: Color) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(DIM))
        .title(styled_title(title, accent))
}

/// Render a horizontally and vertically centered empty-state message.
pub fn render_empty(frame: &mut Frame, area: Rect, title: &str, message: &str, accent: Color) {
    let block = panel(title, accent);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [_, message_area, _] = Layout::vertical([
        Constraint::Percentage(50),
        Constraint::Length(1),
        Constraint::Percentage(50),
    ])
    .areas(inner);

    let paragraph = Paragraph::new(message)
        .alignment(Alignment::Center)
        .style(Style::default().fg(DIM));
    frame.render_widget(paragraph, message_area);
}

/// Render the tab bar and the active tab's content.
pub fn draw(frame: &mut Frame, app: &App) {
    let full = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(app.palette.background)),
        full,
    );
    // Background keeps the full terminal; the dim borders sit two cells in on
    // the sides and at the bottom, one row in at the top.
    let area = Rect {
        x: full.x.saturating_add(2),
        y: full.y.saturating_add(1),
        width: full.width.saturating_sub(4),
        height: full.height.saturating_sub(2),
    };

    let [tab_area, content_area] =
        Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).areas(area);

    let titles = [
        "system",
        "cpu",
        "memory",
        "disk",
        "network",
        "gpu",
        "battery",
        "processes",
        "fans",
        "theme",
    ];
    let selected = match app.tab {
        Tab::System => 0,
        Tab::Cpu => 1,
        Tab::Memory => 2,
        Tab::Disk => 3,
        Tab::Network => 4,
        Tab::Gpu => 5,
        Tab::Battery => 6,
        Tab::Processes => 7,
        Tab::Fans => 8,
        Tab::Theme => 9,
    };

    let accent = accent(app);
    let tab_block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(DIM));
    let tab_inner = tab_block.inner(tab_area);
    frame.render_widget(tab_block, tab_area);

    let spans = tab_line(&titles, selected, accent, tab_inner.width as usize);
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(app.palette.background)),
        tab_inner,
    );

    match app.tab {
        Tab::System => system::draw(frame, content_area, app),
        Tab::Cpu => cpu::draw(frame, content_area, app),
        Tab::Memory => memory::draw(frame, content_area, app),
        Tab::Disk => disk::draw(frame, content_area, app),
        Tab::Network => network::draw(frame, content_area, app),
        Tab::Gpu => gpu::draw(frame, content_area, app),
        Tab::Battery => battery::draw(frame, content_area, app),
        Tab::Processes => processes::draw(frame, content_area, app),
        Tab::Fans => fans::draw(frame, content_area, app),
        Tab::Theme => theme::draw(frame, content_area, app),
    }
}

#[cfg(test)]
mod tests {
    use super::{IDLE, scrim, segmented_bar};
    use crate::app::App;
    use crate::config::Config;
    use crate::theme::Palette;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::{Color, Style};
    use ratatui::widgets::Paragraph;

    #[test]
    fn scrim_blends_foreground_towards_the_background() {
        let mut terminal = Terminal::new(TestBackend::new(10, 3)).unwrap();
        terminal
            .draw(|frame| {
                let text =
                    Paragraph::new("x").style(Style::default().fg(Color::Rgb(255, 255, 255)));
                frame.render_widget(text, frame.area());
                scrim(frame, frame.area(), Color::Rgb(0, 0, 0), 30);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(0, 0)].fg, Color::Rgb(76, 76, 76));
        assert_eq!(buffer[(0, 0)].bg, Color::Rgb(0, 0, 0));
    }

    #[test]
    fn background_covers_the_whole_terminal() {
        let app = App::new(&Config::default(), 80).unwrap();
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|f| super::draw(f, &app)).unwrap();
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(0, 0)].bg, app.palette.background);
        assert_eq!(buffer[(79, 23)].bg, app.palette.background);
        assert_eq!(buffer[(2, 10)].bg, app.palette.background);
        // The tab bar must be painted too, not left transparent.
        assert_eq!(buffer[(3, 2)].bg, app.palette.background);
        // Borders are inset by two cells (one row at the top) over a full background.
        assert_eq!(buffer[(0, 0)].symbol(), " ");
        assert_eq!(buffer[(1, 0)].symbol(), " ");
        assert_eq!(buffer[(2, 0)].symbol(), " ");
        assert_eq!(buffer[(2, 1)].symbol(), "╭");
        assert_eq!(buffer[(2, 23)].symbol(), " ");
        assert_eq!(buffer[(2, 23)].bg, app.palette.background);
        assert_eq!(buffer[(77, 1)].symbol(), "╮");

        let mut transparent = App::new(&Config::default(), 80).unwrap();
        transparent.palette.background = Color::Reset;
        terminal.draw(|f| super::draw(f, &transparent)).unwrap();
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(2, 10)].bg, Color::Reset);
    }

    #[test]
    fn tab_bar_is_spread() {
        for width in [80u16, 100] {
            let app = App::new(&Config::default(), width).unwrap();
            let mut terminal = Terminal::new(TestBackend::new(width, 24)).unwrap();
            terminal.draw(|f| super::draw(f, &app)).unwrap();
            let buffer = terminal.backend().buffer();
            // Inner row: two inset columns plus the block border on each side.
            let row: Vec<String> = (3..width - 3)
                .map(|x| buffer[(x, 2)].symbol().to_string())
                .collect();
            let first = row.iter().position(|symbol| symbol != " ").unwrap();
            let last = row.iter().rposition(|symbol| symbol != " ").unwrap();
            assert!(first <= 1, "width {width}");
            assert!(row.len() - 1 - last <= 2, "width {width}");

            // Every divider must have the same number of spaces on both sides.
            for (index, symbol) in row.iter().enumerate() {
                if symbol != "│" {
                    continue;
                }
                let left = row[..index].iter().rev().take_while(|s| *s == " ").count();
                let right = row[index + 1..].iter().take_while(|s| *s == " ").count();
                assert_eq!(left, right, "width {width} divider {index}");
            }
        }
    }

    #[test]
    fn tab_bar_survives_a_narrow_terminal() {
        let app = App::new(&Config::default(), 40).unwrap();
        let mut terminal = Terminal::new(TestBackend::new(40, 20)).unwrap();
        terminal.draw(|f| super::draw(f, &app)).unwrap();
        let buffer = terminal.backend().buffer();
        let row: String = (0..40)
            .map(|x| buffer[(x, 2)].symbol().to_string())
            .collect();
        assert!(row.contains("cpu"));
        assert!(row.contains("network"));
    }

    #[test]
    fn segmented_bar_colors_by_level() {
        let palette = Palette::default();
        let bar = segmented_bar(70.0, 100, |value| palette.threshold(value));
        let colors: Vec<Option<Color>> = bar.spans.iter().map(|span| span.style.fg).collect();
        assert_eq!(
            colors,
            vec![Some(palette.good), Some(palette.warn), Some(IDLE)]
        );

        let total: usize = bar
            .spans
            .iter()
            .map(|span| span.content.chars().count())
            .sum();
        assert_eq!(total, 100);

        assert!(
            segmented_bar(90.0, 100, |value| palette.threshold(value))
                .spans
                .iter()
                .any(|span| span.style.fg == Some(palette.crit))
        );
        assert_eq!(
            segmented_bar(50.0, 0, |value| palette.threshold(value))
                .spans
                .len(),
            0
        );

        let mono: Vec<Option<Color>> = segmented_bar(70.0, 100, |_| palette.good)
            .spans
            .iter()
            .map(|span| span.style.fg)
            .collect();
        assert_eq!(mono, vec![Some(palette.good), Some(IDLE)]);
    }
}
