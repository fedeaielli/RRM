use super::{DIM, accent, dim_text, panel, render_empty, threshold_color};
use crate::app::App;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{LineGauge, Paragraph, Sparkline};

pub fn draw(frame: &mut ratatui::Frame, area: Rect, app: &App) {
    let accent = accent(app);
    let cores = &app.cpu.latest.per_core_percent;
    if cores.is_empty() {
        render_empty(frame, area, "cpu", "no cpu data", accent);
        return;
    }

    let [top_area, history_area] =
        Layout::vertical([Constraint::Percentage(70), Constraint::Percentage(30)]).areas(area);
    let [cores_area, details_area] =
        Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)])
            .areas(top_area);

    draw_cores(frame, cores_area, app, accent);
    draw_details(frame, details_area, app, accent);
    draw_history(frame, history_area, app, accent);
}

fn draw_cores(frame: &mut ratatui::Frame, area: Rect, app: &App, accent: ratatui::style::Color) {
    let block = panel("cpu cores", accent);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    draw_core_rows(frame, inner, &app.cpu.latest.per_core_percent);
}

fn draw_core_rows(frame: &mut ratatui::Frame, area: Rect, cores: &[f32]) {
    let available = area.height as usize;
    if available == 0 {
        return;
    }

    let needs_footer = cores.len() > available;
    let shown = if needs_footer {
        available.saturating_sub(1)
    } else {
        cores.len().min(available)
    };

    for (index, percent) in cores.iter().take(shown).enumerate() {
        let row = Rect {
            x: area.x,
            y: area.y.saturating_add(index as u16),
            width: area.width,
            height: 1,
        };
        draw_core_row(frame, row, index, *percent);
    }

    if needs_footer {
        let footer = Rect {
            x: area.x,
            y: area.y.saturating_add(shown as u16),
            width: area.width,
            height: 1,
        };
        let paragraph = Paragraph::new(dim_text(format!("↓ {} more", cores.len() - shown)));
        frame.render_widget(paragraph, footer);
    }
}

fn draw_core_row(frame: &mut ratatui::Frame, area: Rect, index: usize, percent: f32) {
    let color = if percent <= 0.0 {
        DIM
    } else {
        threshold_color(percent)
    };
    let ratio = (f64::from(percent) / 100.0).clamp(0.005, 1.0);
    let [bar_area, percent_area] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(5)]).areas(area);

    let gauge = LineGauge::default()
        .filled_style(Style::default().fg(color))
        .unfilled_style(Style::default().fg(DIM))
        .line_set(symbols::line::THICK)
        .label(dim_text(format!("core {index:<2} ")))
        .ratio(ratio);
    frame.render_widget(gauge, bar_area);

    let value = Paragraph::new(Line::from(Span::styled(
        format!("{percent:>3.0}%"),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    )))
    .alignment(Alignment::Right);
    frame.render_widget(value, percent_area);
}

fn draw_history(frame: &mut ratatui::Frame, area: Rect, app: &App, accent: ratatui::style::Color) {
    let block = panel("cpu history", accent);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }

    let [label_area, chart_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(inner);
    let total = app.cpu.latest.total_percent;
    let label = Line::from(Span::styled(
        format!("total  {total:.1}%"),
        Style::default().fg(accent).add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(Paragraph::new(label), label_area);

    let history: Vec<u64> = app
        .cpu_history
        .as_slice()
        .iter()
        .map(|value| (*value * 10.0) as u64)
        .collect();
    let sparkline = Sparkline::default()
        .data(&history)
        .style(Style::default().fg(threshold_color(total)));
    frame.render_widget(sparkline, chart_area);
}

fn draw_details(frame: &mut ratatui::Frame, area: Rect, app: &App, accent: ratatui::style::Color) {
    let block = panel("details", accent);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }

    let mut lines = vec![Line::from(dim_text("frequencies"))];

    let frequencies = &app.cpu.latest.frequencies_mhz;
    let columns = if inner.width >= 40 { 2 } else { 1 };
    for (chunk_index, chunk) in frequencies.chunks(columns).enumerate() {
        let mut spans = Vec::new();
        for (offset, frequency) in chunk.iter().enumerate() {
            if offset > 0 {
                spans.push(Span::raw("   "));
            }
            let index = chunk_index * columns + offset;
            spans.push(dim_text(format!("c{index}:")));
            spans.push(Span::raw(format!(" {frequency} MHz")));
        }
        lines.push(Line::from(spans));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(dim_text("temperature")));
    lines.push(Line::from(match app.cpu.latest.temperature_celsius {
        Some(temperature) => Span::styled(
            format!("{temperature:.1}°C"),
            Style::default()
                .fg(threshold_color(temperature))
                .add_modifier(Modifier::BOLD),
        ),
        None => dim_text("n/a"),
    }));

    frame.render_widget(Paragraph::new(lines), inner);
}
