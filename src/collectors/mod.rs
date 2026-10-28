pub mod battery;
pub mod cpu;
pub mod disk;
pub mod fans;
pub mod gpu;
pub mod memory;
pub mod network;
pub mod processes;
pub mod system;

/// A source of live system metrics.
pub trait Collector: Send + Sync {
    fn update(&mut self) -> anyhow::Result<()>;
}
