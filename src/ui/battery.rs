use super::{
    CRIT, DIM, GOOD, RX, WARN, accent, braille, dim_text, panel, render_empty, segmented_bar,
};
use crate::app::App;
use crate::history::RingBuffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::Paragraph;

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

    for (index, (battery, battery_area)) in app
        .battery
        .batteries
        .iter()
        .zip(battery_areas.iter())
        .enumerate()
    {
        let block = panel(format!("battery — {}", battery.name.to_lowercase()), accent);
        let inner = block.inner(*battery_area);
        frame.render_widget(block, *battery_area);
        if inner.height == 0 {
            continue;
        }

        let [
            charge_area,
            health_area,
            _,
            history_label_area,
            history_area,
        ] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(2),
        ])
        .areas(inner);

        let color = charge_color(&battery.status, battery.capacity_percent);

        draw_charge_row(
            frame,
            charge_area,
            battery.capacity_percent,
            &battery.status,
        );
        draw_health_row(frame, health_area, health_percent(battery));

        let history = if index == 0 {
            Some(&app.battery_charge_history)
        } else {
            None
        };
        draw_history(
            frame,
            history_label_area,
            history_area,
            battery.capacity_percent,
            color,
            history,
        );
    }
}

fn draw_charge_row(frame: &mut ratatui::Frame, area: Rect, pct: u8, status: &str) {
    if area.height == 0 {
        return;
    }

    let [label_area, gauge_area, value_area] = Layout::horizontal([
        Constraint::Length(10),
        Constraint::Min(0),
        Constraint::Length(18),
    ])
    .areas(area);

    frame.render_widget(Paragraph::new(dim_text("charge")), label_area);
    frame.render_widget(
        Paragraph::new(segmented_bar(f32::from(pct), gauge_area.width, |level| {
            charge_color(status, level as u8)
        })),
        gauge_area,
    );

    let value = Paragraph::new(Span::styled(
        format!("{pct:>3}%  {status:<12}"),
        Style::default()
            .fg(charge_color(status, pct))
            .add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(value, value_area);
}

fn draw_health_row(frame: &mut ratatui::Frame, area: Rect, health: Option<f32>) {
    if area.height == 0 {
        return;
    }

    let Some(health) = health else {
        let paragraph = Paragraph::new(dim_text("health  n/a  (charge_full_design not reported)"));
        frame.render_widget(paragraph, area);
        return;
    };

    let color = health_color(health);
    let [label_area, gauge_area, value_area] = Layout::horizontal([
        Constraint::Length(10),
        Constraint::Min(0),
        Constraint::Length(18),
    ])
    .areas(area);

    frame.render_widget(Paragraph::new(dim_text("health")), label_area);
    frame.render_widget(
        Paragraph::new(segmented_bar(health, gauge_area.width, health_color)),
        gauge_area,
    );

    let value = Paragraph::new(Span::styled(
        format!("{:>6.1}%  {}", health, health_label(health)),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(value, value_area);
}

fn draw_history(
    frame: &mut ratatui::Frame,
    label_area: Rect,
    chart_area: Rect,
    pct: u8,
    color: Color,
    history: Option<&RingBuffer>,
) {
    if label_area.height == 0 {
        return;
    }

    let [title_area, value_area] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(8)]).areas(label_area);
    frame.render_widget(Paragraph::new(dim_text("charge history")), title_area);
    frame.render_widget(
        Paragraph::new(Span::styled(
            format!("{pct:>3}%"),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ))
        .alignment(ratatui::layout::Alignment::Right),
        value_area,
    );

    if chart_area.height == 0 {
        return;
    }
    let Some(history) = history else {
        return;
    };
    // Fixed 0-100% scale, like the memory charts.
    braille::render(
        frame,
        chart_area,
        history,
        10.0,
        1000,
        Style::default().fg(color),
    );
}

fn health_percent(battery: &crate::collectors::battery::BatterySample) -> Option<f32> {
    match (battery.energy_full_uwh, battery.energy_full_design_uwh) {
        (Some(full), Some(design)) if design > 0 => Some(full as f32 / design as f32 * 100.0),
        _ => None,
    }
}

fn charge_color(status: &str, pct: u8) -> Color {
    match status {
        "Charging" | "Not charging" => RX,
        "Full" => GOOD,
        "Discharging" => {
            if pct > 50 {
                GOOD
            } else if pct > 20 {
                WARN
            } else {
                CRIT
            }
        }
        _ => DIM,
    }
}

fn health_color(health: f32) -> Color {
    if health > 80.0 {
        GOOD
    } else if health > 50.0 {
        WARN
    } else {
        CRIT
    }
}

fn health_label(health: f32) -> &'static str {
    if health > 80.0 {
        "good"
    } else if health > 50.0 {
        "fair"
    } else {
        "poor"
    }
}
