use super::Collector;

/// A snapshot of a single battery.
pub struct BatterySample {
    pub name: String,
    pub capacity_percent: u8,
    pub status: String,
    #[allow(dead_code)]
    pub energy_now_uwh: Option<u64>,
    pub energy_full_uwh: Option<u64>,
    pub energy_full_design_uwh: Option<u64>,
}

/// Collects battery information from `/sys/class/power_supply`.
pub struct BatteryCollector {
    pub batteries: Vec<BatterySample>,
}

impl BatteryCollector {
    /// Create a collector and populate it with an initial sample.
    pub fn new() -> anyhow::Result<Self> {
        let mut collector = Self {
            batteries: Vec::new(),
        };
        collector.update()?;
        Ok(collector)
    }
}

impl Collector for BatteryCollector {
    fn update(&mut self) -> anyhow::Result<()> {
        let entries = std::fs::read_dir("/sys/class/power_supply")?;
        let mut batteries = Vec::new();

        for entry in entries.flatten() {
            let path = entry.path();

            let Ok(raw_type) = std::fs::read_to_string(path.join("type")) else {
                continue;
            };
            if raw_type.trim() != "Battery" {
                continue;
            }

            let Ok(raw_capacity) = std::fs::read_to_string(path.join("capacity")) else {
                continue;
            };
            let Ok(capacity_percent) = raw_capacity.trim().parse::<u8>() else {
                continue;
            };

            let status = std::fs::read_to_string(path.join("status"))
                .map(|raw| normalize_status(&raw))
                .unwrap_or_else(|_| "Unknown".to_string());

            // Some laptops expose charge_* (µAh) instead of energy_* (µWh).
            // Keep all three from one family; only unitless ratios are displayed.
            let use_energy = path.join("energy_full").exists();
            let (now_attr, full_attr, design_attr) = if use_energy {
                ("energy_now", "energy_full", "energy_full_design")
            } else {
                ("charge_now", "charge_full", "charge_full_design")
            };

            batteries.push(BatterySample {
                name: entry.file_name().to_string_lossy().into_owned(),
                capacity_percent,
                status,
                energy_now_uwh: read_optional_u64(&path, now_attr),
                energy_full_uwh: read_optional_u64(&path, full_attr),
                energy_full_design_uwh: read_optional_u64(&path, design_attr),
            });
        }

        batteries.sort_by(|a, b| a.name.cmp(&b.name));
        self.batteries = batteries;

        Ok(())
    }
}

fn normalize_status(status: &str) -> String {
    match status.trim() {
        "Charging" | "Discharging" | "Full" | "Not charging" | "Unknown" => {
            status.trim().to_string()
        }
        _ => "Unknown".to_string(),
    }
}

/// Read an optional numeric sysfs attribute.
fn read_optional_u64(path: &std::path::Path, attribute: &str) -> Option<u64> {
    let raw = std::fs::read_to_string(path.join(attribute)).ok()?;
    raw.trim().parse::<u64>().ok()
}
