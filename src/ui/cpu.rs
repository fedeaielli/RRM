use super::{DIM, accent, braille, dim_text, panel, render_empty, threshold_color};
use crate::app::App;
use crate::history::RingBuffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph};

/// Percent history is stored times 10; never normalize below 5% so idle cores
/// keep a visible, low waveform instead of a full-height block.
const MIN_AUTOSCALE: u64 = 50;

pub fn draw(frame: &mut ratatui::Frame, area: Rect, app: &App) {
    let accent = accent(app);
    if app.cpu.latest.per_core_percent.is_empty() {
        render_empty(frame, area, "cpu", "no cpu data", accent);
        return;
    }

    let [top_area, grid_area] =
        Layout::vertical([Constraint::Length(6), Constraint::Min(0)]).areas(area);
    draw_top(frame, top_area, app, accent);
    draw_core_grid(frame, grid_area, app);
}

fn draw_top(frame: &mut ratatui::Frame, area: Rect, app: &App, accent: Color) {
    let block = panel("cpu", accent);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 || inner.width == 0 {
        return;
    }

    let [header_area, chart_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(inner);

    let total = app.cpu.latest.total_percent;
    let mut spans = vec![Span::styled(
        format!("total  {total:.1}%"),
        Style::default()
            .fg(threshold_color(total))
            .add_modifier(Modifier::BOLD),
    )];
    if let Some(avg_mhz) = average_frequency(&app.cpu.latest.frequencies_mhz) {
        spans.push(dim_text("   "));
        spans.push(dim_text(format!("{avg_mhz} MHz avg")));
    }
    if let Some(temperature) = app.cpu.latest.temperature_celsius {
        spans.push(dim_text("   "));
        spans.push(Span::styled(
            format!("{temperature:.1}°C"),
            Style::default().fg(threshold_color(temperature)),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), header_area);

    braille::render(
        frame,
        chart_area,
        &app.cpu_history,
        10.0,
        MIN_AUTOSCALE,
        Style::default().fg(threshold_color(total)),
    );
}

fn average_frequency(frequencies_mhz: &[u64]) -> Option<u64> {
    let reported: Vec<u64> = frequencies_mhz
        .iter()
        .copied()
        .filter(|value| *value > 0)
        .collect();
    reported
        .iter()
        .sum::<u64>()
        .checked_div(reported.len() as u64)
}

fn draw_core_grid(frame: &mut ratatui::Frame, area: Rect, app: &App) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let num_cols = (area.width / 26).max(1) as usize;
    let cell_w = area.width / num_cols as u16;
    let rows = app
        .cpu
        .latest
        .per_core_percent
        .len()
        .div_ceil(num_cols)
        .max(1) as u16;
    // Braille yields four dot rows per cell, so taller cells keep the waveform smooth.
    let cell_h = (area.height / rows).clamp(5, 7);
    let bottom = area.y.saturating_add(area.height);

    for (index, &percent) in app.cpu.latest.per_core_percent.iter().enumerate() {
        let col = (index % num_cols) as u16;
        let row = (index / num_cols) as u16;
        let y = area.y.saturating_add(row * cell_h);
        if y >= bottom {
            break;
        }
        let height = cell_h.min(bottom - y);
        // A cell needs both borders plus one row of chart to be worth drawing.
        if height < 3 {
            break;
        }
        let cell = Rect {
            x: area.x.saturating_add(col * cell_w),
            y,
            width: cell_w,
            height,
        };
        draw_core_cell(
            frame,
            cell,
            index,
            percent,
            app.cpu_core_histories.get(index),
        );
    }
}

fn draw_core_cell(
    frame: &mut ratatui::Frame,
    area: Rect,
    index: usize,
    percent: f32,
    history: Option<&RingBuffer>,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let color = threshold_color(percent);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(DIM))
        .title_top(Span::styled(
            format!(" core {index} "),
            Style::default().fg(DIM),
        ))
        .title_top(
            Line::from(Span::styled(
                format!(" {percent:.0}% "),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ))
            .right_aligned(),
        );

    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 || inner.width == 0 {
        return;
    }

    let Some(history) = history else {
        return;
    };
    braille::render(
        frame,
        inner,
        history,
        10.0,
        MIN_AUTOSCALE,
        Style::default().fg(color),
    );
}
