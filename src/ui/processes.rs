use super::{DIM, WARN, accent, dim_text, render_empty, threshold_color};
use crate::app::App;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Cell, Paragraph, Row, Table};

pub fn draw(frame: &mut ratatui::Frame, area: Rect, app: &App) {
    let accent = accent(app);
    if app.processes.top.is_empty() {
        render_empty(frame, area, "processes", "no process data", accent);
        return;
    }

    let [table_area, footer_area] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(area);

    let header = Row::new(vec![
        Cell::from("PID"),
        Cell::from("NAME"),
        Cell::from("CPU%"),
        Cell::from("MEM MiB"),
        Cell::from(format!("SORT {} ▼", app.processes.sort_label())),
    ])
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

            let cpu_color = threshold_color(process.cpu_percent);
            let mem_mib = process.mem_kb as f64 / 1024.0;
            let mem_color = if mem_mib < 50.0 {
                DIM
            } else if mem_mib < 500.0 {
                Color::Reset
            } else {
                WARN
            };

            Row::new(vec![
                Cell::from(process.pid.to_string()).style(base),
                Cell::from(process.name.clone()).style(base),
                Cell::from(format!("{:.1}%", process.cpu_percent)).style(base.fg(cpu_color)),
                Cell::from(format!("{mem_mib:.1}")).style(base.fg(mem_color)),
                Cell::from("").style(base),
            ])
        });

    let widths = [
        Constraint::Percentage(8),
        Constraint::Percentage(40),
        Constraint::Percentage(20),
        Constraint::Percentage(16),
        Constraint::Percentage(16),
    ];
    let table = Table::new(rows, widths).header(header);
    frame.render_widget(table, table_area);

    let footer = Paragraph::new(dim_text(format!(
        "showing {}/{} · sort: {} · [s] toggle sort",
        app.processes.top.len(),
        app.processes.total,
        app.processes.sort_label()
    )));
    frame.render_widget(footer, footer_area);
}
