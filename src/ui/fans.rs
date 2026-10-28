use super::{accent, braille, panel, render_empty};
use crate::app::App;
use crate::collectors::fans::FanSample;
use crate::history::RingBuffer;
use crate::theme::Palette;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let accent = accent(app);
    if app.fans.fans.is_empty() {
        render_empty(frame, area, "fans", "no fans detected", accent);
        return;
    }

    let constraints: Vec<Constraint> = app.fans.fans.iter().map(|_| Constraint::Fill(1)).collect();
    let areas = Layout::vertical(constraints).split(area);

    for (fan, area) in app.fans.fans.iter().zip(areas.iter()) {
        draw_fan(
            frame,
            *area,
            fan,
            app.fan_histories.get(&fan.name),
            accent,
            &app.palette,
        );
    }
}

fn draw_fan(
    frame: &mut Frame,
    area: Rect,
    fan: &FanSample,
    history: Option<&RingBuffer>,
    accent: ratatui::style::Color,
    palette: &Palette,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let title = match fan.max_rpm {
        Some(max) => format!("{} — {} / {} rpm", fan.name.to_lowercase(), fan.rpm, max),
        None => format!("{} — {} rpm", fan.name.to_lowercase(), fan.rpm),
    };
    let block = panel(title, accent);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(history) = history else {
        return;
    };
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    // The sensor's own maximum is the chart ceiling; without one, autoscale.
    match fan.max_rpm.filter(|&max| max > 0) {
        Some(max) => {
            let percent = fan.rpm as f32 / max as f32 * 100.0;
            braille::render_fixed(
                frame,
                inner,
                history,
                1.0,
                max,
                Style::default().fg(palette.threshold(percent)),
            );
        }
        None => braille::render(
            frame,
            inner,
            history,
            1.0,
            1,
            Style::default().fg(palette.accent),
        ),
    }
}

#[cfg(test)]
mod tests {
    use crate::app::App;
    use crate::collectors::fans::FanSample;
    use crate::config::Config;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn fan_panel_shows_rpm_and_maximum() {
        let mut app = App::new(&Config::default(), 80).expect("app");
        app.fans.fans = vec![FanSample {
            name: "cpu fan".to_string(),
            rpm: 1800,
            max_rpm: Some(3700),
        }];
        let mut history = crate::history::RingBuffer::new(160);
        for rpm in [1000.0, 1500.0, 1800.0] {
            history.push(rpm);
        }
        app.fan_histories.insert("cpu fan".to_string(), history);

        let mut terminal = Terminal::new(TestBackend::new(80, 20)).expect("terminal");
        terminal
            .draw(|frame| super::draw(frame, frame.area(), &app))
            .expect("draw");
        let buffer = terminal.backend().buffer();
        let text: String = buffer.content().iter().map(|cell| cell.symbol()).collect();
        assert!(text.contains("cpu fan — 1800 / 3700 rpm"), "{text}");
    }
}
