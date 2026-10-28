use super::{DIM, accent, braille, dim_text, panel, render_empty};
use crate::app::App;
use crate::history::RingBuffer;
use crate::theme::Palette;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Modifier, Style};
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
        Layout::vertical([Constraint::Length(8), Constraint::Min(0)]).areas(area);
    draw_top(frame, top_area, app);
    draw_core_grid(frame, grid_area, app);
}

fn draw_top(frame: &mut ratatui::Frame, area: Rect, app: &App) {
    let block = panel(format!("cpu — {}", app.cpu.model), app.palette.banner)
        .border_style(Style::default().fg(app.palette.banner));
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
            .fg(app.palette.threshold(total))
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
            Style::default().fg(app.palette.threshold(temperature)),
        ));
    }
    let [_, text_area] =
        Layout::horizontal([Constraint::Length(2), Constraint::Min(0)]).areas(header_area);
    frame.render_widget(Paragraph::new(Line::from(spans)), text_area);

    braille::render(
        frame,
        chart_area,
        &app.cpu_history,
        10.0,
        MIN_AUTOSCALE,
        Style::default().fg(app.palette.threshold(total)),
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

    let num_cols = (area.width / 26).max(1);
    let widths = core_cell_widths(area.width, num_cols);
    let rows = app
        .cpu
        .latest
        .per_core_percent
        .len()
        .div_ceil(num_cols as usize)
        .max(1) as u16;
    // Braille yields four dot rows per cell, so taller cells keep the waveform smooth.
    let cell_h = (area.height / rows).clamp(4, 6);
    let bottom = area.y.saturating_add(area.height);

    for (index, &percent) in app.cpu.latest.per_core_percent.iter().enumerate() {
        let col = index % num_cols as usize;
        let row = (index / num_cols as usize) as u16;
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
            x: area.x.saturating_add(widths[..col].iter().sum::<u16>()),
            y,
            width: widths[col],
            height,
        };
        draw_core_cell(
            frame,
            cell,
            index,
            percent,
            app.cpu_core_histories.get(index),
            &app.palette,
        );
    }
}

fn draw_core_cell(
    frame: &mut ratatui::Frame,
    area: Rect,
    index: usize,
    percent: f32,
    history: Option<&RingBuffer>,
    palette: &Palette,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let color = palette.threshold(percent);
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

    let inner = block.inner(area).inner(Margin {
        horizontal: 1,
        vertical: 0,
    });
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

/// Split `width` into `columns` cells, giving the spare columns to the middle
/// ones so the outer cells stay the same size.
fn core_cell_widths(width: u16, columns: u16) -> Vec<u16> {
    if columns == 0 {
        return Vec::new();
    }
    let base = width / columns;
    let extra = width % columns;
    let center = (columns - 1) / 2;
    let mut order: Vec<u16> = (0..columns).collect();
    order.sort_by_key(|index| (index.abs_diff(center), *index));

    let mut widths = vec![base; usize::from(columns)];
    for index in order.into_iter().take(usize::from(extra)) {
        widths[usize::from(index)] += 1;
    }
    widths
}

#[cfg(test)]
mod tests {
    use super::core_cell_widths;

    #[test]
    fn cell_widths_fill_the_row() {
        for (width, columns) in [(100, 3), (99, 3), (80, 4), (26, 1), (5, 2)] {
            let widths = core_cell_widths(width, columns);
            assert_eq!(widths.len(), usize::from(columns));
            assert_eq!(widths.iter().sum::<u16>(), width);
            let max = widths.iter().copied().max().unwrap_or(0);
            let min = widths.iter().copied().min().unwrap_or(0);
            assert!(max - min <= 1);
            if columns % 2 == 1 {
                assert_eq!(widths[0], widths[widths.len() - 1]);
            }
        }
        assert!(core_cell_widths(10, 0).is_empty());
    }
}
