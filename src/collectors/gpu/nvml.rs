use super::GpuProvider;

/// Placeholder NVIDIA provider; NVML support is reserved for a future feature.
#[allow(dead_code)]
pub struct NvmlGpu;

impl GpuProvider for NvmlGpu {
    fn name(&self) -> &str {
        "NVIDIA (NVML)"
    }

    fn usage_percent(&self) -> anyhow::Result<f32> {
        Err(anyhow::anyhow!(
            "NVML not compiled — enable feature 'nvidia'"
        ))
    }

    fn vram_used_mb(&self) -> anyhow::Result<u64> {
        Err(anyhow::anyhow!(
            "NVML not compiled — enable feature 'nvidia'"
        ))
    }

    fn vram_total_mb(&self) -> anyhow::Result<u64> {
        Err(anyhow::anyhow!(
            "NVML not compiled — enable feature 'nvidia'"
        ))
    }

    fn temperature_celsius(&self) -> anyhow::Result<f32> {
        Err(anyhow::anyhow!(
            "NVML not compiled — enable feature 'nvidia'"
        ))
    }
}
