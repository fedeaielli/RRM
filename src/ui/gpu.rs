use super::{DIM, accent, dim_text, panel, render_empty, threshold_color};
use crate::app::App;
use ratatui::layout::{Alignment, Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{LineGauge, Paragraph};

pub fn draw(frame: &mut ratatui::Frame, area: Rect, app: &App) {
    let accent = accent(app);

    let Some(gpu) = app.gpu.as_ref() else {
        render_empty(frame, area, "gpu", "no gpu detected", accent);
        return;
    };

    let usage = gpu.usage_percent().ok();
    let vram_used = gpu.vram_used_mb().ok();
    let vram_total = gpu.vram_total_mb().ok();
    let temperature = gpu.temperature_celsius().ok();

    let outer = panel(format!("gpu — {}", gpu.name().to_lowercase()), accent);
    let inner = outer.inner(area);
    frame.render_widget(outer, area);
    if inner.height == 0 {
        return;
    }
    let inner = inner.inner(Margin::new(1, 0));
    if inner.height == 0 {
        return;
    }

    let is_intel_limited = usage.is_none() && (vram_total.is_none() || vram_total == Some(0));
    if is_intel_limited {
        draw_intel_info(frame, inner, temperature, accent);
        return;
    }

    let [usage_area, vram_area, temperature_area] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Length(1),
    ])
    .areas(inner);

    draw_usage(frame, usage_area, usage);
    draw_vram(frame, vram_area, vram_used, vram_total, accent);
    draw_temperature(frame, temperature_area, temperature);
}

fn draw_intel_info(
    frame: &mut ratatui::Frame,
    area: Rect,
    temperature: Option<f32>,
    accent: Color,
) {
    let [_, text_area] = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);

    let mut lines = vec![
        Line::from(Span::styled(
            "intel integrated graphics",
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        )),
        Line::from(dim_text(
            "usage and vram usage are not available via sysfs on this gpu.",
        )),
    ];
    if let Some(temperature) = temperature {
        lines.push(Line::from(vec![
            dim_text("temperature:  "),
            Span::styled(
                format!("{temperature:.1}°C"),
                Style::default()
                    .fg(threshold_color(temperature))
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(dim_text(
        "to monitor intel gpu activity, consider:",
    )));
    lines.push(Line::from("  intel_gpu_top"));
    lines.push(Line::from(dim_text("  package: intel-gpu-tools")));
    lines.push(Line::from("  sudo intel_gpu_top"));

    frame.render_widget(Paragraph::new(lines), text_area);
}

fn draw_usage(frame: &mut ratatui::Frame, area: Rect, usage: Option<f32>) {
    if area.height == 0 {
        return;
    }
    let [label_area, gauge_row] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(area);
    frame.render_widget(Paragraph::new(dim_text("usage")), label_area);

    let [gauge_area, suffix_area] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(10)]).areas(gauge_row);

    let (ratio, color, suffix, style) = match usage {
        Some(value) => {
            let color = threshold_color(value);
            (
                (f64::from(value) / 100.0).clamp(0.0, 1.0),
                color,
                format!("{value:.1}%"),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )
        }
        None => (0.0, DIM, "n/a".to_string(), Style::default().fg(DIM)),
    };

    let gauge = LineGauge::default()
        .filled_style(Style::default().fg(color))
        .unfilled_style(Style::default().fg(DIM))
        .line_set(symbols::line::THICK)
        .label("")
        .ratio(ratio);
    frame.render_widget(gauge, gauge_area);
    frame.render_widget(
        Paragraph::new(Span::styled(suffix, style)).alignment(Alignment::Right),
        suffix_area,
    );
}

fn draw_vram(
    frame: &mut ratatui::Frame,
    area: Rect,
    used: Option<u64>,
    total: Option<u64>,
    accent: Color,
) {
    if area.height == 0 {
        return;
    }
    let [label_area, gauge_row] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(area);
    frame.render_widget(Paragraph::new(dim_text("vram")), label_area);

    let [gauge_area, suffix_area] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(24)]).areas(gauge_row);

    let (ratio, color, suffix, style) = match (used, total) {
        (Some(used), Some(total)) => {
            let ratio = if total == 0 {
                0.0
            } else {
                (used as f64 / total as f64).clamp(0.0, 1.0)
            };
            (
                ratio,
                accent,
                format!("{used} MiB / {total} MiB"),
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            )
        }
        _ => (0.0, DIM, "n/a".to_string(), Style::default().fg(DIM)),
    };

    let gauge = LineGauge::default()
        .filled_style(Style::default().fg(color))
        .unfilled_style(Style::default().fg(DIM))
        .line_set(symbols::line::THICK)
        .label("")
        .ratio(ratio);
    frame.render_widget(gauge, gauge_area);
    frame.render_widget(
        Paragraph::new(Span::styled(suffix, style)).alignment(Alignment::Right),
        suffix_area,
    );
}

fn draw_temperature(frame: &mut ratatui::Frame, area: Rect, temperature: Option<f32>) {
    if area.height == 0 {
        return;
    }
    let [label_area, value_area] =
        Layout::horizontal([Constraint::Length(12), Constraint::Min(0)]).areas(area);
    frame.render_widget(Paragraph::new(dim_text("temp")), label_area);

    let text = match temperature {
        Some(value) => Line::from(Span::styled(
            format!("{value:.1}°C"),
            Style::default()
                .fg(threshold_color(value))
                .add_modifier(Modifier::BOLD),
        )),
        None => Line::from(dim_text("n/a")),
    };
    frame.render_widget(Paragraph::new(text), value_area);
}
