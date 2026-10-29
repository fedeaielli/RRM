use super::{DIM, accent, dim_text, format_rate, render_empty};
use crate::app::App;
use crate::collectors::processes::SortBy;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Cell, Paragraph, Row, Table, TableState};

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
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));
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
