pub mod nvml;
pub mod sysfs;

/// A source of live GPU metrics.
pub trait GpuProvider: Send + Sync {
    fn name(&self) -> &str;
    fn usage_percent(&self) -> anyhow::Result<f32>;
    fn vram_used_mb(&self) -> anyhow::Result<u64>;
    fn vram_total_mb(&self) -> anyhow::Result<u64>;
    fn temperature_celsius(&self) -> anyhow::Result<f32>;

    /// Current and maximum clock in MHz, when the driver exposes them.
    fn frequency_mhz(&self) -> anyhow::Result<(u64, u64)> {
        Err(anyhow::anyhow!("frequency not available"))
    }

    /// Kernel driver name, e.g. `i915`, `xe`, `amdgpu`.
    fn driver(&self) -> Option<&str> {
        None
    }

    /// Runtime power state, e.g. `active` or `suspended`.
    fn power_state(&self) -> Option<String> {
        None
    }

    /// True when the temperature comes from a surrogate sensor (CPU package).
    fn temperature_is_proxy(&self) -> bool {
        false
    }
}

/// Detect a compatible GPU, preferring the sysfs provider.
pub fn detect_gpu() -> Option<Box<dyn GpuProvider>> {
    match sysfs::SysFsGpu::detect() {
        Ok(Some(gpu)) => Some(Box::new(gpu)),
        _ => None,
    }
}
