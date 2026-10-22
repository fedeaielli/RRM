use super::{CYAN, MGNTA, accent, dim_text, format_rate, panel, render_empty};
use crate::app::App;
use crate::history::RingBuffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Sparkline};
use std::collections::HashMap;

pub fn draw(frame: &mut ratatui::Frame, area: Rect, app: &App) {
    let accent = accent(app);
    if app.network.latest.is_empty() {
        render_empty(frame, area, "network", "no network interfaces", accent);
        return;
    }

    let count = app.network.latest.len();
    let constraints: Vec<Constraint> = (0..count)
        .map(|_| Constraint::Ratio(1, count as u32))
        .collect();
    let interface_areas = Layout::vertical(constraints).split(area);

    for (sample, interface_area) in app.network.latest.iter().zip(interface_areas.iter()) {
        let block = panel(sample.interface.clone(), accent);
        let inner = block.inner(*interface_area);
        frame.render_widget(block, *interface_area);
        if inner.height == 0 {
            continue;
        }

        let [rate_area, charts_area] =
            Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).areas(inner);
        let [_, rate_line_area, _] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .areas(rate_area);

        let rate_line = Line::from(vec![
            ratatui::text::Span::styled(
                format!("rx {}", format_rate(sample.rx_bytes_per_sec)),
                Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
            ),
            dim_text("  ↓↑  "),
            ratatui::text::Span::styled(
                format!("tx {}", format_rate(sample.tx_bytes_per_sec)),
                Style::default().fg(MGNTA).add_modifier(Modifier::BOLD),
            ),
        ]);
        frame.render_widget(Paragraph::new(rate_line), rate_line_area);

        let [rx_chart_area, tx_chart_area] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                .areas(charts_area);

        let rx_history = history_values(&app.net_rx_history, &sample.interface);
        let rx_sparkline = Sparkline::default()
            .data(&rx_history)
            .style(Style::default().fg(CYAN));
        frame.render_widget(rx_sparkline, rx_chart_area);

        let tx_history = history_values(&app.net_tx_history, &sample.interface);
        let tx_sparkline = Sparkline::default()
            .data(&tx_history)
            .style(Style::default().fg(MGNTA));
        frame.render_widget(tx_sparkline, tx_chart_area);
    }
}

fn history_values(histories: &HashMap<String, RingBuffer>, interface: &str) -> Vec<u64> {
    histories
        .get(interface)
        .map(|buffer| {
            buffer
                .as_slice()
                .iter()
                .map(|value| (f64::from(*value) / 1024.0) as u64)
                .collect()
        })
        .unwrap_or_default()
}
