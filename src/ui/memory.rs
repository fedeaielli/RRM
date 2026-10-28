use super::{accent, braille, dim_text, panel, render_empty, segmented_bar};
use crate::app::App;
use crate::history::RingBuffer;
use crate::theme::Palette;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::Paragraph;

pub fn draw(frame: &mut ratatui::Frame, area: Rect, app: &App) {
    let accent = accent(app);
    let memory = &app.memory.latest;

    if memory.total_kb == 0 && memory.swap_total_kb == 0 {
        render_empty(frame, area, "memory", "no memory data", accent);
        return;
    }

    let block = panel(
        format!("memory — {} GiB", memory.total_kb / 1_048_576),
        accent,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }

    let [ram_area, swap_area, _spacer_area, charts_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Min(4),
    ])
    .areas(inner);
    let [ram_chart_area, swap_chart_area] =
        Layout::vertical([Constraint::Fill(2), Constraint::Fill(1)]).areas(charts_area);

    let ram_percent = percent(memory.used_kb, memory.total_kb);
    draw_gauge_row(
        frame,
        ram_area,
        "RAM",
        memory.used_kb,
        memory.total_kb,
        ram_percent,
        accent,
        &app.palette,
    );
    draw_chart(
        frame,
        ram_chart_area,
        "ram usage",
        &app.mem_history,
        ram_percent,
        accent,
        &app.palette,
    );

    if memory.swap_total_kb == 0 {
        draw_disabled_row(frame, swap_area, "swap  not available");
        render_empty(
            frame,
            swap_chart_area,
            "swap usage",
            "not available",
            accent,
        );
    } else {
        let swap_percent = percent(memory.swap_used_kb, memory.swap_total_kb);
        draw_gauge_row(
            frame,
            swap_area,
            "SWAP",
            memory.swap_used_kb,
            memory.swap_total_kb,
            swap_percent,
            accent,
            &app.palette,
        );
        draw_chart(
            frame,
            swap_chart_area,
            "swap usage",
            &app.swap_history,
            swap_percent,
            accent,
            &app.palette,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_gauge_row(
    frame: &mut ratatui::Frame,
    area: Rect,
    label: &str,
    used_kb: u64,
    total_kb: u64,
    percent: f32,
    accent: Color,
    palette: &Palette,
) {
    if area.height == 0 {
        return;
    }

    let [_, row_area, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .areas(area);
    let [_, label_area, gauge_area, _, value_area, _] = Layout::horizontal([
        Constraint::Length(1),
        Constraint::Length(6),
        Constraint::Min(0),
        Constraint::Length(3),
        Constraint::Length(20),
        Constraint::Length(1),
    ])
    .areas(row_area);

    let label = Paragraph::new(Span::styled(
        label.to_string(),
        Style::default().fg(accent).add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(label, label_area);

    let color = palette.threshold(percent);
    frame.render_widget(
        Paragraph::new(segmented_bar(percent, gauge_area.width, |value| {
            palette.threshold(value)
        })),
        gauge_area,
    );

    let value = Paragraph::new(Span::styled(
        format!(
            "{:.1} / {:.1} GiB  {:>3.0}%",
            used_kb as f64 / 1_048_576.0,
            total_kb as f64 / 1_048_576.0,
            percent
        ),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    ))
    .alignment(Alignment::Right);
    frame.render_widget(value, value_area);
}

fn draw_disabled_row(frame: &mut ratatui::Frame, area: Rect, text: &str) {
    if area.height == 0 {
        return;
    }

    let [_, line_area, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .areas(area);
    let [_, text_area] =
        Layout::horizontal([Constraint::Length(1), Constraint::Min(0)]).areas(line_area);
    frame.render_widget(Paragraph::new(dim_text(text)), text_area);
}

fn draw_chart(
    frame: &mut ratatui::Frame,
    area: Rect,
    title: &str,
    history: &RingBuffer,
    percent: f32,
    accent: Color,
    palette: &Palette,
) {
    let block = panel(title, accent);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    braille::render(
        frame,
        inner,
        history,
        10.0,
        1000,
        Style::default().fg(palette.threshold(percent)),
    );
}

fn percent(used_kb: u64, total_kb: u64) -> f32 {
    if total_kb == 0 {
        0.0
    } else {
        used_kb as f32 / total_kb as f32 * 100.0
    }
}
