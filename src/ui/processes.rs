use super::{DIM, accent, dim_text, panel, render_empty, threshold_color};
use crate::app::App;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Cell, Row, Table};

pub fn draw(frame: &mut ratatui::Frame, area: Rect, app: &App) {
    let accent = accent(app);
    if app.processes.top.is_empty() {
        render_empty(frame, area, "processes", "no process data", accent);
        return;
    }

    let [table_area, footer_area] =
        ratatui::layout::Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(area);

    let header = Row::new(vec!["PID", "NAME", "CPU%", "MEM (MiB)"])
        .style(Style::default().fg(accent).add_modifier(Modifier::BOLD));

    let rows = app
        .processes
        .top
        .iter()
        .enumerate()
        .map(|(index, process)| {
            let secondary = if index % 2 == 0 {
                Style::default().fg(Color::Reset)
            } else {
                Style::default().fg(DIM)
            };

            let cpu_color = threshold_color(process.cpu_percent);
            let mem_mib = process.mem_kb as f64 / 1024.0;
            let mem_style = if mem_mib < 10.0 {
                Style::default().fg(DIM)
            } else {
                Style::default().fg(Color::Reset)
            };

            Row::new(vec![
                Cell::from(process.pid.to_string()).style(secondary),
                Cell::from(process.name.clone()).style(secondary),
                Cell::from(format!("{:.1}%", process.cpu_percent))
                    .style(Style::default().fg(cpu_color)),
                Cell::from(format!("{mem_mib:.1}")).style(mem_style),
            ])
        });

    let widths = [
        Constraint::Percentage(10),
        Constraint::Percentage(42),
        Constraint::Percentage(23),
        Constraint::Percentage(25),
    ];
    let table = Table::new(rows, widths)
        .header(header)
        .block(panel("processes", accent));
    frame.render_widget(table, table_area);

    let footer = ratatui::widgets::Paragraph::new(dim_text(format!(
        " showing {} / {} processes  •  sort: {}",
        app.processes.top.len(),
        app.processes.total,
        app.processes.sort_label()
    )));
    frame.render_widget(footer, footer_area);
}
