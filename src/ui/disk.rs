use super::{CYAN, DIM, MGNTA, accent, dim_text, disk_usage_color, panel, render_empty};
use crate::app::App;
use crate::history::RingBuffer;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{LineGauge, Paragraph, Sparkline};
use std::collections::HashMap;

pub fn draw(frame: &mut ratatui::Frame, area: Rect, app: &App) {
    let accent = accent(app);
    let [io_area, partition_area] =
        Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)]).areas(area);

    draw_io(frame, io_area, app, accent);
    draw_partitions(frame, partition_area, app, accent);
}

fn draw_io(frame: &mut ratatui::Frame, area: Rect, app: &App, accent: ratatui::style::Color) {
    if app.disk.io.is_empty() {
        render_empty(frame, area, "io", "no disk i/o data", accent);
        return;
    }

    let block = panel("io", accent);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }

    let mut constraints: Vec<Constraint> =
        app.disk.io.iter().map(|_| Constraint::Length(3)).collect();
    constraints.push(Constraint::Min(0));
    let sections = Layout::vertical(constraints).split(inner);

    for (sample, section) in app.disk.io.iter().zip(sections.iter()) {
        draw_io_section(frame, *section, sample, app, accent);
    }
}

fn draw_io_section(
    frame: &mut ratatui::Frame,
    area: Rect,
    sample: &crate::collectors::disk::DiskSample,
    app: &App,
    accent: ratatui::style::Color,
) {
    if area.height == 0 {
        return;
    }

    let [name_area, read_area, write_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(area);

    let active = sample.read_bytes_per_sec > 0.0 || sample.write_bytes_per_sec > 0.0;
    let name_style = if active {
        Style::default().fg(accent).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(DIM)
    };
    let read_style = Style::default().fg(if active { CYAN } else { DIM });
    let write_style = Style::default().fg(if active { MGNTA } else { DIM });

    let name = Line::from(vec![
        Span::styled(format!("{}   ", sample.device), name_style),
        Span::styled(
            format!("↓ {}", format_rate(sample.read_bytes_per_sec)),
            read_style,
        ),
        dim_text("   "),
        Span::styled(
            format!("↑ {}", format_rate(sample.write_bytes_per_sec)),
            write_style,
        ),
    ]);
    frame.render_widget(Paragraph::new(name), name_area);

    let read_history = history_values(&app.disk_read_history, &sample.device);
    let read_sparkline = Sparkline::default()
        .data(&read_history)
        .style(Style::default().fg(CYAN));
    frame.render_widget(read_sparkline, read_area);

    let write_history = history_values(&app.disk_write_history, &sample.device);
    let write_sparkline = Sparkline::default()
        .data(&write_history)
        .style(Style::default().fg(MGNTA));
    frame.render_widget(write_sparkline, write_area);
}

fn draw_partitions(
    frame: &mut ratatui::Frame,
    area: Rect,
    app: &App,
    accent: ratatui::style::Color,
) {
    let block = panel("partitions", accent);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }

    let mut constraints: Vec<Constraint> = app
        .disk
        .partitions
        .iter()
        .map(|_| Constraint::Length(2))
        .collect();
    constraints.push(Constraint::Min(0));
    let sections = Layout::vertical(constraints).split(inner);

    for (partition, section) in app.disk.partitions.iter().zip(sections.iter()) {
        if section.height == 0 {
            continue;
        }

        let [mount_area, gauge_area, size_area] = Layout::horizontal([
            Constraint::Length(20),
            Constraint::Min(0),
            Constraint::Length(14),
        ])
        .areas(*section);
        draw_partition_row(frame, mount_area, gauge_area, size_area, partition);
    }
}

fn draw_partition_row(
    frame: &mut ratatui::Frame,
    mount_area: Rect,
    gauge_area: Rect,
    size_area: Rect,
    partition: &crate::collectors::disk::PartitionSample,
) {
    let total_gib = gib(partition.total_bytes);
    let used_gib = gib(partition.total_bytes.saturating_sub(partition.free_bytes));
    let used_percent = if total_gib > 0.0 {
        (used_gib / total_gib * 100.0) as f32
    } else {
        0.0
    };

    let mount = Paragraph::new(dim_text(truncate_mount(&partition.mount)));
    frame.render_widget(mount, mount_area);

    let gauge = LineGauge::default()
        .filled_style(Style::default().fg(disk_usage_color(used_percent)))
        .unfilled_style(Style::default().fg(DIM))
        .line_set(symbols::line::THICK)
        .ratio((f64::from(used_percent) / 100.0).clamp(0.0, 1.0));
    frame.render_widget(gauge, gauge_area);

    let size = Paragraph::new(dim_text(size_label(used_gib, total_gib, size_area.width)))
        .alignment(Alignment::Right);
    frame.render_widget(size, size_area);
}

/// Render `used/total GiB`, dropping the space when the column is too narrow.
fn size_label(used_gib: f64, total_gib: f64, width: u16) -> String {
    let label = format!("{used_gib:.1}/{total_gib:.1} GiB");
    if label.chars().count() as u16 <= width {
        return label;
    }

    format!("{used_gib:.1}/{total_gib:.1}GiB")
}

fn history_values(histories: &HashMap<String, RingBuffer>, device: &str) -> Vec<u64> {
    histories
        .get(device)
        .map(|buffer| {
            buffer
                .as_slice()
                .iter()
                .map(|value| *value as u64)
                .collect()
        })
        .unwrap_or_default()
}

fn gib(bytes: u64) -> f64 {
    bytes as f64 / 1_073_741_824.0
}

fn truncate_mount(mount: &str) -> &str {
    mount
        .char_indices()
        .nth_back(19)
        .map_or(mount, |(index, _)| &mount[index..])
}

fn format_rate(bytes_per_sec: f64) -> String {
    if bytes_per_sec < 1_024.0 {
        format!("{bytes_per_sec:.0} B/s")
    } else if bytes_per_sec < 1_048_576.0 {
        format!("{:.1} KB/s", bytes_per_sec / 1_024.0)
    } else {
        format!("{:.2} MB/s", bytes_per_sec / 1_048_576.0)
    }
}
