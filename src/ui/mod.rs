use crate::app::{App, Tab};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph, Tabs};

pub mod battery;
pub mod cpu;
pub mod disk;
pub mod gpu;
pub mod memory;
pub mod network;
pub mod processes;

/// Healthy / low-load values.
pub const GOOD: Color = Color::Green;
/// Elevated values.
pub const WARN: Color = Color::Yellow;
/// Critical values.
pub const CRIT: Color = Color::Red;
/// Secondary labels, units, separators and idle rows.
pub const DIM: Color = Color::DarkGray;
/// Receive traffic.
pub const CYAN: Color = Color::Cyan;
/// Transmit traffic.
pub const MGNTA: Color = Color::Magenta;

/// Resolve the configured accent color.
pub fn accent(app: &App) -> Color {
    Color::Rgb(app.accent.0, app.accent.1, app.accent.2)
}

/// Compact load threshold: good below 60%, warning to 85%, critical above.
pub fn threshold_color(pct: f32) -> Color {
    if pct < 60.0 {
        GOOD
    } else if pct <= 85.0 {
        WARN
    } else {
        CRIT
    }
}

/// Disk usage threshold: good below 70%, warning to 85%, critical above.
pub fn disk_usage_color(pct: f32) -> Color {
    if pct < 70.0 {
        GOOD
    } else if pct <= 85.0 {
        WARN
    } else {
        CRIT
    }
}

/// Format a byte rate using KB/s below 1 MiB/s and MB/s above.
pub fn format_rate(bytes_per_sec: f64) -> String {
    if bytes_per_sec < 1_048_576.0 {
        format!("{:.1} KB/s", bytes_per_sec / 1024.0)
    } else {
        format!("{:.1} MB/s", bytes_per_sec / 1_048_576.0)
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
    ];
    let selected = match app.tab {
        Tab::Cpu => 0,
        Tab::Memory => 1,
        Tab::Disk => 2,
        Tab::Network => 3,
        Tab::Gpu => 4,
        Tab::Battery => 5,
        Tab::Processes => 6,
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
    }
}
