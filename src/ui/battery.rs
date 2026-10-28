use super::{DIM, accent, braille, dim_text, panel, render_empty, segmented_bar};
use crate::app::App;
use crate::collectors::battery::BatterySample;
use crate::history::RingBuffer;
use crate::theme::Palette;
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
        let [charge_area, health_area, history_area] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(4),
        ])
        .areas(*battery_area);

        draw_charge(frame, charge_area, battery, accent, &app.palette);
        draw_health(
            frame,
            health_area,
            health_percent(battery),
            accent,
            &app.palette,
        );

        let history = if index == 0 {
            Some(&app.battery_power_history)
        } else {
            None
        };
        draw_power(
            frame,
            history_area,
            battery.power_now_uw,
            history,
            accent,
            &app.palette,
        );
    }
}

fn draw_charge(
    frame: &mut ratatui::Frame,
    area: Rect,
    battery: &BatterySample,
    accent: Color,
    palette: &Palette,
) {
    let block = panel(format!("{} — charge", battery.name.to_lowercase()), accent);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }

    let [bar_area, value_area] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(18)]).areas(inner);

    let pct = battery.capacity_percent;
    let status = battery.status.as_str();
    frame.render_widget(
        Paragraph::new(segmented_bar(f32::from(pct), bar_area.width, |_| {
            charge_color(status, pct, palette)
        })),
        bar_area,
    );
    frame.render_widget(
        Paragraph::new(Span::styled(
            format!("{pct:>3}%  {status:<12}"),
            Style::default()
                .fg(charge_color(status, pct, palette))
                .add_modifier(Modifier::BOLD),
        )),
        value_area,
    );
}

fn draw_health(
    frame: &mut ratatui::Frame,
    area: Rect,
    health: Option<f32>,
    accent: Color,
    palette: &Palette,
) {
    let block = panel("health", accent);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }

    let Some(health) = health else {
        frame.render_widget(
            Paragraph::new(dim_text("n/a  (charge_full_design not reported)")),
            inner,
        );
        return;
    };

    let [bar_area, value_area] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(18)]).areas(inner);

    frame.render_widget(
        Paragraph::new(segmented_bar(health, bar_area.width, |_| {
            health_color(health, palette)
        })),
        bar_area,
    );
    frame.render_widget(
        Paragraph::new(Span::styled(
            format!("{:>6.1}%  {}", health, health_label(health)),
            Style::default()
                .fg(health_color(health, palette))
                .add_modifier(Modifier::BOLD),
        )),
        value_area,
    );
}

fn draw_power(
    frame: &mut ratatui::Frame,
    area: Rect,
    power_uw: Option<u64>,
    history: Option<&RingBuffer>,
    accent: Color,
    palette: &Palette,
) {
    let title = power_uw.map_or_else(
        || "power draw  n/a".to_string(),
        |uw| format!("power draw  {:.1} W", uw as f32 / 1_000_000.0),
    );
    let block = panel(title, accent);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let (Some(history), Some(uw)) = (history, power_uw) else {
        return;
    };
    let color = palette.threshold(uw as f32 / 1_000_000.0);
    // History is in mW: fixed 0-30 W scale.
    braille::render_fixed(
        frame,
        inner,
        history,
        1.0,
        30_000,
        Style::default().fg(color),
    );
}

fn health_percent(battery: &BatterySample) -> Option<f32> {
    match (battery.energy_full_uwh, battery.energy_full_design_uwh) {
        (Some(full), Some(design)) if design > 0 => Some(full as f32 / design as f32 * 100.0),
        _ => None,
    }
}

fn charge_color(status: &str, pct: u8, palette: &Palette) -> Color {
    match status {
        "Charging" | "Not charging" => palette.rx,
        "Full" => palette.good,
        "Discharging" => {
            if pct > 50 {
                palette.good
            } else if pct > 20 {
                palette.warn
            } else {
                palette.crit
            }
        }
        _ => DIM,
    }
}

fn health_color(health: f32, palette: &Palette) -> Color {
    if health > 80.0 {
        palette.good
    } else if health > 50.0 {
        palette.warn
    } else {
        palette.crit
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

#[cfg(test)]
mod tests {
    use crate::app::App;
    use crate::config::Config;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;

    #[test]
    fn renders_three_sections_and_history_chart() {
        let mut app = App::new(&Config::default(), 80).unwrap();
        if app.battery.batteries.is_empty() {
            return;
        }
        for i in 0..160 {
            app.battery_power_history
                .push(5000.0 + (i % 30) as f32 * 500.0);
        }

        let mut terminal = Terminal::new(TestBackend::new(80, 16)).unwrap();
        terminal
            .draw(|f| super::draw(f, Rect::new(0, 0, 80, 16), &app))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let text: String = (0..16)
            .flat_map(|y| (0..80).map(move |x| (x, y)))
            .map(|(x, y)| buffer[(x, y)].symbol().to_string())
            .collect();
        assert!(text.contains("charge"));
        assert!(text.contains("health"));
        assert!(text.contains("power draw"));
        assert!(text.chars().any(|c| c >= '\u{2800}'));
        assert!(text.contains('█'), "battery bars should be solid blocks");
    }
}
