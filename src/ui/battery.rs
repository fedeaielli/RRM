use super::{CRIT, CYAN, DIM, GOOD, WARN, accent, dim_text, panel, render_empty};
use crate::app::App;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{LineGauge, Paragraph};

pub fn draw(frame: &mut ratatui::Frame, area: Rect, app: &App) {
    let accent = accent(app);
    if app.battery.batteries.is_empty() {
        render_empty(frame, area, "battery", "no battery", accent);
        return;
    }

    let count = app.battery.batteries.len();
    let constraints: Vec<Constraint> = (0..count)
        .map(|_| Constraint::Ratio(1, count as u32))
        .collect();
    let battery_areas = Layout::vertical(constraints).split(area);

    for (battery, battery_area) in app.battery.batteries.iter().zip(battery_areas.iter()) {
        let block = panel(format!("battery — {}", battery.name.to_lowercase()), accent);
        let inner = block.inner(*battery_area);
        frame.render_widget(block, *battery_area);
        if inner.height == 0 {
            continue;
        }

        let [charge_area, status_area, health_area, big_charge_area] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .areas(inner);

        let color = status_color(&battery.status, battery.capacity_percent);
        draw_charge(frame, charge_area, battery, color);
        draw_status(frame, status_area, battery, color);
        draw_health(frame, health_area, battery);
        draw_big_charge(frame, big_charge_area, battery.capacity_percent, color);
    }
}

fn draw_charge(
    frame: &mut ratatui::Frame,
    area: Rect,
    battery: &crate::collectors::battery::BatterySample,
    color: Color,
) {
    let [_, gauge_area, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .areas(area);

    let label = Line::from(Span::styled(
        format!("  {}%  —  {}  ", battery.capacity_percent, battery.status),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    ));
    let gauge = LineGauge::default()
        .filled_style(Style::default().fg(color))
        .unfilled_style(Style::default().fg(DIM))
        .line_set(symbols::line::THICK)
        .label(label)
        .ratio((f64::from(battery.capacity_percent) / 100.0).clamp(0.01, 1.0));
    frame.render_widget(gauge, gauge_area);
}

fn draw_status(
    frame: &mut ratatui::Frame,
    area: Rect,
    battery: &crate::collectors::battery::BatterySample,
    color: Color,
) {
    let line = Line::from(vec![
        dim_text("status: "),
        Span::styled(
            battery.status.clone(),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        dim_text("   capacity: "),
        Span::styled(
            format!("{}%", battery.capacity_percent),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn draw_health(
    frame: &mut ratatui::Frame,
    area: Rect,
    battery: &crate::collectors::battery::BatterySample,
) {
    let health = match (battery.energy_full_uwh, battery.energy_full_design_uwh) {
        (Some(full), Some(design)) if design > 0 => Some(full as f32 / design as f32 * 100.0),
        _ => None,
    };

    let line = match health {
        Some(value) => Line::from(vec![
            dim_text("health: "),
            Span::styled(
                format!("{value:.1}%"),
                Style::default()
                    .fg(health_color(value))
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        None => Line::from(dim_text(
            "health: n/a  (energy_full_design not reported by this battery)",
        )),
    };
    frame.render_widget(Paragraph::new(line), area);
}

fn draw_big_charge(frame: &mut ratatui::Frame, area: Rect, capacity_percent: u8, color: Color) {
    let [_, number_area, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .areas(area);

    let big = format!("{capacity_percent}");
    let paragraph = Paragraph::new(big)
        .alignment(Alignment::Center)
        .style(Style::default().fg(color).add_modifier(Modifier::BOLD));
    frame.render_widget(paragraph, number_area);
}

fn status_color(status: &str, capacity_percent: u8) -> Color {
    match status {
        "Charging" => CYAN,
        "Full" => GOOD,
        "Discharging" => WARN,
        "Not charging" if capacity_percent > 50 => CYAN,
        _ => DIM,
    }
}

fn health_color(health: f32) -> Color {
    if health > 80.0 {
        GOOD
    } else if health >= 50.0 {
        WARN
    } else {
        CRIT
    }
}
