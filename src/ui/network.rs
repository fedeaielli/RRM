use super::{DIM, accent, braille, dim_text, format_rate, panel, render_empty};
use crate::app::App;
use crate::history::RingBuffer;
use crate::theme::Palette;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
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

    let [rates_area, chart_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(inner);

    frame.render_widget(
        Paragraph::new(rates_line(
            sample.rx_bytes_per_sec,
            sample.tx_bytes_per_sec,
            &app.palette,
        )),
        rates_area,
    );
    draw_traffic(
        frame,
        chart_area,
        app.net_rx_history.get(&sample.interface),
        app.net_tx_history.get(&sample.interface),
        &app.palette,
    );
}

fn draw_traffic(
    frame: &mut ratatui::Frame,
    area: Rect,
    rx: Option<&RingBuffer>,
    tx: Option<&RingBuffer>,
    palette: &Palette,
) {
    let (Some(rx), Some(tx)) = (rx, tx) else {
        return;
    };
    braille::render_split(
        frame,
        area,
        rx,
        tx,
        1.0,
        1,
        Style::default().fg(palette.rx),
        Style::default().fg(palette.tx),
        Style::default().fg(super::DIM),
    );
}

fn rates_line(rx: f64, tx: f64, palette: &Palette) -> Line<'static> {
    Line::from(vec![
        dim_text("↓ rx  "),
        Span::styled(
            format_rate(rx),
            Style::default().fg(rate_color(rx, palette.rx)),
        ),
        dim_text("   "),
        dim_text("↑ tx  "),
        Span::styled(
            format_rate(tx),
            Style::default().fg(rate_color(tx, palette.tx)),
        ),
    ])
}

fn rate_color(rate: f64, color: Color) -> Color {
    if rate > 0.0 { color } else { DIM }
}
