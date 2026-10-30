use crate::collectors::Collector;
use crate::collectors::battery::BatteryCollector;
use crate::collectors::cpu::CpuCollector;
use crate::collectors::disk::DiskCollector;
use crate::collectors::fans::FansCollector;
use crate::collectors::gpu::{GpuProvider, detect_gpu};
use crate::collectors::memory::MemoryCollector;
use crate::collectors::network::NetworkCollector;
use crate::collectors::processes::{
    DetailExtras, ProcessCollector, ProcessSample, SortBy, detail_extras,
};
use crate::collectors::system::SystemCollector;
use crate::config::Config;
use crate::history::RingBuffer;
use crate::theme::Palette;
use ratatui::style::Color;
use std::collections::HashMap;
use std::time::Instant;

/// Tabs shown in the tab bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    System,
    Cpu,
    Memory,
    Disk,
    Network,
    Gpu,
    Battery,
    Processes,
    Fans,
    Theme,
}

/// Per-process overlay: latest sample plus chart history for the open pid.
pub struct ProcessDetail {
    pub pid: u32,
    pub name: String,
    pub sample: ProcessSample,
    pub extras: DetailExtras,
    pub gpu_percent: f32,
    pub cpu_history: RingBuffer,
    pub mem_history: RingBuffer,
    pub gpu_history: RingBuffer,
    pub read_history: RingBuffer,
    pub write_history: RingBuffer,
    prev_gpu: Option<(Instant, u64)>,
}

/// Application state shared by the event loop and the UI.
pub struct App {
    pub tab: Tab,
    pub system: SystemCollector,
    pub cpu: CpuCollector,
    pub memory: MemoryCollector,
    pub disk: DiskCollector,
    pub network: NetworkCollector,
    pub battery: BatteryCollector,
    pub processes: ProcessCollector,
    pub fans: FansCollector,
    pub gpu: Option<Box<dyn GpuProvider>>,

    pub cpu_history: RingBuffer,
    pub cpu_core_histories: Vec<RingBuffer>,
    pub mem_history: RingBuffer,
    pub swap_history: RingBuffer,
    pub battery_power_history: RingBuffer,
    pub gpu_usage: Option<f32>,
    pub gpu_usage_history: RingBuffer,
    pub gpu_temp_history: RingBuffer,
    pub net_rx_history: HashMap<String, RingBuffer>,
    pub net_tx_history: HashMap<String, RingBuffer>,
    pub disk_read_history: HashMap<String, RingBuffer>,
    pub disk_write_history: HashMap<String, RingBuffer>,
    pub fan_histories: HashMap<String, RingBuffer>,

    pub tick_rate_ms: u64,
    pub palette: Palette,
    pub config: Config,
    pub theme_row: Option<usize>,
    /// Selected process row while the Processes table has focus.
    pub process_selected: Option<usize>,
    /// First visible column of the Processes table.
    pub process_column: usize,
    /// Freeze the process list so rows stop moving while inspecting them.
    pub process_paused: bool,
    /// Detail overlay for the selected process.
    pub process_detail: Option<ProcessDetail>,
    pub theme_error: Option<String>,
    /// In-progress hex entry for the selected Theme slot.
    pub theme_edit: Option<String>,
}

impl App {
    /// Initialize collectors, history buffers and GPU detection.
    pub fn new(config: &Config, terminal_width: u16) -> anyhow::Result<Self> {
        let mut cpu = CpuCollector::new()?;
        cpu.update()?;

        let mut memory = MemoryCollector::new()?;
        memory.update()?;

        let mut disk = DiskCollector::new()?;
        disk.update()?;

        let mut network = NetworkCollector::new()?;
        network.update()?;

        let mut battery = BatteryCollector::new()?;
        battery.update()?;

        let sort_by = match config.processes.sort_by.as_str() {
            "mem" => SortBy::Mem,
            _ => SortBy::Cpu,
        };
        let mut processes = ProcessCollector::new(sort_by)?;
        processes.update()?;

        // Braille charts consume two samples per column.
        let capacity = (terminal_width as usize).max(1) * 2;
        let num_cores = cpu.latest.per_core_percent.len();

        Ok(Self {
            tab: Tab::System,
            system: SystemCollector::new()?,
            cpu,
            memory,
            disk,
            network,
            battery,
            processes,
            fans: FansCollector::new()?,
            gpu: detect_gpu(),
            cpu_history: RingBuffer::new(capacity),
            cpu_core_histories: (0..num_cores).map(|_| RingBuffer::new(capacity)).collect(),
            mem_history: RingBuffer::new(capacity),
            swap_history: RingBuffer::new(capacity),
            battery_power_history: RingBuffer::new(capacity),
            gpu_usage: None,
            gpu_usage_history: RingBuffer::new(capacity),
            gpu_temp_history: RingBuffer::new(capacity),
            net_rx_history: HashMap::new(),
            net_tx_history: HashMap::new(),
            disk_read_history: HashMap::new(),
            disk_write_history: HashMap::new(),
            fan_histories: HashMap::new(),
            tick_rate_ms: config.display.refresh_ms,
            palette: Palette::from_config(config),
            config: config.clone(),
            theme_row: None,
            process_selected: None,
            process_column: 0,
            process_paused: false,
            process_detail: None,
            theme_error: None,
            theme_edit: None,
        })
    }

    /// Refresh every collector and append new values to the history buffers.
    pub fn on_tick(&mut self) {
        let _ = self.system.update();
        let _ = self.cpu.update();
        let _ = self.memory.update();
        let _ = self.disk.update();
        let _ = self.network.update();
        let _ = self.battery.update();
        if !self.process_paused {
            let _ = self.processes.update();
        }
        if let Some(selected) = self.process_selected
            && selected >= self.processes.top.len()
        {
            self.process_selected = self.processes.top.len().checked_sub(1);
        }
        self.refresh_process_detail();
        let _ = self.fans.update();

        self.cpu_history.push(self.cpu.latest.total_percent);

        let mem_percent = if self.memory.latest.total_kb > 0 {
            self.memory.latest.used_kb as f32 / self.memory.latest.total_kb as f32 * 100.0
        } else {
            0.0
        };
        self.mem_history.push(mem_percent);
        let swap_percent = if self.memory.latest.swap_total_kb > 0 {
            self.memory.latest.swap_used_kb as f32 / self.memory.latest.swap_total_kb as f32 * 100.0
        } else {
            0.0
        };
        self.swap_history.push(swap_percent);

        let capacity = self.cpu_history.capacity();
        for sample in &self.network.latest {
            self.net_rx_history
                .entry(sample.interface.clone())
                .or_insert_with(|| RingBuffer::new(capacity))
                .push(sample.rx_bytes_per_sec as f32);
            self.net_tx_history
                .entry(sample.interface.clone())
                .or_insert_with(|| RingBuffer::new(capacity))
                .push(sample.tx_bytes_per_sec as f32);
        }

        for sample in &self.disk.io {
            self.disk_read_history
                .entry(sample.device.clone())
                .or_insert_with(|| RingBuffer::new(capacity))
                .push((sample.read_bytes_per_sec / 1024.0) as f32);
            self.disk_write_history
                .entry(sample.device.clone())
                .or_insert_with(|| RingBuffer::new(capacity))
                .push((sample.write_bytes_per_sec / 1024.0) as f32);
        }

        let live_fans: Vec<&str> = self.fans.fans.iter().map(|f| f.name.as_str()).collect();
        self.fan_histories
            .retain(|name, _| live_fans.contains(&name.as_str()));
        for fan in &self.fans.fans {
            self.fan_histories
                .entry(fan.name.clone())
                .or_insert_with(|| RingBuffer::new(capacity))
                .push(fan.rpm as f32);
        }

        for (index, history) in self.cpu_core_histories.iter_mut().enumerate() {
            if let Some(&percent) = self.cpu.latest.per_core_percent.get(index) {
                history.push(percent);
            }
        }
        if let Some(gpu) = self.gpu.as_ref() {
            // Sampled once per tick: the Intel RC6 estimate is stateful, so the
            // UI must not call it again while drawing.
            self.gpu_usage = gpu.usage_percent().ok();
            self.gpu_usage_history.push(self.gpu_usage.unwrap_or(0.0));
            self.gpu_temp_history
                .push(gpu.temperature_celsius().unwrap_or(0.0));
        }
        if let Some(battery) = self.battery.batteries.first() {
            // Stored in mW so sub-watt values survive the braille chart's integer scale.
            self.battery_power_history
                .push(battery.power_now_uw.map_or(0.0, |uw| uw as f32 / 1000.0));
        }
    }

    /// Advance to the next tab, wrapping around at the end.
    pub fn next_tab(&mut self) {
        self.tab = match self.tab {
            Tab::System => Tab::Cpu,
            Tab::Cpu => Tab::Memory,
            Tab::Memory => Tab::Disk,
            Tab::Disk => Tab::Network,
            Tab::Network => Tab::Gpu,
            Tab::Gpu => Tab::Battery,
            Tab::Battery => Tab::Processes,
            Tab::Processes => Tab::Fans,
            Tab::Fans => Tab::Theme,
            Tab::Theme => Tab::System,
        };
        self.clear_theme_selection();
    }

    /// Move to the previous tab, wrapping around at the start.
    pub fn prev_tab(&mut self) {
        self.tab = match self.tab {
            Tab::System => Tab::Theme,
            Tab::Cpu => Tab::System,
            Tab::Memory => Tab::Cpu,
            Tab::Disk => Tab::Memory,
            Tab::Network => Tab::Disk,
            Tab::Gpu => Tab::Network,
            Tab::Battery => Tab::Gpu,
            Tab::Fans => Tab::Processes,
            Tab::Processes => Tab::Battery,
            Tab::Theme => Tab::Fans,
        };
        self.clear_theme_selection();
    }

    /// Entering the Theme tab always starts with no field selected.
    fn clear_theme_selection(&mut self) {
        if self.tab == Tab::Theme {
            self.theme_row = None;
        }
        if self.tab != Tab::Processes {
            self.process_selected = None;
            self.process_column = 0;
            self.process_paused = false;
        }
    }

    /// Give the Processes table focus, always starting on the first row.
    pub fn focus_processes(&mut self) {
        self.tab = Tab::Processes;
        self.process_selected = Some(0);
    }

    /// Move the Processes selection, clamped to the list.
    pub fn process_move(&mut self, delta: i64) {
        let last = self.processes.top.len().saturating_sub(1);
        let current = self.process_selected.unwrap_or(0) as i64;
        let next = current.saturating_add(delta).clamp(0, last as i64) as usize;
        if !self.processes.top.is_empty() {
            self.process_selected = Some(next);
        }
    }

    /// Scroll the Processes table horizontally, one column per press.
    pub fn process_columns(&mut self, delta: i32) {
        let next = self.process_column as i64 + i64::from(delta);
        self.process_column = next.clamp(0, crate::ui::processes::MAX_COLUMN as i64) as usize;
    }

    /// Open the detail overlay for the selected row.
    pub fn open_process_detail(&mut self) {
        let Some(row) = self
            .process_selected
            .and_then(|index| self.processes.top.get(index))
            .cloned()
        else {
            return;
        };
        let Some(extras) = detail_extras(row.pid) else {
            return;
        };
        let capacity = self.cpu_history.capacity();
        self.process_detail = Some(ProcessDetail {
            pid: row.pid,
            name: row.name.clone(),
            sample: row,
            extras,
            gpu_percent: 0.0,
            cpu_history: RingBuffer::new(capacity),
            mem_history: RingBuffer::new(capacity),
            gpu_history: RingBuffer::new(capacity),
            read_history: RingBuffer::new(capacity),
            write_history: RingBuffer::new(capacity),
            prev_gpu: None,
        });
    }

    pub fn close_process_detail(&mut self) {
        self.process_detail = None;
    }

    /// Re-sample the open process; closes the overlay if the pid is gone.
    fn refresh_process_detail(&mut self) {
        let Some(pid) = self.process_detail.as_ref().map(|detail| detail.pid) else {
            return;
        };
        let row = self
            .processes
            .top
            .iter()
            .find(|process| process.pid == pid)
            .cloned();
        let (Some(row), Some(extras)) = (row, detail_extras(pid)) else {
            self.process_detail = None;
            return;
        };
        let Some(detail) = self.process_detail.as_mut() else {
            return;
        };

        let now = Instant::now();
        let gpu_percent = match detail.prev_gpu {
            Some((prev_time, prev_ns)) => {
                let elapsed = now.duration_since(prev_time).as_secs_f64();
                if elapsed > 0.0 {
                    (extras.gpu_ns.saturating_sub(prev_ns) as f64 / (elapsed * 1e9) * 100.0)
                        .clamp(0.0, 100.0) as f32
                } else {
                    0.0
                }
            }
            None => 0.0,
        };
        detail.prev_gpu = Some((now, extras.gpu_ns));
        detail.gpu_percent = gpu_percent;
        detail.cpu_history.push(row.cpu_percent);
        detail.mem_history.push(row.mem_kb as f32);
        detail.gpu_history.push(gpu_percent);
        detail.read_history.push(row.read_bytes_per_sec as f32);
        detail.write_history.push(row.write_bytes_per_sec as f32);
        detail.sample = row;
        detail.extras = extras;
    }

    /// Freeze or resume the process list.
    pub fn toggle_process_pause(&mut self) {
        self.process_paused = !self.process_paused;
    }

    /// Change the process ranking and keep the selection on a valid row.
    pub fn sort_processes(&mut self) {
        self.processes.cycle_sort();
        if self.process_paused {
            self.processes.sort_rows();
        } else {
            self.processes.update().ok();
        }
        self.process_move(0);
    }

    /// Open the Theme tab with nothing selected.
    pub fn open_theme(&mut self) {
        self.tab = Tab::Theme;
        self.theme_row = None;
    }

    /// Move the Theme-tab selection, wrapping around; the first press selects
    /// the first field (down) or the last one (up).
    /// Esc: leave hex editing, otherwise hand input back to the tab bar.
    pub fn escape(&mut self) {
        if self.process_detail.is_some() {
            self.close_process_detail();
        } else if self.theme_edit.is_some() {
            self.cancel_theme_edit();
        } else if self.tab == Tab::Theme {
            self.theme_row = None;
        } else if self.tab == Tab::Processes && self.process_selected.is_some() {
            self.process_selected = None;
            self.process_column = 0;
        }
    }

    pub fn select_theme_row(&mut self, delta: i32) {
        let order = crate::theme::SLOT_ORDER;
        let count = order.len() as i32;
        let position = self
            .theme_row
            .and_then(|row| order.iter().position(|&slot| slot == row));
        let next = match position {
            Some(position) => (position as i32 + delta).rem_euclid(count) as usize,
            None if delta < 0 => (count - 1) as usize,
            None => 0,
        };
        self.theme_row = Some(order[next]);
    }

    /// Cycle the selected Theme-tab color and persist the palette.
    /// Left/right in the Theme tab: cycle the selected color, otherwise
    /// behave like the other tabs and switch tab.
    pub fn arrow(&mut self, delta: i32) {
        if self.tab == Tab::Theme && self.theme_row.is_some() {
            self.cycle_theme_color(delta);
        } else if self.tab == Tab::Processes && self.process_selected.is_some() {
            self.process_columns(delta);
        } else if delta > 0 {
            self.next_tab();
        } else {
            self.prev_tab();
        }
    }

    pub fn cycle_theme_color(&mut self, delta: i32) {
        let Some(row) = self.theme_row else {
            return;
        };
        let current = self.palette.slot(row);
        let color = if row == crate::theme::BACKGROUND_INDEX {
            crate::theme::cycle_background(current, delta)
        } else {
            crate::theme::cycle(current, delta)
        };
        self.palette.set_slot(row, color);
        self.palette.apply_to(&mut self.config);
        self.theme_error = self.config.save().err().map(|error| error.to_string());
    }

    /// Start editing the selected Theme slot, seeded with its current hex.
    pub fn start_theme_edit(&mut self) {
        let Some(row) = self.theme_row else {
            return;
        };
        let color = self.palette.slot(row);
        self.theme_edit = Some(match color {
            Color::Rgb(..) => crate::theme::to_hex(color),
            _ => String::new(),
        });
        self.theme_error = None;
    }

    pub fn cancel_theme_edit(&mut self) {
        self.theme_edit = None;
    }

    pub fn push_theme_char(&mut self, c: char) {
        if let Some(buffer) = &mut self.theme_edit
            && buffer.len() < 7
            && (c.is_ascii_hexdigit() || c == '#')
        {
            buffer.push(c);
        }
    }

    pub fn pop_theme_char(&mut self) {
        if let Some(buffer) = &mut self.theme_edit {
            buffer.pop();
        }
    }

    /// Parse the hex buffer, apply it and persist; keeps editing on bad input.
    pub fn commit_theme_edit(&mut self) {
        let Some(buffer) = self.theme_edit.clone() else {
            return;
        };
        let Some(row) = self.theme_row else {
            return;
        };
        match crate::theme::parse_hex(&buffer) {
            Some(color) => {
                self.palette.set_slot(row, color);
                self.palette.apply_to(&mut self.config);
                self.theme_error = self.config.save().err().map(|error| error.to_string());
                self.theme_edit = None;
            }
            None => self.theme_error = Some(format!("invalid color: {buffer}")),
        }
    }

    /// Set the refresh interval, clamped to the supported range.
    pub fn set_tick_rate(&mut self, ms: u64) {
        self.tick_rate_ms = ms.clamp(100, 10_000);
    }
}

#[cfg(test)]
mod tab_tests {
    use super::{App, Tab};
    use crate::config::Config;

    #[test]
    fn enter_opens_and_escape_closes_the_process_detail() {
        let mut app = App::new(&Config::default(), 80).unwrap();
        app.tab = Tab::Processes;
        app.process_selected = Some(0);
        app.open_process_detail();
        assert!(app.process_detail.is_some());
        app.escape();
        assert!(app.process_detail.is_none());
    }

    #[test]
    fn pause_toggles_and_resets_when_leaving_the_tab() {
        let mut app = App::new(&Config::default(), 80).unwrap();
        app.tab = Tab::Processes;
        app.toggle_process_pause();
        assert!(app.process_paused);
        app.next_tab();
        assert!(!app.process_paused);
    }

    #[test]
    fn escape_cancels_editing_first() {
        let mut app = App::new(&Config::default(), 80).unwrap();
        app.open_theme();
        app.theme_row = Some(0);
        app.start_theme_edit();
        app.escape();
        assert!(app.theme_edit.is_none());
        assert_eq!(app.theme_row, Some(0));
    }

    #[test]
    fn escape_hands_input_back_to_the_tab_bar() {
        let mut app = App::new(&Config::default(), 80).unwrap();
        app.open_theme();
        app.theme_row = Some(2);
        app.escape();
        assert_eq!(app.theme_row, None);
        assert_eq!(app.tab, Tab::Theme);
    }

    #[test]
    fn arrows_include_the_theme_tab() {
        let mut app = App::new(&Config::default(), 80).unwrap();
        app.tab = Tab::Processes;
        app.next_tab();
        assert_eq!(app.tab, Tab::Fans);
        app.next_tab();
        assert_eq!(app.tab, Tab::Theme);
        app.next_tab();
        assert_eq!(app.tab, Tab::System);
        app.prev_tab();
        assert_eq!(app.tab, Tab::Theme);
        app.prev_tab();
        assert_eq!(app.tab, Tab::Fans);
        app.prev_tab();
        assert_eq!(app.tab, Tab::Processes);
    }

    #[test]
    fn arrows_leave_theme_when_nothing_is_selected() {
        let mut app = App::new(&Config::default(), 80).unwrap();
        app.open_theme();
        app.arrow(1);
        assert_eq!(app.tab, Tab::System);
        app.open_theme();
        app.arrow(-1);
        assert_eq!(app.tab, Tab::Fans);
    }

    #[test]
    fn page_down_opens_theme_without_selection() {
        let mut app = App::new(&Config::default(), 80).unwrap();
        app.tab = Tab::Cpu;
        app.open_theme();
        assert_eq!(app.tab, Tab::Theme);
        assert_eq!(app.theme_row, None);
    }

    #[test]
    fn first_arrow_press_selects_the_first_visible_field() {
        let mut app = App::new(&Config::default(), 80).unwrap();
        app.select_theme_row(1);
        assert_eq!(app.theme_row, Some(7), "background is the first row shown");
        app.select_theme_row(-1);
        assert_eq!(app.theme_row, Some(10), "value is the last row shown");

        app.theme_row = None;
        app.select_theme_row(-1);
        assert_eq!(app.theme_row, Some(10));
    }

    #[test]
    fn entering_theme_clears_the_selection() {
        let mut app = App::new(&Config::default(), 80).unwrap();
        app.theme_row = Some(2);
        app.tab = Tab::Fans;
        app.next_tab();
        assert_eq!(app.tab, Tab::Theme);
        assert_eq!(app.theme_row, None);

        app.theme_row = Some(3);
        app.tab = Tab::System;
        app.prev_tab();
        assert_eq!(app.tab, Tab::Theme);
        assert_eq!(app.theme_row, None);
    }
}
