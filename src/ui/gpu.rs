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

    let usage = gpu.usage_percent().ok();
    let vram_used = gpu.vram_used_mb().ok();
    let vram_total = gpu.vram_total_mb().ok();
    let temperature = gpu.temperature_celsius().ok();

    let [usage_row, vram_row, temperature_row, context_area] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Min(0),
    ])
    .areas(inner);

    draw_usage(frame, usage_row, usage);
    draw_vram(frame, vram_row, vram_used, vram_total, accent);
    draw_temperature(frame, temperature_row, temperature);

    let missing = usize::from(usage.is_none())
        + usize::from(vram_used.is_none())
        + usize::from(temperature.is_none());
    let intel_usage_unavailable = usage == Some(0.0) && vram_total == Some(0);
    if intel_usage_unavailable || missing >= 2 {
        let [_, text_area, _] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .areas(context_area);
        let message = if intel_usage_unavailable {
            "note: intel igpu — usage via sysfs not available"
        } else {
            "intel gpu detected — usage and vram not exposed via sysfs"
        };
        let paragraph = Paragraph::new(dim_text(message)).alignment(Alignment::Center);
        frame.render_widget(paragraph, text_area);
    }
}

fn draw_usage(frame: &mut ratatui::Frame, area: Rect, usage: Option<f32>) {
    let [_, line_area, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .areas(area);
    let [label_area, gauge_area, suffix_area] = Layout::horizontal([
        Constraint::Length(12),
        Constraint::Min(0),
        Constraint::Length(10),
    ])
    .areas(line_area);

    frame.render_widget(Paragraph::new(dim_text("usage")), label_area);

    let (ratio, color, suffix, style) = match usage {
        Some(value) => {
            let color = threshold_color(value);
            (
                (f64::from(value) / 100.0).clamp(0.0, 1.0),
                color,
                format!("  {value:.1}%"),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )
        }
        None => (0.0, DIM, "  n/a".to_string(), Style::default().fg(DIM)),
    };

    let gauge = LineGauge::default()
        .filled_style(Style::default().fg(color))
        .unfilled_style(Style::default().fg(DIM))
        .line_set(symbols::line::THICK)
        .label("")
        .ratio(ratio);
    frame.render_widget(gauge, gauge_area);
    frame.render_widget(Paragraph::new(Span::styled(suffix, style)), suffix_area);
}

fn draw_vram(
    frame: &mut ratatui::Frame,
    area: Rect,
    used: Option<u64>,
    total: Option<u64>,
    accent: Color,
) {
    let [_, line_area, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .areas(area);
    let [label_area, gauge_area, suffix_area] = Layout::horizontal([
        Constraint::Length(12),
        Constraint::Min(0),
        Constraint::Length(26),
    ])
    .areas(line_area);

    frame.render_widget(Paragraph::new(dim_text("vram")), label_area);

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
                format!("  {used} MiB / {total} MiB"),
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            )
        }
        _ => (0.0, DIM, "  n/a".to_string(), Style::default().fg(DIM)),
    };

    let gauge = LineGauge::default()
        .filled_style(Style::default().fg(color))
        .unfilled_style(Style::default().fg(DIM))
        .line_set(symbols::line::THICK)
        .label("")
        .ratio(ratio);
    frame.render_widget(gauge, gauge_area);
    frame.render_widget(Paragraph::new(Span::styled(suffix, style)), suffix_area);
}

fn draw_temperature(frame: &mut ratatui::Frame, area: Rect, temperature: Option<f32>) {
    let [_, line_area, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .areas(area);

    let Some(value) = temperature else {
        let paragraph = Paragraph::new(dim_text("temp: check hwmon config"));
        frame.render_widget(paragraph, line_area);
        return;
    };

    let [label_area, value_area] =
        Layout::horizontal([Constraint::Length(12), Constraint::Min(0)]).areas(line_area);

    frame.render_widget(Paragraph::new(dim_text("temp")), label_area);

    let text = format!("  {value:.1}°C");
    let style = Style::default()
        .fg(threshold_color(value))
        .add_modifier(Modifier::BOLD);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(text, style))),
        value_area,
    );
}
