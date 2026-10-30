mod app;
mod collectors;
mod config;
mod event;
mod history;
mod theme;
mod ui;

use app::{App, Tab};
use config::Config;
use event::{AppEvent, EventHandler};
use history::RingBuffer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Load config
    let config = Config::load()?;

    // 2. Setup terminal (crossterm: enable raw mode, enter alternate screen)
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(
        stdout,
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    )?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    // 3. Get terminal width for history buffer sizing
    let width = terminal.size()?.width as usize;

    // 4. Init app
    let mut app = App::new(&config, width as u16)?;

    // 5. Init event handler
    let mut events = EventHandler::new(app.tick_rate_ms);

    // 6. Main loop
    loop {
        terminal.draw(|f| ui::draw(f, &app))?;
        match events.next().await? {
            AppEvent::Tick => app.on_tick(),
            AppEvent::Key(key) => {
                use crossterm::event::{KeyCode, KeyModifiers};
                if app.theme_edit.is_some() {
                    match key.code {
                        KeyCode::Esc => app.cancel_theme_edit(),
                        KeyCode::Enter => app.commit_theme_edit(),
                        KeyCode::Backspace => app.pop_theme_char(),
                        KeyCode::Char(c) => app.push_theme_char(c),
                        _ => {}
                    }
                    continue;
                }
                match (key.code, key.modifiers) {
                    (KeyCode::Char('q'), _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => break,
                    (KeyCode::Tab, _) => app.next_tab(),
                    (KeyCode::BackTab, _) => app.prev_tab(),
                    (KeyCode::Esc, _) => app.escape(),
                    (KeyCode::Right, _) => app.arrow(1),
                    (KeyCode::Left, _) => app.arrow(-1),
                    (KeyCode::Enter, _) if app.tab == Tab::Processes => app.open_process_detail(),
                    (KeyCode::Enter, _) if app.tab == Tab::Theme => app.start_theme_edit(),
                    (KeyCode::Up, _) if app.tab == Tab::Theme => app.select_theme_row(-1),
                    (KeyCode::Down, _) if app.tab == Tab::Theme => app.select_theme_row(1),
                    (KeyCode::Up, _) if app.tab == Tab::Processes => app.process_move(-1),
                    (KeyCode::Down, _) if app.tab == Tab::Processes => app.process_move(1),
                    (KeyCode::PageUp, _) if app.tab == Tab::Processes => app.process_move(-10),
                    (KeyCode::Home, _) if app.tab == Tab::Processes => app.process_move(i64::MIN),
                    (KeyCode::End, _) if app.tab == Tab::Processes => app.process_move(i64::MAX),
                    (KeyCode::Char('1'), _) => app.tab = Tab::System,
                    (KeyCode::Char('2'), _) => app.tab = Tab::Cpu,
                    (KeyCode::Char('3'), _) => app.tab = Tab::Memory,
                    (KeyCode::Char('4'), _) => app.tab = Tab::Disk,
                    (KeyCode::Char('5'), _) => app.tab = Tab::Network,
                    (KeyCode::Char('6'), _) => app.tab = Tab::Gpu,
                    (KeyCode::Char('7'), _) => app.tab = Tab::Battery,
                    (KeyCode::Char('8'), _) => app.tab = Tab::Processes,
                    (KeyCode::Char('9'), _) => app.tab = Tab::Fans,
                    (KeyCode::PageDown, _) if app.tab == Tab::Processes => app.focus_processes(),
                    (KeyCode::PageDown, _) => app.open_theme(),
                    (KeyCode::Char('s'), _) => app.sort_processes(),
                    (KeyCode::Char(' '), _) if app.tab == Tab::Processes => {
                        app.toggle_process_pause()
                    }
                    (KeyCode::Char('+'), _) => {
                        app.set_tick_rate(app.tick_rate_ms.saturating_add(100));
                        events.set_tick_rate(app.tick_rate_ms);
                    }
                    (KeyCode::Char('-'), _) => {
                        app.set_tick_rate(app.tick_rate_ms.saturating_sub(100));
                        events.set_tick_rate(app.tick_rate_ms);
                    }
                    _ => {}
                }
            }
            AppEvent::Resize(w, h) => {
                let _height = h;
                app.cpu_history = RingBuffer::new(w as usize * 2);
                app.mem_history = RingBuffer::new(w as usize * 2);
                app.swap_history = RingBuffer::new(w as usize * 2);
                for history in &mut app.cpu_core_histories {
                    *history = RingBuffer::new(w as usize * 2);
                }
                app.battery_power_history = RingBuffer::new(w as usize * 2);
                app.gpu_usage_history = RingBuffer::new(w as usize * 2);
                app.gpu_temp_history = RingBuffer::new(w as usize * 2);
                app.net_rx_history.clear();
                app.net_tx_history.clear();
                // Rebuilt at the new width on the next tick.
                app.fan_histories = std::collections::HashMap::new();
            }
        }
    }

    // 7. Restore terminal
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}
