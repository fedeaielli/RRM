pub mod nvml;
pub mod sysfs;

/// A source of live GPU metrics.
pub trait GpuProvider: Send + Sync {
    fn name(&self) -> &str;
    fn usage_percent(&self) -> anyhow::Result<f32>;
    fn vram_used_mb(&self) -> anyhow::Result<u64>;
    fn vram_total_mb(&self) -> anyhow::Result<u64>;
    fn temperature_celsius(&self) -> anyhow::Result<f32>;
}

/// Detect a compatible GPU, preferring the sysfs provider.
pub fn detect_gpu() -> Option<Box<dyn GpuProvider>> {
    match sysfs::SysFsGpu::detect() {
        Ok(Some(gpu)) => Some(Box::new(gpu)),
        _ => None,
    }
}
