use super::{DIM, accent, braille, panel, render_empty};
use crate::app::App;
use crate::theme::Palette;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

pub fn draw(frame: &mut ratatui::Frame, area: Rect, app: &App) {
    let accent = accent(app);

    let Some(gpu) = app.gpu.as_ref() else {
        render_empty(frame, area, "gpu", "no gpu detected", accent);
        return;
    };

    let usage = app.gpu_usage;
    let vram_used = gpu.vram_used_mb().ok();
    let vram_total = gpu.vram_total_mb().ok();
    let temperature = gpu.temperature_celsius().ok();
    let frequency = gpu.frequency_mhz().ok();
    let driver = gpu.driver();
    let power_state = gpu.power_state();

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

    let [banner_area, usage_area, temperature_area] = Layout::vertical([
        Constraint::Length(6),
        Constraint::Fill(1),
        Constraint::Fill(1),
    ])
    .areas(inner);

    draw_banner(
        frame,
        banner_area,
        Banner {
            driver,
            power_state,
            frequency,
            vram_used,
            vram_total,
            system_gib: app.memory.latest.total_kb / 1_048_576,
            temperature,
            temp_is_proxy: gpu.temperature_is_proxy(),
        },
        accent,
        &app.palette,
    );

    let usage_title = usage.map_or_else(
        || "usage  n/a".to_string(),
        |value| format!("usage  {value:.1}%"),
    );
    draw_chart(
        frame,
        usage_area,
        usage_title,
        usage.map_or(0.0, |value| value),
        &app.gpu_usage_history,
        "no usage data",
        &app.palette,
    );

    let temp_title = if gpu.temperature_is_proxy() {
        "temperature (cpu package)"
    } else {
        "temperature"
    };
    let temp_title = temperature.map_or_else(
        || format!("{temp_title}  n/a"),
        |value| format!("{temp_title}  {value:.1}°C"),
    );
    draw_chart(
        frame,
        temperature_area,
        temp_title,
        temperature.map_or(0.0, |value| value),
        &app.gpu_temp_history,
        "no temperature sensor",
        &app.palette,
    );
}

struct Banner<'a> {
    driver: Option<&'a str>,
    power_state: Option<String>,
    frequency: Option<(u64, u64)>,
    vram_used: Option<u64>,
    vram_total: Option<u64>,
    system_gib: u64,
    temperature: Option<f32>,
    temp_is_proxy: bool,
}

fn draw_banner(
    frame: &mut ratatui::Frame,
    area: Rect,
    banner: Banner<'_>,
    accent: Color,
    palette: &Palette,
) {
    let block = panel("info", accent);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }

    let mut driver = banner.driver.unwrap_or("n/a").to_string();
    if let Some(state) = banner.power_state {
        driver.push_str(" · ");
        driver.push_str(&state);
    }
    let clock = banner.frequency.map_or_else(
        || "n/a".to_string(),
        |(current, max)| format!("{current} / {max} MHz"),
    );
    let memory = match (banner.vram_used, banner.vram_total) {
        (Some(used), Some(total)) if total > 0 => format!("{used} MiB / {total} MiB"),
        _ => format!("shared — {} GiB system RAM", banner.system_gib),
    };
    let temp = match banner.temperature {
        Some(value) if banner.temp_is_proxy => format!("{value:.1}°C (cpu package)"),
        Some(value) => format!("{value:.1}°C"),
        None => "n/a".to_string(),
    };

    let lines = vec![
        info_line("driver", driver, accent),
        info_line("clock", clock, accent),
        info_line("memory", memory, accent),
        info_line(
            "temp",
            temp,
            banner
                .temperature
                .map_or(DIM, |value| palette.threshold(value)),
        ),
    ];
    frame.render_widget(Paragraph::new(lines), inner);
}

fn info_line(label: &str, value: String, color: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<10}"), Style::default().fg(DIM)),
        Span::styled(
            value,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
    ])
}

fn draw_chart(
    frame: &mut ratatui::Frame,
    area: Rect,
    title: String,
    value: f32,
    history: &crate::history::RingBuffer,
    empty_message: &str,
    palette: &Palette,
) {
    if history.as_slice().is_empty() {
        render_empty(frame, area, &title, empty_message, palette.accent);
        return;
    }
    let block = panel(title, palette.accent);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    braille::render_fixed(
        frame,
        inner,
        history,
        10.0,
        1000,
        Style::default().fg(palette.threshold(value)),
    );
}

#[cfg(test)]
mod tests {
    use crate::app::App;
    use crate::config::Config;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn usage_title_uses_ticked_sample() {
        let mut app = App::new(&Config::default(), 80).unwrap();
        if app.gpu.is_none() {
            return;
        }
        app.gpu_usage = Some(42.0);
        for _ in 0..160 {
            app.gpu_usage_history.push(42.0);
        }

        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|f| super::draw(f, f.area(), &app)).unwrap();
        let buffer = terminal.backend().buffer();
        let text: String = (0..24)
            .flat_map(|y| (0..80).map(move |x| (x, y)))
            .map(|(x, y)| buffer[(x, y)].symbol().to_string())
            .collect();
        assert!(text.contains("usage  42.0%"));
    }
}
