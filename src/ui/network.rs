use super::{RX, TX, accent, braille, dim_text, format_rate, panel, render_empty};
use crate::app::App;
use crate::history::RingBuffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

const MAX_SHOWN: usize = 3;
const MIN_BLOCK_HEIGHT: u16 = 9;

pub fn draw(frame: &mut ratatui::Frame, area: Rect, app: &App) {
    let accent = accent(app);
    if app.network.latest.is_empty() {
        render_empty(frame, area, "network", "no network interfaces", accent);
        return;
    }

    let max_by_height = (area.height / MIN_BLOCK_HEIGHT).max(1) as usize;
    let shown = app.network.latest.len().min(MAX_SHOWN).min(max_by_height);
    let hidden = app.network.latest.len() - shown;

    let mut constraints: Vec<Constraint> = (0..shown).map(|_| Constraint::Fill(1)).collect();
    if hidden > 0 {
        constraints.push(Constraint::Length(1));
    }
    let slices = Layout::vertical(constraints).split(area);

    for (sample, slice) in app.network.latest.iter().take(shown).zip(slices.iter()) {
        draw_interface(frame, *slice, sample, app, accent);
    }

    if hidden > 0 {
        let footer = Paragraph::new(dim_text(format!(
            "  and {hidden} more interfaces not shown"
        )));
        frame.render_widget(footer, slices[shown]);
    }
}

fn draw_interface(
    frame: &mut ratatui::Frame,
    area: Rect,
    sample: &crate::collectors::network::NetSample,
    app: &App,
    accent: ratatui::style::Color,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let block = panel(sample.interface.to_lowercase(), accent);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }

    let [rx_label_area, rx_chart_area, tx_label_area, tx_chart_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(2),
        Constraint::Length(1),
        Constraint::Min(2),
    ])
    .areas(inner);

    frame.render_widget(
        Paragraph::new(rate_line("rx", sample.rx_bytes_per_sec, RX)),
        rx_label_area,
    );
    draw_history(
        frame,
        rx_chart_area,
        app.net_rx_history.get(&sample.interface),
        RX,
    );

    frame.render_widget(
        Paragraph::new(rate_line("tx", sample.tx_bytes_per_sec, TX)),
        tx_label_area,
    );
    draw_history(
        frame,
        tx_chart_area,
        app.net_tx_history.get(&sample.interface),
        TX,
    );
}

fn draw_history(
    frame: &mut ratatui::Frame,
    area: Rect,
    history: Option<&RingBuffer>,
    color: Color,
) {
    let Some(history) = history else {
        return;
    };
    braille::render(
        frame,
        area,
        history,
        1.0 / 1024.0,
        1,
        Style::default().fg(color),
    );
}

fn rate_line(label: &str, rate: f64, color: ratatui::style::Color) -> Line<'static> {
    Line::from(vec![
        dim_text(format!("{label}  ")),
        ratatui::text::Span::styled(format_rate(rate), Style::default().fg(color)),
    ])
}
