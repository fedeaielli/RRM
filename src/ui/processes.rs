use super::{DIM, accent, braille, dim_text, format_rate, render_empty, scrim};
use crate::app::{App, ProcessDetail};
use crate::collectors::processes::SortBy;
use crate::history::RingBuffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Cell, Clear, Paragraph, Row, Table, TableState};

/// Fixed column widths; narrow terminals scroll horizontally instead of squeezing.
const COLUMNS: [(&str, u16); 6] = [
    ("PID", 7),
    ("NAME", 24),
    ("CPU%", 7),
    ("MEM MiB", 9),
    ("READ/s", 10),
    ("WRITE/s", 10),
];

/// Last selectable column offset.
pub const MAX_COLUMN: usize = COLUMNS.len() - 1;

fn sort_column(sort: SortBy) -> usize {
    match sort {
        SortBy::Cpu => 2,
        SortBy::Mem => 3,
        SortBy::Read => 4,
        SortBy::Write => 5,
    }
}

pub fn draw(frame: &mut ratatui::Frame, area: Rect, app: &App) {
    let accent = accent(app);
    if app.processes.top.is_empty() {
        render_empty(frame, area, "processes", "no process data", accent);
        return;
    }

    let [table_area, footer_area] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(area);

    let first = app.process_column.min(MAX_COLUMN);
    let (last, widths) = visible_columns(first, table_area.width);
    let active = sort_column(app.processes.sort_by());

    let header = Row::new(
        COLUMNS[first..last]
            .iter()
            .enumerate()
            .map(|(offset, (title, _))| {
                let index = first + offset;
                let label = if index == active {
                    format!("{title} ▼")
                } else {
                    (*title).to_string()
                };
                Cell::from(label)
            })
            .collect::<Vec<_>>(),
    )
    .style(Style::default().fg(accent).add_modifier(Modifier::BOLD));

    let rows = app
        .processes
        .top
        .iter()
        .enumerate()
        .map(|(index, process)| {
            let base = if index % 2 == 0 {
                Style::default().fg(Color::Reset)
            } else {
                Style::default().fg(DIM)
            };

            let mem_mib = process.mem_kb as f64 / 1024.0;
            let mem_color = if mem_mib < 50.0 {
                DIM
            } else if mem_mib < 500.0 {
                Color::Reset
            } else {
                app.palette.warn
            };

            let cells = [
                Cell::from(process.pid.to_string()).style(base),
                Cell::from(process.name.clone()).style(base),
                Cell::from(format!("{:.1}%", process.cpu_percent))
                    .style(base.fg(app.palette.threshold(process.cpu_percent))),
                Cell::from(format!("{mem_mib:.1}")).style(base.fg(mem_color)),
                Cell::from(format_rate(process.read_bytes_per_sec)).style(base.fg(app.palette.rx)),
                Cell::from(format_rate(process.write_bytes_per_sec)).style(base.fg(app.palette.tx)),
            ];

            Row::new(cells[first..last].to_vec())
        });

    let table = Table::new(rows, widths)
        .header(header)
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("› ");
    let mut state = TableState::default().with_selected(app.process_selected);
    frame.render_stateful_widget(table, table_area, &mut state);

    let footer = if app.process_paused {
        format!(
            "showing all {} · sort: {} · paused — space to resume · ↑↓ move · Esc exit",
            app.processes.total,
            app.processes.sort_label()
        )
    } else if app.process_selected.is_some() {
        format!(
            "showing all {} · sort: {} · ↑↓ move · ←→ cols · space pause · Esc exit",
            app.processes.total,
            app.processes.sort_label()
        )
    } else {
        format!(
            "showing all {} · sort: {} · PgDn scroll · space pause · [s] sort",
            app.processes.total,
            app.processes.sort_label()
        )
    };
    frame.render_widget(Paragraph::new(dim_text(footer)), footer_area);

    if let Some(detail) = &app.process_detail {
        draw_detail(frame, area, app, detail);
    }
}

/// Centered overlay, lighter than the background so it stands out.
fn draw_detail(frame: &mut ratatui::Frame, area: Rect, app: &App, detail: &ProcessDetail) {
    let width = (area.width * 4 / 5).max(40).min(area.width);
    let height = (area.height * 4 / 5).max(12).min(area.height);
    let rect = Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    };

    let bg = shade(app.palette.background, 24);
    // Pure black matches the terminal default, which some terminals render
    // translucent, so a transparent palette darkens towards near black instead.
    let base = match app.palette.background {
        Color::Rgb(..) => app.palette.background,
        _ => crate::theme::NEAR_BLACK,
    };
    scrim(frame, area, base, 20);

    // One-cell shadow ring so the overlay lifts off the blurred content.
    let shadow = Rect {
        x: rect.x.saturating_sub(1),
        y: rect.y.saturating_sub(1),
        width: rect.width.saturating_add(2).min(area.width),
        height: rect.height.saturating_add(2).min(area.height),
    };
    frame.render_widget(
        Block::default().style(Style::default().bg(darken(base, 6))),
        shadow,
    );

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.palette.accent))
        .style(Style::default().bg(bg))
        .title(format!(" {} ({}) ", detail.name, detail.pid));
    let inner = block.inner(rect);
    frame.render_widget(Clear, rect);
    frame.render_widget(block, rect);
    let [info_area, charts_area] =
        Layout::vertical([Constraint::Length(2), Constraint::Min(0)]).areas(inner);

    let sample = &detail.sample;
    let gpu_mem_mib = detail.extras.gpu_mem_kb as f64 / 1024.0;
    let info = Line::from(vec![
        Span::styled(
            format!("state {} ", detail.extras.state),
            Style::default().fg(app.palette.key),
        ),
        Span::styled(
            format!("· threads {} ", detail.extras.threads),
            Style::default().fg(app.palette.value),
        ),
        Span::styled(
            format!("· gpu mem {gpu_mem_mib:.1} MiB "),
            Style::default().fg(app.palette.value),
        ),
        Span::styled(
            format!(
                "· {}",
                truncate(&detail.extras.cmdline, info_area.width as usize)
            ),
            Style::default().fg(DIM),
        ),
    ]);
    frame.render_widget(Paragraph::new(info), info_area);

    let rows = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Fill(1),
        Constraint::Fill(1),
    ])
    .split(charts_area);
    let [top_left, top_right] =
        Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).areas(rows[0]);
    let [mid_left, mid_right] =
        Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).areas(rows[1]);
    let [low_left, low_right] =
        Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).areas(rows[2]);

    detail_chart(
        frame,
        top_left,
        format!("cpu  {:.1}%", sample.cpu_percent),
        &detail.cpu_history,
        10.0,
        1000,
        app.palette.threshold(sample.cpu_percent),
        bg,
    );
    detail_chart(
        frame,
        top_right,
        format!("ram  {:.1} MiB", sample.mem_kb as f64 / 1024.0),
        &detail.mem_history,
        1.0,
        1,
        app.palette.accent,
        bg,
    );
    detail_chart(
        frame,
        mid_left,
        format!("gpu  {:.1}%", detail.gpu_percent),
        &detail.gpu_history,
        10.0,
        1000,
        app.palette.threshold(detail.gpu_percent),
        bg,
    );
    detail_chart(
        frame,
        mid_right,
        format!("read  {}", format_rate(sample.read_bytes_per_sec)),
        &detail.read_history,
        1.0,
        1,
        app.palette.rx,
        bg,
    );
    detail_chart(
        frame,
        low_left,
        format!("write  {}", format_rate(sample.write_bytes_per_sec)),
        &detail.write_history,
        1.0,
        1,
        app.palette.tx,
        bg,
    );

    let notes = Paragraph::new(vec![
        Line::from(Span::styled("pid", Style::default().fg(app.palette.key))),
        Line::from(Span::styled(
            detail.pid.to_string(),
            Style::default().fg(app.palette.value),
        )),
        Line::from(Span::styled(
            "network and power are not exposed per process",
            Style::default().fg(DIM),
        )),
    ])
    .block(
        Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(DIM))
            .style(Style::default().bg(bg))
            .title(" notes "),
    )
    .style(Style::default().bg(bg));
    frame.render_widget(notes, low_right);
}

#[allow(clippy::too_many_arguments)]
fn detail_chart(
    frame: &mut ratatui::Frame,
    area: Rect,
    title: String,
    history: &RingBuffer,
    scale: f32,
    min_max: u64,
    color: Color,
    bg: Color,
) {
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(DIM))
        .style(Style::default().bg(bg))
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(color),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    braille::render(
        frame,
        inner,
        history,
        scale,
        min_max,
        Style::default().fg(color).bg(bg),
    );
}

/// Lighten a background color so the overlay is distinguishable.
fn shade(color: Color, step: u8) -> Color {
    match color {
        Color::Rgb(red, green, blue) => Color::Rgb(
            red.saturating_add(step),
            green.saturating_add(step),
            blue.saturating_add(step),
        ),
        _ => Color::Rgb(24, 24, 24),
    }
}

/// Darken a background color for the overlay shadow, never down to pure black.
fn darken(color: Color, step: u8) -> Color {
    let channel = |value: u8| value.saturating_sub(step).max(4);
    match color {
        Color::Rgb(red, green, blue) => Color::Rgb(channel(red), channel(green), channel(blue)),
        _ => Color::Rgb(4, 4, 4),
    }
}

fn truncate(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        text.to_string()
    } else if width <= 3 {
        text.chars().take(width).collect()
    } else {
        format!("{}...", text.chars().take(width - 3).collect::<String>())
    }
}

/// Columns from `first` that fit in `width`, at least one.
fn visible_columns(first: usize, width: u16) -> (usize, Vec<Constraint>) {
    let mut used = 0u16;
    let mut last = first + 1;
    for (index, (_, column_width)) in COLUMNS.iter().enumerate().skip(first) {
        if index > first && used + column_width > width {
            break;
        }
        used = used.saturating_add(*column_width);
        last = index + 1;
    }
    let widths = COLUMNS[first..last]
        .iter()
        .map(|(_, column_width)| Constraint::Length(*column_width))
        .collect();
    (last, widths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn focus_selects_the_first_row() {
        let mut app = App::new(&Config::default(), 80).unwrap();
        app.tab = crate::app::Tab::Processes;
        app.focus_processes();
        assert_eq!(app.process_selected, Some(0));
    }

    #[test]
    fn selected_row_is_the_first_data_row() {
        let config = Config::default();
        let mut app = App::new(&config, 80).unwrap();
        app.tab = crate::app::Tab::Processes;
        app.focus_processes();

        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        terminal
            .draw(|frame| draw(frame, frame.area(), &app))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let reversed: Vec<u16> = (0..20)
            .filter(|&y| (0..80).any(|x| buffer[(x, y)].modifier.contains(Modifier::REVERSED)))
            .collect();
        assert_eq!(reversed, vec![1]);
    }

    #[test]
    fn transparent_background_scrims_towards_near_black() {
        let config = Config::default();
        let mut app = App::new(&config, 80).unwrap();
        app.tab = crate::app::Tab::Processes;
        app.palette.background = Color::Reset;
        app.process_selected = Some(0);
        app.open_process_detail();
        let Some(detail) = app.process_detail.as_ref() else {
            return;
        };

        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| draw_detail(frame, frame.area(), &app, detail))
            .unwrap();
        let buffer = terminal.backend().buffer();
        for y in 0..24 {
            for x in 0..80 {
                assert_ne!(
                    buffer[(x, y)].bg,
                    Color::Rgb(0, 0, 0),
                    "cell ({x},{y}) is pure black and may render translucent"
                );
            }
        }
    }

    #[test]
    fn paused_footer_is_shown() {
        let config = Config::default();
        let mut app = App::new(&config, 80).unwrap();
        app.tab = crate::app::Tab::Processes;
        app.process_paused = true;

        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| draw(frame, frame.area(), &app))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let text: String = buffer
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(text.contains("paused"));
    }

    #[test]
    fn visible_columns_fit_the_width() {
        let (last, widths) = visible_columns(0, 40);
        assert_eq!(last, 3);
        assert_eq!(widths.len(), 3);

        let (last, widths) = visible_columns(0, 200);
        assert_eq!(last, COLUMNS.len());
        assert_eq!(widths.len(), COLUMNS.len());

        let (last, _) = visible_columns(MAX_COLUMN, 5);
        assert_eq!(last, COLUMNS.len());
    }
}
