use crate::collectors::Collector;
use crate::collectors::battery::BatteryCollector;
use crate::collectors::cpu::CpuCollector;
use crate::collectors::disk::DiskCollector;
use crate::collectors::gpu::{GpuProvider, detect_gpu};
use crate::collectors::memory::MemoryCollector;
use crate::collectors::network::NetworkCollector;
use crate::collectors::processes::{ProcessCollector, SortBy};
use crate::config::Config;
use crate::history::RingBuffer;
use crate::theme::{Palette, SLOT_NAMES};
use std::collections::HashMap;

/// Tabs shown in the tab bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Cpu,
    Memory,
    Disk,
    Network,
    Gpu,
    Battery,
    Processes,
    Theme,
}

/// Application state shared by the event loop and the UI.
pub struct App {
    pub tab: Tab,
    pub cpu: CpuCollector,
    pub memory: MemoryCollector,
    pub disk: DiskCollector,
    pub network: NetworkCollector,
    pub battery: BatteryCollector,
    pub processes: ProcessCollector,
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

    pub tick_rate_ms: u64,
    pub palette: Palette,
    pub config: Config,
    pub theme_row: usize,
    pub theme_error: Option<String>,
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
        let mut processes = ProcessCollector::new(config.processes.top_n, sort_by)?;
        processes.update()?;

        // Braille charts consume two samples per column.
        let capacity = (terminal_width as usize).max(1) * 2;
        let num_cores = cpu.latest.per_core_percent.len();

        Ok(Self {
            tab: Tab::Cpu,
            cpu,
            memory,
            disk,
            network,
            battery,
            processes,
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
            tick_rate_ms: config.display.refresh_ms,
            palette: Palette::from_config(config),
            config: config.clone(),
            theme_row: 0,
            theme_error: None,
        })
    }

    /// Refresh every collector and append new values to the history buffers.
    pub fn on_tick(&mut self) {
        let _ = self.cpu.update();
        let _ = self.memory.update();
        let _ = self.disk.update();
        let _ = self.network.update();
        let _ = self.battery.update();
        let _ = self.processes.update();

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
                .push(gpu.temperature_celsius().map_or(0.0, |value| value));
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
            Tab::Cpu => Tab::Memory,
            Tab::Memory => Tab::Disk,
            Tab::Disk => Tab::Network,
            Tab::Network => Tab::Gpu,
            Tab::Gpu => Tab::Battery,
            Tab::Battery => Tab::Processes,
            Tab::Processes => Tab::Theme,
            Tab::Theme => Tab::Cpu,
        };
    }

    /// Move to the previous tab, wrapping around at the start.
    pub fn prev_tab(&mut self) {
        self.tab = match self.tab {
            Tab::Cpu => Tab::Processes,
            Tab::Memory => Tab::Cpu,
            Tab::Disk => Tab::Memory,
            Tab::Network => Tab::Disk,
            Tab::Gpu => Tab::Network,
            Tab::Battery => Tab::Gpu,
            Tab::Processes => Tab::Battery,
            Tab::Theme => Tab::Processes,
        };
    }

    /// Move the Theme-tab selection, wrapping around.
    pub fn select_theme_row(&mut self, delta: i32) {
        self.theme_row =
            (self.theme_row as i32 + delta).rem_euclid(SLOT_NAMES.len() as i32) as usize;
    }

    /// Cycle the selected Theme-tab color and persist the palette.
    pub fn cycle_theme_color(&mut self, delta: i32) {
        let color = crate::theme::cycle(self.palette.slot(self.theme_row), delta);
        self.palette.set_slot(self.theme_row, color);
        self.palette.apply_to(&mut self.config);
        self.theme_error = self.config.save().err().map(|error| error.to_string());
    }

    /// Set the refresh interval, clamped to the supported range.
    pub fn set_tick_rate(&mut self, ms: u64) {
        self.tick_rate_ms = ms.clamp(100, 10_000);
    }
}
