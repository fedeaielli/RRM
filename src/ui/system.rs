use super::{accent, panel};
use crate::app::App;
use crate::collectors::system::humanize_uptime;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

const ARCH: &str = "       /\\\n      /  \\\n     /    \\\n    /      \\\n   /   ,,   \\\n  /   |  |   \\\n /    |  |    \\\n/___''    ''___\\";
const GENERIC: &str =
    "   .--------.\n  /          \\\n |    rrm     |\n  \\          /\n   '--------'";

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let accent = accent(app);
    let block = panel("system", accent);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let [_, logo_area, _, info_area] = Layout::horizontal([
        Constraint::Length(4),
        Constraint::Length(16),
        Constraint::Length(8),
        Constraint::Min(0),
    ])
    .areas(inner);

    let logo = Paragraph::new(logo_for(&app.system.latest.distro_id))
        .style(Style::default().fg(app.palette.logo));
    // Two blank rows above the logo; the info column stays at the top.
    let logo_area = Rect {
        y: logo_area.y.saturating_add(2),
        height: logo_area.height.saturating_sub(2),
        ..logo_area
    };
    frame.render_widget(logo, logo_area);
    frame.render_widget(
        Paragraph::new(info_lines(app)),
        info_area.inner(Margin::new(0, 2)),
    );
}

fn logo_for(distro_id: &str) -> &'static str {
    if distro_id == "arch" { ARCH } else { GENERIC }
}

fn info_lines(app: &App) -> Vec<Line<'static>> {
    let sample = &app.system.latest;
    let key = Style::default().fg(app.palette.key);
    let value = Style::default().fg(app.palette.value);
    let row = |label: &str, text: String, value_style: Style| {
        Line::from(vec![
            Span::styled(format!("{label:<12}"), key),
            Span::styled(text, value_style),
        ])
    };

    vec![
        Line::from(Span::styled(
            format!("{}@{}", sample.user, sample.host),
            Style::default()
                .fg(app.palette.key)
                .add_modifier(Modifier::BOLD),
        )),
        Line::default(),
        row("distro", sample.distro.clone(), value),
        row("host", sample.host.clone(), value),
        row("kernel", sample.kernel.clone(), value),
        row("uptime", humanize_uptime(sample.uptime_secs), value),
        row("packages", packages_text(sample), value),
        Line::default(),
        row("shell", sample.shell.clone(), value),
        row("desktop", sample.desktop.clone(), value),
        row("terminal", sample.terminal.clone(), value),
        Line::default(),
        row("cpu", app.cpu.model.clone(), value),
        row(
            "gpu",
            app.gpu
                .as_ref()
                .map_or_else(|| "n/a".to_string(), |gpu| gpu.name().to_string()),
            value,
        ),
        row("memory", memory_text(app), value),
        row("disk", disk_text(app), value),
        row(
            "load",
            format!(
                "{:.2} {:.2} {:.2}",
                sample.load[0], sample.load[1], sample.load[2]
            ),
            value,
        ),
    ]
}

fn packages_text(sample: &crate::collectors::system::SystemSample) -> String {
    match sample.packages {
        Some(count) if !sample.package_manager.is_empty() => {
            format!("{count} ({})", sample.package_manager)
        }
        Some(count) => count.to_string(),
        None => "n/a".to_string(),
    }
}

fn memory_text(app: &App) -> String {
    let memory = &app.memory.latest;
    if memory.total_kb == 0 {
        return "n/a".to_string();
    }
    format!(
        "{:.1}/{:.1} GiB",
        memory.used_kb as f64 / 1_048_576.0,
        memory.total_kb as f64 / 1_048_576.0
    )
}

fn disk_text(app: &App) -> String {
    let Some(root) = app
        .disk
        .partitions
        .iter()
        .find(|partition| partition.mount == "/")
    else {
        return "n/a".to_string();
    };
    let used = root.total_bytes.saturating_sub(root.free_bytes);
    let percent = if root.total_bytes > 0 {
        used as f64 / root.total_bytes as f64 * 100.0
    } else {
        0.0
    };
    format!(
        "{:.1}/{:.1} GiB ({percent:.0}%)",
        used as f64 / 1_073_741_824.0,
        root.total_bytes as f64 / 1_073_741_824.0
    )
}

#[cfg(test)]
mod tests {
    use crate::app::App;
    use crate::config::Config;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;

    #[test]
    fn system_tab_lists_the_host_details() {
        let app = App::new(&Config::default(), 90).unwrap();
        let mut terminal = Terminal::new(TestBackend::new(90, 26)).unwrap();
        terminal
            .draw(|f| super::draw(f, Rect::new(0, 0, 90, 26), &app))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let text: String = (0..26)
            .map(|y| {
                (0..90)
                    .map(|x| buffer[(x, y)].symbol().to_string())
                    .collect()
            })
            .collect::<Vec<String>>()
            .join("\n");

        assert!(text.contains("distro"));
        assert!(text.contains("kernel"));
        assert!(text.contains("uptime"));
        assert!(text.contains("load"));
    }
}
