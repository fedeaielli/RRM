use crate::app::{App, Tab};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph, Tabs};

pub mod battery;
pub mod braille;
pub mod cpu;
pub mod disk;
pub mod gpu;
pub mod memory;
pub mod network;
pub mod processes;
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
    let [tab_area, content_area] =
        Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).areas(frame.area());

    let titles = [
        "cpu",
        "memory",
        "disk",
        "network",
        "gpu",
        "battery",
        "processes",
        "theme",
    ];
    let selected = match app.tab {
        Tab::Cpu => 0,
        Tab::Memory => 1,
        Tab::Disk => 2,
        Tab::Network => 3,
        Tab::Gpu => 4,
        Tab::Battery => 5,
        Tab::Processes => 6,
        Tab::Theme => 7,
    };

    let accent = accent(app);
    let tab_block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(DIM));
    let tabs = Tabs::new(titles)
        .block(tab_block)
        .select(selected)
        .style(Style::default().fg(DIM))
        .highlight_style(
            Style::default()
                .fg(accent)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )
        .divider(Span::styled(" │ ", Style::default().fg(DIM)));

    frame.render_widget(tabs, tab_area);

    match app.tab {
        Tab::Cpu => cpu::draw(frame, content_area, app),
        Tab::Memory => memory::draw(frame, content_area, app),
        Tab::Disk => disk::draw(frame, content_area, app),
        Tab::Network => network::draw(frame, content_area, app),
        Tab::Gpu => gpu::draw(frame, content_area, app),
        Tab::Battery => battery::draw(frame, content_area, app),
        Tab::Processes => processes::draw(frame, content_area, app),
        Tab::Theme => theme::draw(frame, content_area, app),
    }
}

#[cfg(test)]
mod tests {
    use super::{IDLE, segmented_bar};
    use crate::theme::Palette;
    use ratatui::style::Color;

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
