use crate::history::RingBuffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

/// Braille dot bit for each of the four dot rows, left and right half of a cell.
const LEFT_BITS: [u32; 4] = [0x01, 0x02, 0x04, 0x40];
const RIGHT_BITS: [u32; 4] = [0x08, 0x10, 0x20, 0x80];
const BRAILLE_BASE: u32 = 0x2800;

/// Draw the newest samples as a braille sparkline: two samples per terminal
/// column and four dot rows per cell, so bars are half a cell wide.
pub fn render(
    frame: &mut ratatui::Frame,
    area: Rect,
    history: &RingBuffer,
    scale: f32,
    min_max: u64,
    style: Style,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let points = area.width as usize * 2;
    let levels = area.height as usize * 4;
    let data = window(history, points, scale);
    let max = data.iter().copied().max().unwrap_or(1).max(min_max).max(1);

    let buf = frame.buffer_mut();
    for (col, pair) in data.chunks(2).enumerate() {
        let left = filled(pair.first().copied().unwrap_or(0), max, levels);
        let right = filled(pair.get(1).copied().unwrap_or(0), max, levels);
        let x = area.x.saturating_add(col as u16);
        for row in 0..area.height {
            let symbol = cell_char(left, right, row as usize, area.height as usize);
            buf[(x, area.y.saturating_add(row))]
                .set_char(symbol)
                .set_style(style);
        }
    }
}

/// Like `render`, but with a fixed vertical scale: values above `max` stay at
/// full height instead of stretching the scale.
pub fn render_fixed(
    frame: &mut ratatui::Frame,
    area: Rect,
    history: &RingBuffer,
    scale: f32,
    max: u64,
    style: Style,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let points = area.width as usize * 2;
    let data = window(history, points, scale);
    draw_bars(frame.buffer_mut(), area, &data, max.max(1), style, false);
}

/// Draw read above a centre baseline and write below it, sharing one scale.
#[allow(clippy::too_many_arguments)]
pub fn render_split(
    frame: &mut ratatui::Frame,
    area: Rect,
    up: &RingBuffer,
    down: &RingBuffer,
    scale: f32,
    min_max: u64,
    up_style: Style,
    down_style: Style,
    baseline_style: Style,
) {
    if area.width == 0 || area.height < 3 {
        return;
    }

    let points = area.width as usize * 2;
    let up_data = window(up, points, scale);
    let down_data = window(down, points, scale);
    let max = up_data
        .iter()
        .chain(down_data.iter())
        .copied()
        .max()
        .unwrap_or(1)
        .max(min_max)
        .max(1);

    let up_height = (area.height - 1) / 2;
    let baseline_y = area.y + up_height;
    let buf = frame.buffer_mut();
    for x in area.x..area.right() {
        buf[(x, baseline_y)].set_char('─').set_style(baseline_style);
    }

    draw_bars(
        buf,
        Rect {
            height: up_height,
            ..area
        },
        &up_data,
        max,
        up_style,
        false,
    );
    draw_bars(
        buf,
        Rect {
            y: baseline_y + 1,
            height: area.height - up_height - 1,
            ..area
        },
        &down_data,
        max,
        down_style,
        true,
    );
}

/// Bars grow from the bottom edge, or from the top edge when `flip` is set.
fn draw_bars(
    buf: &mut ratatui::buffer::Buffer,
    area: Rect,
    data: &[u64],
    max: u64,
    style: Style,
    flip: bool,
) {
    if area.height == 0 {
        return;
    }
    let levels = area.height as usize * 4;
    for (col, pair) in data.chunks(2).enumerate() {
        let left = filled(pair.first().copied().unwrap_or(0), max, levels);
        let right = filled(pair.get(1).copied().unwrap_or(0), max, levels);
        let x = area.x.saturating_add(col as u16);
        for row in 0..area.height {
            let symbol = cell_char_series(left, right, row as usize, area.height as usize, flip);
            buf[(x, area.y.saturating_add(row))]
                .set_char(symbol)
                .set_style(style);
        }
    }
}

/// Newest samples scaled into `points` values, oldest front-padded with zero.
fn window(history: &RingBuffer, points: usize, scale: f32) -> Vec<u64> {
    let values = history.as_slice();
    let tail = &values[values.len().saturating_sub(points)..];
    let mut data = vec![0u64; points.saturating_sub(tail.len())];
    data.extend(tail.iter().map(|value| (value * scale) as u64));
    data
}

/// Dots filled from the bottom for a value, never zero for a nonzero sample.
fn filled(value: u64, max: u64, levels: usize) -> usize {
    if value == 0 || max == 0 {
        return 0;
    }
    (((value * levels as u64) / max) as usize).clamp(1, levels)
}

/// One braille cell for a given character row, from filled dot counts.
fn cell_char(left: usize, right: usize, row: usize, height: usize) -> char {
    cell_char_series(left, right, row, height, false)
}

fn cell_char_series(left: usize, right: usize, row: usize, height: usize, flip: bool) -> char {
    let levels = height * 4;
    let mut bits = 0;
    for (dot_row, (&left_bit, &right_bit)) in LEFT_BITS.iter().zip(&RIGHT_BITS).enumerate() {
        let level = (height - 1 - row) * 4 + (3 - dot_row);
        let level = if flip { levels - 1 - level } else { level };
        if level < left {
            bits |= left_bit;
        }
        if level < right {
            bits |= right_bit;
        }
    }
    if bits == 0 {
        return ' ';
    }
    char::from_u32(BRAILLE_BASE + bits).unwrap_or(' ')
}

#[cfg(test)]
mod tests {
    use super::{
        RingBuffer, Style, cell_char, cell_char_series, filled, render, render_fixed, render_split,
    };
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn filled_clamps_to_one_dot_and_bottom() {
        assert_eq!(filled(0, 100, 20), 0);
        assert_eq!(filled(1, 1000, 20), 1);
        assert_eq!(filled(1000, 1000, 20), 20);
        assert_eq!(filled(5, 0, 20), 0);
    }

    #[test]
    fn cell_chars_map_dot_columns() {
        assert_eq!(cell_char(0, 0, 0, 1), ' ');
        assert_eq!(cell_char(4, 0, 0, 1), '\u{2847}');
        assert_eq!(cell_char(0, 4, 0, 1), '\u{28B8}');
        assert_eq!(cell_char(4, 4, 0, 1), '\u{28FF}');
        // Values grow from the bottom row upwards.
        assert_eq!(cell_char(0, 4, 1, 2), '\u{28B8}');
        assert_eq!(cell_char(0, 4, 0, 2), ' ');
    }

    #[test]
    fn flipped_bars_grow_from_the_top() {
        // One dot: bottom-anchored hits the lowest dot row, flipped the highest.
        assert_eq!(cell_char_series(1, 1, 0, 1, false), '\u{28C0}');
        assert_eq!(cell_char_series(1, 1, 0, 1, true), '\u{2809}');
        assert_eq!(cell_char_series(4, 4, 0, 1, true), '\u{28FF}');
    }

    #[test]
    fn renders_braille_symbols_into_buffer() {
        let mut history = RingBuffer::new(4);
        history.push(100.0);
        let mut terminal =
            Terminal::new(TestBackend::new(4, 1)).expect("test backend should initialize");
        terminal
            .draw(|frame| render(frame, frame.area(), &history, 10.0, 50, Style::default()))
            .expect("render should succeed");

        let cell = terminal
            .backend()
            .buffer()
            .cell((3, 0))
            .expect("cell should exist");
        assert!(
            ('\u{2800}'..='\u{28FF}').contains(&cell.symbol().chars().next().unwrap_or(' ')),
            "expected a braille symbol, got {:?}",
            cell.symbol()
        );
    }

    #[test]
    fn fixed_scale_caps_tall_values() {
        fn draw(values: &[f32]) -> String {
            let mut history = RingBuffer::new(8);
            for value in values {
                history.push(*value);
            }
            let mut terminal = Terminal::new(TestBackend::new(2, 2)).unwrap();
            terminal
                .draw(|f| {
                    render_fixed(f, f.area(), &history, 1.0, 30_000, Style::default());
                })
                .unwrap();
            let buffer = terminal.backend().buffer();
            (0..2)
                .flat_map(|y| (0..2).map(move |x| (x, y)))
                .map(|(x, y)| buffer[(x, y)].symbol().to_string())
                .collect()
        }

        // Anything above the cap renders exactly like the cap.
        assert_eq!(draw(&[10_000.0, 60_000.0]), draw(&[10_000.0, 30_000.0]));
        assert_ne!(draw(&[10_000.0, 15_000.0]), draw(&[10_000.0, 30_000.0]));
    }

    #[test]
    fn renders_up_and_down_into_buffer() {
        let mut up = RingBuffer::new(4);
        up.push(100.0);
        let mut down = RingBuffer::new(4);
        down.push(50.0);
        let mut terminal =
            Terminal::new(TestBackend::new(4, 5)).expect("test backend should initialize");
        terminal
            .draw(|frame| {
                render_split(
                    frame,
                    frame.area(),
                    &up,
                    &down,
                    10.0,
                    50,
                    Style::default(),
                    Style::default(),
                    Style::default(),
                )
            })
            .expect("render should succeed");

        let buffer = terminal.backend().buffer();
        let baseline: String = (0..4)
            .map(|x| buffer[(x, 2)].symbol().to_string())
            .collect();
        assert_eq!(baseline, "────");
        // Down bars start at the baseline and grow down, so a half-height
        // write leaves the row furthest from the baseline empty.
        assert_eq!(buffer[(3, 4)].symbol(), " ");
        for y in [0, 1, 3] {
            let symbol = buffer[(3, y)].symbol().chars().next().unwrap_or(' ');
            assert!(
                ('\u{2800}'..='\u{28FF}').contains(&symbol),
                "expected a braille symbol at row {y}, got {symbol:?}"
            );
        }
    }
}
