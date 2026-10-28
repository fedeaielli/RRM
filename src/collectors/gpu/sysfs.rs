use super::GpuProvider;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

/// Intel/AMD GPU backed by `/sys/class/drm` and hwmon.
pub struct SysFsGpu {
    card_path: PathBuf,
    temp_path: Option<PathBuf>,
    temp_proxy: bool,
    name: String,
    vendor: String,
    driver: Option<String>,
    rc6: Mutex<Option<(u64, Instant)>>,
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

            let temp_path = find_hwmon(&card_path)
                .map(|hwmon| hwmon.join("temp1_input"))
                .filter(|path| path.is_file());
            let (temp_path, temp_proxy) = match temp_path {
                Some(path) => (Some(path), false),
                None => match find_thermal_zone("gpu") {
                    Some(path) => (Some(path), false),
                    None => (find_thermal_zone("x86_pkg_temp"), true),
                },
            };

            let raw_device =
                std::fs::read_to_string(card_path.join("device/device")).unwrap_or_default();
            let model = read_pci_name(&raw_vendor, &raw_device);
            let name = model.map_or_else(
                || format!("{vendor} GPU ({card_name})"),
                |model| format!("{vendor} {model}"),
            );

            let driver = std::fs::read_link(card_path.join("device/driver"))
                .ok()
                .and_then(|path| {
                    path.file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                });

            return Ok(Some(Self {
                card_path,
                temp_path,
                temp_proxy,
                name,
                vendor: vendor.to_string(),
                driver,
                rc6: Mutex::new(None),
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
        if self.vendor != "Intel" {
            return read_f32(self.card_path.join("device/gpu_busy_percent"), "GPU usage");
        }

        // i915/xe expose no busy counter, but RC6 residency tells how long the
        // GPU stayed idle, so two samples give an approximate busy percentage.
        let current = read_u64(self.rc6_path()?, "RC6 residency")?;
        let now = Instant::now();
        let mut state = self
            .rc6
            .lock()
            .map_err(|_| anyhow::anyhow!("RC6 state poisoned"))?;
        let previous = state.replace((current, now));
        match previous {
            Some((prev_rc6, prev_time)) => {
                let elapsed_ms = now.duration_since(prev_time).as_millis() as u64;
                Ok(busy_from_rc6(prev_rc6, current, elapsed_ms))
            }
            None => Err(anyhow::anyhow!("GPU usage warming up")),
        }
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
        let Some(path) = &self.temp_path else {
            return Err(anyhow::anyhow!("no GPU temperature sensor"));
        };
        read_f32(path.clone(), "GPU temperature").map(|millidegrees| millidegrees / 1000.0)
    }

    fn temperature_is_proxy(&self) -> bool {
        self.temp_proxy
    }

    fn frequency_mhz(&self) -> anyhow::Result<(u64, u64)> {
        if self.vendor == "Intel" {
            let current = read_u64(self.card_path.join("gt_cur_freq_mhz"), "GPU clock")?;
            let max = read_u64(self.card_path.join("gt_max_freq_mhz"), "GPU max clock")?;
            return Ok((current, max));
        }
        let raw = std::fs::read_to_string(self.card_path.join("device/pp_dpm_sclk"))
            .map_err(|error| anyhow::anyhow!("failed to read GPU clocks: {error}"))?;
        parse_sclk(&raw).ok_or_else(|| anyhow::anyhow!("no GPU clock in pp_dpm_sclk"))
    }

    fn driver(&self) -> Option<&str> {
        self.driver.as_deref()
    }

    fn power_state(&self) -> Option<String> {
        std::fs::read_to_string(self.card_path.join("device/power/runtime_status"))
            .ok()
            .map(|raw| raw.trim().to_string())
    }
}

impl SysFsGpu {
    /// RC6 residency counter: `gt/gt0/` on newer kernels, `gt_rc6_*` on older ones.
    fn rc6_path(&self) -> anyhow::Result<PathBuf> {
        let nested = self.card_path.join("gt/gt0/rc6_residency_ms");
        if nested.is_file() {
            return Ok(nested);
        }
        let legacy = self.card_path.join("gt_rc6_residency_ms");
        if legacy.is_file() {
            return Ok(legacy);
        }
        Err(anyhow::anyhow!("RC6 residency not exposed"))
    }
}

/// Busy percentage from two RC6 residency samples over `elapsed_ms`.
fn busy_from_rc6(prev_rc6: u64, cur_rc6: u64, elapsed_ms: u64) -> f32 {
    if elapsed_ms == 0 {
        return 0.0;
    }
    let idle = cur_rc6.saturating_sub(prev_rc6).min(elapsed_ms);
    (100.0 * (1.0 - idle as f32 / elapsed_ms as f32)).clamp(0.0, 100.0)
}

/// Current (`*` line) and maximum clock from an AMD `pp_dpm_sclk` table.
fn parse_sclk(raw: &str) -> Option<(u64, u64)> {
    let mut current = None;
    let mut max = 0;
    for line in raw.lines() {
        let Some((_, value)) = line.split_once(':') else {
            continue;
        };
        let clock = value
            .split_whitespace()
            .next()?
            .trim_end_matches("Mhz")
            .trim_end_matches("MHz")
            .parse::<u64>()
            .ok()?;
        max = max.max(clock);
        if line.contains('*') {
            current = Some(clock);
        }
    }
    Some((current?, max))
}

/// Model name from `pci.ids` for a PCI vendor/device pair, when the file exists.
fn read_pci_name(vendor: &str, device: &str) -> Option<String> {
    let ids = ["/usr/share/hwdata/pci.ids", "/usr/share/misc/pci.ids"]
        .iter()
        .find_map(|path| std::fs::read_to_string(path).ok())?;
    pci_name(&ids, vendor, device)
}

/// Look up a PCI id pair in a `pci.ids` database.
fn pci_name(ids: &str, vendor: &str, device: &str) -> Option<String> {
    let vendor = vendor.trim().trim_start_matches("0x").to_ascii_lowercase();
    let device = device.trim().trim_start_matches("0x").to_ascii_lowercase();
    let mut in_vendor = false;

    for line in ids.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        if !line.starts_with('\t') {
            in_vendor = line
                .split_whitespace()
                .next()
                .is_some_and(|id| id.eq_ignore_ascii_case(&vendor));
            continue;
        }
        // Two tabs mark subsystem entries, which we do not need.
        if !in_vendor || line.starts_with("\t\t") {
            continue;
        }
        let mut parts = line.trim().splitn(2, char::is_whitespace);
        let id = parts.next()?;
        if id.eq_ignore_ascii_case(&device) {
            return Some(marketing_name(parts.next()?.trim()));
        }
    }
    None
}

/// Prefer the bracketed marketing name: `Navi 23 [Radeon RX 6600]`.
fn marketing_name(name: &str) -> String {
    if let Some(start) = name.find('[')
        && let Some(end) = name[start..].find(']')
    {
        return name[start + 1..start + end].to_string();
    }
    name.to_string()
}

/// Temperature file of the first thermal zone whose type contains `needle`.
fn find_thermal_zone(needle: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir("/sys/class/thermal").ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(raw_type) = std::fs::read_to_string(path.join("type")) else {
            continue;
        };
        if raw_type.trim().to_ascii_lowercase().contains(needle) {
            let temp = path.join("temp");
            if temp.is_file() {
                return Some(temp);
            }
        }
    }
    None
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

#[cfg(test)]
mod tests {
    use super::{busy_from_rc6, parse_sclk};

    #[test]
    fn busy_from_rc6_complements_idle_time() {
        assert_eq!(busy_from_rc6(1000, 1250, 1000), 75.0);
        assert_eq!(busy_from_rc6(0, 0, 0), 0.0);
        assert_eq!(busy_from_rc6(0, 5000, 1000), 0.0);
    }

    #[test]
    fn parse_sclk_reads_current_and_max() {
        let raw = "0: 500Mhz\n1: 1200Mhz *\n2: 2100Mhz\n";
        assert_eq!(parse_sclk(raw), Some((1200, 2100)));
        assert_eq!(parse_sclk("garbage"), None);
    }
}

#[cfg(test)]
mod pci_tests {
    use super::{marketing_name, pci_name};

    const IDS: &str = "# comment\n8086  Intel Corporation\n\t46a8  Alder Lake-UP3 GT2 [Iris Xe Graphics]\n\t46aa  Alder Lake-UP4 GT2 [Iris Xe Graphics]\n1002  Advanced Micro Devices\n\t73ff  Navi 23 [Radeon RX 6600]\n";

    #[test]
    fn pci_name_prefers_marketing_name() {
        assert_eq!(
            pci_name(IDS, "0x8086", "0x46A8").as_deref(),
            Some("Iris Xe Graphics")
        );
        assert_eq!(
            pci_name(IDS, "0x1002", "0x73ff").as_deref(),
            Some("Radeon RX 6600")
        );
        assert_eq!(pci_name(IDS, "0x8086", "0x9999"), None);
        assert_eq!(marketing_name("Plain Name"), "Plain Name");
    }
}
