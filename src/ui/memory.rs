use super::{DIM, accent, dim_text, panel, render_empty, threshold_color};
use crate::app::App;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{LineGauge, Paragraph, Sparkline};

pub fn draw(frame: &mut ratatui::Frame, area: Rect, app: &App) {
    let accent = accent(app);
    let memory = &app.memory.latest;

    if memory.total_kb == 0 && memory.swap_total_kb == 0 {
        render_empty(frame, area, "memory", "no memory data", accent);
        return;
    }

    let block = panel("memory", accent);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }

    let [ram_area, swap_area, history_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(0),
    ])
    .areas(inner);

    let ram_percent = percent(memory.used_kb, memory.total_kb);
    draw_usage_row(
        frame,
        ram_area,
        &format!(
            "ram  {:.1} / {:.1} GiB",
            memory.used_kb as f64 / 1_048_576.0,
            memory.total_kb as f64 / 1_048_576.0
        ),
        ram_percent,
        false,
    );

    let swap_percent = percent(memory.swap_used_kb, memory.swap_total_kb);
    draw_usage_row(
        frame,
        swap_area,
        &format!(
            "swap {:.1} / {:.1} GiB",
            memory.swap_used_kb as f64 / 1_048_576.0,
            memory.swap_total_kb as f64 / 1_048_576.0
        ),
        swap_percent,
        memory.swap_total_kb == 0,
    );

    let history: Vec<u64> = app
        .mem_history
        .as_slice()
        .iter()
        .map(|value| (*value * 10.0) as u64)
        .collect();
    let sparkline = Sparkline::default()
        .block(panel("ram history", accent))
        .data(&history)
        .style(Style::default().fg(threshold_color(ram_percent)));
    frame.render_widget(sparkline, history_area);
}

fn draw_usage_row(
    frame: &mut ratatui::Frame,
    area: Rect,
    label: &str,
    percent: f32,
    disabled: bool,
) {
    if disabled {
        let [_, line_area, _] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .areas(area);
        frame.render_widget(Paragraph::new(dim_text("swap  disabled")), line_area);
        return;
    }

    let color = threshold_color(percent);
    let [label_area, gauge_area] =
        Layout::horizontal([Constraint::Length(20), Constraint::Min(0)]).areas(area);
    let [_, label_line, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .areas(label_area);
    let [_, gauge_line, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .areas(gauge_area);

    let label = Paragraph::new(Span::styled(
        label.to_string(),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(label, label_line);

    let gauge = LineGauge::default()
        .filled_style(Style::default().fg(color))
        .unfilled_style(Style::default().fg(DIM))
        .line_set(symbols::line::THICK)
        .label(Line::from(Span::styled(
            format!("{percent:.1}% "),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )))
        .ratio((f64::from(percent) / 100.0).clamp(0.0, 1.0));
    frame.render_widget(gauge, gauge_line);
}

fn percent(used_kb: u64, total_kb: u64) -> f32 {
    if total_kb == 0 {
        0.0
    } else {
        used_kb as f32 / total_kb as f32 * 100.0
    }
}
