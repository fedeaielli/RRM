use super::{DIM, accent, dim_text, panel};
use crate::app::App;
use crate::theme::{SLOT_NAMES, TRANSPARENT, to_hex};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

const SECTIONS: [(&str, &[usize]); 3] = [
    ("background", &[7]),
    ("interface", &[0, 1, 2, 3]),
    ("charts", &[4, 5, 6]),
];

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let accent = accent(app);
    let block = panel("theme", accent);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    // Border + one row per slot, separated by a blank line.
    let mut constraints = Vec::new();
    for (index, (_, slots)) in SECTIONS.iter().enumerate() {
        if index > 0 {
            constraints.push(Constraint::Length(1));
        }
        constraints.push(Constraint::Length(slots.len() as u16 + 2));
    }
    constraints.push(Constraint::Min(0));
    let slices = Layout::vertical(constraints).split(inner);

    let mut slice = 0;
    for (index, (title, slots)) in SECTIONS.iter().enumerate() {
        if index > 0 {
            slice += 1;
        }
        draw_section(frame, slices[slice], title, slots, app, accent);
        slice += 1;
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

    let lines: Vec<Line> = slots
        .iter()
        .map(|index| slot_line(*index, app, accent))
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}

fn slot_line(index: usize, app: &App, accent: ratatui::style::Color) -> Line<'static> {
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

    Line::from(vec![
        Span::styled(
            format!("{} {name:<10}", if selected { ">" } else { " " }),
            label_style,
        ),
        Span::styled("│", outline),
        Span::styled(fill, fill_style),
        Span::styled("│", outline),
        dim_text(format!("  {value}")),
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
    fn sections_render_with_outlined_swatches() {
        let mut app = App::new(&Config::default(), 70).unwrap();
        app.palette.background = TRANSPARENT;
        let text = render(&mut app);

        assert!(text.contains("background"));
        assert!(text.contains("interface"));
        assert!(text.contains("charts"));
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
