use super::{DIM, accent, dim_text, panel};
use crate::app::App;
use crate::theme::{SLOT_NAMES, to_hex};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let accent = accent(app);
    let block = panel("theme", accent);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let mut lines = Vec::new();
    for (index, name) in SLOT_NAMES.iter().enumerate() {
        let color = app.palette.slot(index);
        let selected = index == app.theme_row;
        let label_style = if selected {
            Style::default().fg(accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(DIM)
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("{} {name:<8}", if selected { ">" } else { " " }),
                label_style,
            ),
            Span::styled("██████", Style::default().fg(color)),
            dim_text(format!("  {}", to_hex(color))),
        ]));
    }
    lines.push(Line::default());
    lines.push(Line::from(dim_text("↑/↓ select   ←/→ change color")));
    if let Some(error) = &app.theme_error {
        lines.push(Line::from(Span::styled(
            format!("save failed: {error}"),
            Style::default().fg(app.palette.crit),
        )));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}
