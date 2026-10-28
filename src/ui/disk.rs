use super::{
    DIM, RX, TX, accent, braille, dim_text, disk_usage_color, format_bytes, format_rate, panel,
    render_empty, segmented_bar, threshold_color,
};
use crate::app::App;
use crate::history::RingBuffer;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

pub fn draw(frame: &mut ratatui::Frame, area: Rect, app: &App) {
    let accent = accent(app);
    let partition_height = (app.disk.partitions.len() as u16 + 2)
        .max(3)
        .min((area.height / 2).max(3));
    let zram_height = if app.disk.zram.is_empty() {
        0
    } else {
        app.disk.zram.len() as u16 + 2
    };
    let [partition_area, io_area, zram_area] = Layout::vertical([
        Constraint::Length(partition_height),
        Constraint::Min(0),
        Constraint::Length(zram_height),
    ])
    .areas(area);

    draw_partitions(frame, partition_area, app, accent);
    draw_io(frame, io_area, app, accent);
    draw_zram(frame, zram_area, app, accent);
}

fn draw_partitions(frame: &mut ratatui::Frame, area: Rect, app: &App, accent: Color) {
    if app.disk.partitions.is_empty() {
        render_empty(frame, area, "partitions", "no partitions found", accent);
        return;
    }

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
        .map(|_| Constraint::Length(1))
        .collect();
    constraints.push(Constraint::Min(0));
    let sections = Layout::vertical(constraints).split(inner);

    for (partition, section) in app.disk.partitions.iter().zip(sections.iter()) {
        if section.height == 0 {
            continue;
        }
        let used_bytes = partition.total_bytes.saturating_sub(partition.free_bytes);
        draw_usage_row(
            frame,
            *section,
            &partition.mount,
            used_bytes,
            partition.total_bytes,
            disk_usage_color,
        );
    }
}

fn draw_zram(frame: &mut ratatui::Frame, area: Rect, app: &App, accent: Color) {
    if area.height == 0 || app.disk.zram.is_empty() {
        return;
    }

    let block = panel("zram", accent);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }

    let sections = Layout::vertical(
        app.disk
            .zram
            .iter()
            .map(|_| Constraint::Length(1))
            .chain(std::iter::once(Constraint::Min(0))),
    )
    .split(inner);

    for (sample, section) in app.disk.zram.iter().zip(sections.iter()) {
        if section.height == 0 {
            continue;
        }
        draw_usage_row(
            frame,
            *section,
            &sample.name,
            sample.used_bytes,
            sample.total_bytes,
            threshold_color,
        );
    }
}

fn draw_usage_row(
    frame: &mut ratatui::Frame,
    area: Rect,
    label: &str,
    used_bytes: u64,
    total_bytes: u64,
    color: fn(f32) -> Color,
) {
    let used_percent = if total_bytes > 0 {
        (used_bytes as f64 / total_bytes as f64 * 100.0) as f32
    } else {
        0.0
    };

    let [label_area, gauge_area, size_area] = Layout::horizontal([
        Constraint::Length(18),
        Constraint::Min(0),
        Constraint::Length(22),
    ])
    .areas(area);

    frame.render_widget(Paragraph::new(dim_text(truncate_mount(label))), label_area);
    frame.render_widget(
        Paragraph::new(segmented_bar(used_percent, gauge_area.width, color)),
        gauge_area,
    );

    let size = Paragraph::new(dim_text(format!(
        "{}/{}",
        format_bytes(used_bytes),
        format_bytes(total_bytes)
    )))
    .alignment(Alignment::Right);
    frame.render_widget(size, size_area);
}

fn draw_io(frame: &mut ratatui::Frame, area: Rect, app: &App, accent: Color) {
    let devices: Vec<&crate::collectors::disk::DiskSample> = app
        .disk
        .io
        .iter()
        .filter(|sample| !sample.device.starts_with("zram"))
        .collect();

    if devices.is_empty() {
        render_empty(frame, area, "disk i/o", "no disk i/o data", accent);
        return;
    }

    let count = devices.len() as u32;
    let constraints: Vec<Constraint> = devices
        .iter()
        .map(|_| Constraint::Ratio(1, count))
        .collect();
    let sections = Layout::horizontal(constraints).split(area);

    for (sample, section) in devices.iter().zip(sections.iter()) {
        draw_io_device(frame, *section, sample, app, accent);
    }
}

fn draw_io_device(
    frame: &mut ratatui::Frame,
    area: Rect,
    sample: &crate::collectors::disk::DiskSample,
    app: &App,
    accent: Color,
) {
    let block = panel(sample.device.clone(), accent);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }

    let [rate_area, chart_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(inner);

    let read = sample.read_bytes_per_sec;
    let write = sample.write_bytes_per_sec;
    let rates = Line::from(vec![
        Span::styled(
            format!("↓ read  {}", format_rate(read)),
            Style::default().fg(if read > 0.0 { RX } else { DIM }),
        ),
        dim_text("   "),
        Span::styled(
            format!("↑ write {}", format_rate(write)),
            Style::default().fg(if write > 0.0 { TX } else { DIM }),
        ),
    ]);
    frame.render_widget(Paragraph::new(rates), rate_area);

    draw_rw(
        frame,
        chart_area,
        app.disk_read_history.get(&sample.device),
        app.disk_write_history.get(&sample.device),
    );
}

fn draw_rw(
    frame: &mut ratatui::Frame,
    area: Rect,
    read: Option<&RingBuffer>,
    write: Option<&RingBuffer>,
) {
    let (Some(read), Some(write)) = (read, write) else {
        return;
    };
    braille::render_split(
        frame,
        area,
        read,
        write,
        1.0 / 1024.0,
        1,
        Style::default().fg(RX),
        Style::default().fg(TX),
        Style::default().fg(DIM),
    );
}

fn truncate_mount(mount: &str) -> &str {
    mount
        .char_indices()
        .nth_back(17)
        .map_or(mount, |(index, _)| &mount[index..])
}
