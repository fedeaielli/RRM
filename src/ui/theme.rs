use super::{DIM, accent, dim_text, panel};
use crate::app::App;
use crate::theme::{SLOT_NAMES, TRANSPARENT, to_hex};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

const SECTIONS: [(&str, &[usize]); 4] = [
    ("background", &[7]),
    ("interface", &[0, 1, 2, 3]),
    ("charts", &[4, 5, 6]),
    ("system", &[8, 9, 10]),
];

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let accent = accent(app);
    let block = panel("theme", accent);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    // Sections grow with their slot count, then a spacer and the footer at the bottom.
    let footer_height = if app.theme_error.is_some() { 2 } else { 1 };
    let mut constraints: Vec<Constraint> = section_heights(inner.height, footer_height)
        .iter()
        .map(|height| Constraint::Length(*height))
        .collect();
    constraints.push(Constraint::Length(1));
    constraints.push(Constraint::Length(footer_height));
    let slices = Layout::vertical(constraints).split(inner);

    for (index, (title, slots)) in SECTIONS.iter().enumerate() {
        draw_section(frame, slices[index], title, slots, app, accent);
    }

    let hint = if app.theme_edit.is_some() {
        "#RRGGBB   Enter save   Esc cancel"
    } else if app.theme_row.is_some() {
        "↑/↓ select   ←/→ change color   Enter hex"
    } else {
        "↑/↓ select   ←/→ change tab"
    };
    let mut footer = vec![Line::from(dim_text(hint))];
    if let Some(error) = &app.theme_error {
        footer.push(Line::from(Span::styled(
            format!("save failed: {error}"),
            Style::default().fg(app.palette.crit),
        )));
    }
    frame.render_widget(Paragraph::new(footer), slices[slices.len() - 1]);
}

fn draw_section(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    slots: &[usize],
    app: &App,
    accent: ratatui::style::Color,
) {
    let block = panel(title, accent);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    frame.render_widget(
        Paragraph::new(section_lines(slots, app, accent, inner)),
        inner,
    );
}

// Spare rows go to the sections in proportion to their slot count.
fn section_heights(height: u16, footer: u16) -> [u16; 4] {
    let available = height.saturating_sub(footer + 1);
    let mut heights = SECTIONS.map(|(_, slots)| slots.len() as u16 + 2);
    let spare = u32::from(available).saturating_sub(u32::from(heights.iter().sum::<u16>()));
    let total_slots: u32 = SECTIONS.iter().map(|(_, slots)| slots.len() as u32).sum();
    let mut given = 0;
    for (height, (_, slots)) in heights.iter_mut().zip(SECTIONS) {
        let share = (spare * slots.len() as u32 / total_slots) as u16;
        *height += share;
        given += share;
    }
    let mut rest = spare - u32::from(given);
    let mut index = 0;
    while rest > 0 {
        heights[index] += 1;
        rest -= 1;
        index = (index + 1) % heights.len();
    }
    heights
}

fn section_lines(
    slots: &[usize],
    app: &App,
    accent: ratatui::style::Color,
    area: Rect,
) -> Vec<Line<'static>> {
    let count = slots.len() as u16;
    if count == 0 {
        return Vec::new();
    }
    if count == 1 {
        let lead = area.height.saturating_sub(1) / 2;
        let mut lines: Vec<Line<'static>> = (0..lead).map(|_| Line::default()).collect();
        lines.push(slot_line(slots[0], app, accent, area.width));
        return lines;
    }
    let gaps = count - 1;
    let blanks = area.height.saturating_sub(count);
    let (base, extra) = match blanks.checked_div(gaps) {
        Some(base) => (base, blanks % gaps),
        None => (0, 0),
    };

    let mut lines = Vec::new();
    for (position, index) in slots.iter().enumerate() {
        if position > 0 {
            let count = base + u16::from((position as u16 - 1) < extra);
            lines.extend((0..count).map(|_| Line::default()));
        }
        lines.push(slot_line(*index, app, accent, area.width));
    }
    lines
}

fn slot_line(index: usize, app: &App, accent: ratatui::style::Color, width: u16) -> Line<'static> {
    let name = SLOT_NAMES.get(index).copied().unwrap_or("?");
    let color = app.palette.slot(index);
    let selected = app.theme_row == Some(index);
    let label_style = if selected {
        Style::default().fg(accent).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(DIM)
    };
    let (fill, fill_style) = if color == TRANSPARENT {
        ("░░░░░░".to_string(), Style::default().fg(DIM))
    } else {
        ("██████".to_string(), Style::default().fg(color))
    };
    let outline = Style::default().fg(DIM);
    let value = match (&app.theme_edit, selected) {
        (Some(buffer), true) => format!("{buffer}▏"),
        _ => to_hex(color),
    };
    let label = format!("{} {name:<12}", if selected { ">" } else { " " });
    let used = 14 + 8 + value.chars().count() as u16;
    let padding = " ".repeat(width.saturating_sub(used) as usize);

    Line::from(vec![
        Span::styled(label, label_style),
        Span::styled("│", outline),
        Span::styled(fill, fill_style),
        Span::styled("│", outline),
        Span::raw(padding),
        dim_text(value),
    ])
}

#[cfg(test)]
mod tests {
    use crate::app::App;
    use crate::config::Config;
    use crate::theme::TRANSPARENT;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;

    fn render(app: &mut App) -> String {
        let mut terminal = Terminal::new(TestBackend::new(70, 24)).unwrap();
        terminal
            .draw(|f| super::draw(f, Rect::new(0, 0, 70, 24), app))
            .unwrap();
        let buffer = terminal.backend().buffer();
        (0..24)
            .map(|y| {
                (0..70)
                    .map(|x| buffer[(x, y)].symbol().to_string())
                    .collect()
            })
            .collect::<Vec<String>>()
            .join("\n")
    }

    #[test]
    fn sections_fill_the_available_height() {
        let heights = super::section_heights(30, 1);
        assert_eq!(heights.iter().sum::<u16>(), 28);
        assert!(heights[0] >= 3 && heights[1] >= 6 && heights[2] >= 5 && heights[3] >= 5);
        assert!(heights[1] > heights[0] && heights[2] > heights[0]);
    }

    #[test]
    fn sections_render_with_outlined_swatches() {
        let mut app = App::new(&Config::default(), 70).unwrap();
        app.palette.background = TRANSPARENT;
        let text = render(&mut app);

        assert!(text.contains("background"));
        assert!(text.contains("interface"));
        assert!(text.contains("charts"));
        assert!(text.contains("system"));
        assert!(text.contains("│██████│"));
        assert!(text.contains("transparent"));
    }

    #[test]
    fn selected_row_shows_the_hex_editor() {
        let mut app = App::new(&Config::default(), 70).unwrap();
        app.theme_row = Some(0);
        app.start_theme_edit();
        let text = render(&mut app);
        assert!(text.contains("#00BFFF▏"));
        assert!(text.contains("Enter save"));

        app.theme_edit = Some("#12".to_string());
        app.commit_theme_edit();
        assert!(app.theme_edit.is_some());
        assert!(
            app.theme_error
                .as_deref()
                .is_some_and(|error| error.contains("invalid color"))
        );
        assert_eq!(app.palette.accent, crate::theme::Palette::default().accent);
    }
}
