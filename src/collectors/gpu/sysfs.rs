use super::GpuProvider;
use std::path::{Path, PathBuf};

/// Intel/AMD GPU backed by `/sys/class/drm` and hwmon.
pub struct SysFsGpu {
    card_path: PathBuf,
    hwmon_path: PathBuf,
    name: String,
    vendor: String,
}

impl SysFsGpu {
    /// Detect a supported Intel or AMD GPU.
    ///
    /// Returns `Ok(None)` when no compatible card is present.
    pub fn detect() -> anyhow::Result<Option<Self>> {
        let Ok(entries) = std::fs::read_dir("/sys/class/drm") else {
            return Ok(None);
        };

        for entry in entries.flatten() {
            let card_name = entry.file_name().to_string_lossy().into_owned();
            let Some(suffix) = card_name.strip_prefix("card") else {
                continue;
            };
            if suffix.is_empty() || !suffix.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }

            let card_path = entry.path();
            let Ok(raw_vendor) = std::fs::read_to_string(card_path.join("device/vendor")) else {
                continue;
            };

            let vendor = match raw_vendor.trim().to_ascii_lowercase().as_str() {
                "0x8086" => "Intel",
                "0x1002" => "AMD",
                _ => continue,
            };

            let hwmon_path =
                find_hwmon(&card_path).unwrap_or_else(|| card_path.join("device").join("hwmon"));

            return Ok(Some(Self {
                card_path,
                hwmon_path,
                name: format!("{vendor} GPU ({card_name})"),
                vendor: vendor.to_string(),
            }));
        }

        Ok(None)
    }
}

impl GpuProvider for SysFsGpu {
    fn name(&self) -> &str {
        &self.name
    }

    fn usage_percent(&self) -> anyhow::Result<f32> {
        // i915/xe expose no busy percentage; callers show N/A.
        read_f32(self.card_path.join("device/gpu_busy_percent"), "GPU usage")
    }

    fn vram_used_mb(&self) -> anyhow::Result<u64> {
        if self.vendor == "Intel" {
            return Ok(0);
        }
        read_u64(
            self.card_path.join("device/mem_info_vram_used"),
            "VRAM used",
        )
        .map(|bytes| bytes / 1_048_576)
    }

    fn vram_total_mb(&self) -> anyhow::Result<u64> {
        if self.vendor == "Intel" {
            return Ok(0);
        }
        read_u64(
            self.card_path.join("device/mem_info_vram_total"),
            "VRAM total",
        )
        .map(|bytes| bytes / 1_048_576)
    }

    fn temperature_celsius(&self) -> anyhow::Result<f32> {
        read_f32(self.hwmon_path.join("temp1_input"), "GPU temperature")
            .map(|millidegrees| millidegrees / 1000.0)
    }
}

/// Locate the hwmon device that belongs to the GPU driver.
fn find_hwmon(card_path: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(card_path.join("device").join("hwmon")).ok()?;

    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(raw_name) = std::fs::read_to_string(path.join("name")) else {
            continue;
        };
        if matches!(raw_name.trim(), "amdgpu" | "i915" | "xe") {
            return Some(path);
        }
    }

    None
}

fn read_f32(path: PathBuf, label: &str) -> anyhow::Result<f32> {
    let raw = std::fs::read_to_string(&path).map_err(|error| {
        anyhow::anyhow!("failed to read {label} from {}: {error}", path.display())
    })?;
    raw.trim()
        .parse::<f32>()
        .map_err(|error| anyhow::anyhow!("invalid {label} value: {error}"))
}

fn read_u64(path: PathBuf, label: &str) -> anyhow::Result<u64> {
    let raw = std::fs::read_to_string(&path).map_err(|error| {
        anyhow::anyhow!("failed to read {label} from {}: {error}", path.display())
    })?;
    raw.trim()
        .parse::<u64>()
        .map_err(|error| anyhow::anyhow!("invalid {label} value: {error}"))
}
