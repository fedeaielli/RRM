use super::Collector;

/// A snapshot of CPU metrics.
pub struct CpuSample {
    pub total_percent: f32,
    pub per_core_percent: Vec<f32>,
    pub frequencies_mhz: Vec<u64>,
    pub temperature_celsius: Option<f32>,
}

/// Collects CPU usage, frequency and temperature from `/proc` and `/sys`.
pub struct CpuCollector {
    pub latest: CpuSample,
    prev_idle: Vec<u64>,
    prev_total: Vec<u64>,
}

struct CpuTimes {
    total: u64,
    idle: u64,
}

impl CpuCollector {
    /// Create a collector seeded with the current `/proc/stat` counters.
    pub fn new() -> anyhow::Result<Self> {
        let cores = read_cpu_times()?;
        let prev_idle: Vec<u64> = cores.iter().map(|core| core.idle).collect();
        let prev_total: Vec<u64> = cores.iter().map(|core| core.total).collect();

        Ok(Self {
            latest: CpuSample {
                total_percent: 0.0,
                per_core_percent: vec![0.0; cores.len()],
                frequencies_mhz: vec![0; cores.len()],
                temperature_celsius: None,
            },
            prev_idle,
            prev_total,
        })
    }
}

impl Collector for CpuCollector {
    fn update(&mut self) -> anyhow::Result<()> {
        let cores = read_cpu_times()?;

        let mut per_core_percent = Vec::with_capacity(cores.len());
        let mut delta_total_sum: u64 = 0;
        let mut delta_idle_sum: u64 = 0;

        for (index, core) in cores.iter().enumerate() {
            let previous = self
                .latest
                .per_core_percent
                .get(index)
                .copied()
                .unwrap_or(0.0);
            match (self.prev_total.get(index), self.prev_idle.get(index)) {
                (Some(&prev_total), Some(&prev_idle)) => {
                    let delta_total = core.total.saturating_sub(prev_total);
                    let delta_idle = core.idle.saturating_sub(prev_idle);

                    delta_total_sum = delta_total_sum.saturating_add(delta_total);
                    delta_idle_sum = delta_idle_sum.saturating_add(delta_idle);

                    per_core_percent.push(if delta_total == 0 {
                        previous
                    } else {
                        usage_percent(delta_idle, delta_total)
                    });
                }
                _ => per_core_percent.push(previous),
            }
        }

        let total_percent = if delta_total_sum == 0 {
            self.latest.total_percent
        } else {
            usage_percent(delta_idle_sum, delta_total_sum)
        };
        let frequencies_mhz = read_frequencies(cores.len());
        let temperature_celsius = read_temperature();

        self.latest = CpuSample {
            total_percent,
            per_core_percent,
            frequencies_mhz,
            temperature_celsius,
        };
        self.prev_idle = cores.iter().map(|core| core.idle).collect();
        self.prev_total = cores.iter().map(|core| core.total).collect();

        Ok(())
    }
}

fn usage_percent(delta_idle: u64, delta_total: u64) -> f32 {
    if delta_total == 0 {
        return 0.0;
    }
    let usage = 1.0 - (delta_idle as f32 / delta_total as f32);
    (usage * 100.0).clamp(0.0, 100.0)
}

/// Read per-core counters from `/proc/stat`, skipping the aggregate `cpu` line.
fn read_cpu_times() -> anyhow::Result<Vec<CpuTimes>> {
    let contents = std::fs::read_to_string("/proc/stat")?;
    let mut cores = Vec::new();

    for line in contents.lines() {
        let mut fields = line.split_whitespace();
        let Some(label) = fields.next() else {
            continue;
        };
        if !is_per_core_label(label) {
            continue;
        }

        let mut values = Vec::with_capacity(8);
        for field in fields.take(8) {
            let value = field
                .parse::<u64>()
                .map_err(|_| anyhow::anyhow!("invalid value in /proc/stat: {field}"))?;
            values.push(value);
        }
        if values.len() < 4 {
            return Err(anyhow::anyhow!(
                "incomplete CPU counters in /proc/stat for {label}"
            ));
        }

        // user, nice, system, idle, iowait, irq, softirq, steal
        let idle = values[3].saturating_add(values.get(4).copied().unwrap_or(0));
        let total = values.iter().copied().fold(0u64, u64::saturating_add);
        cores.push(CpuTimes { total, idle });
    }

    if cores.is_empty() {
        return Err(anyhow::anyhow!(
            "no per-core CPU counters found in /proc/stat"
        ));
    }

    Ok(cores)
}

fn is_per_core_label(label: &str) -> bool {
    match label.strip_prefix("cpu") {
        Some(suffix) => !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()),
        None => false,
    }
}

/// Read `scaling_cur_freq` for each core, in MHz. Missing files yield 0.
fn read_frequencies(core_count: usize) -> Vec<u64> {
    (0..core_count)
        .map(|core| {
            let path = format!("/sys/devices/system/cpu/cpu{core}/cpufreq/scaling_cur_freq");
            match std::fs::read_to_string(path) {
                Ok(raw) => raw.trim().parse::<u64>().map(|khz| khz / 1000).unwrap_or(0),
                Err(_) => 0,
            }
        })
        .collect()
}

/// Read the first temperature exposed by a matching CPU hwmon device.
fn read_temperature() -> Option<f32> {
    let entries = std::fs::read_dir("/sys/class/hwmon").ok()?;

    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(raw_name) = std::fs::read_to_string(path.join("name")) else {
            continue;
        };
        let name = raw_name.trim();
        if !matches!(name, "coretemp" | "k10temp" | "zenpower") {
            continue;
        }

        let Ok(raw_temp) = std::fs::read_to_string(path.join("temp1_input")) else {
            continue;
        };
        let Ok(millidegrees) = raw_temp.trim().parse::<f32>() else {
            continue;
        };
        return Some(millidegrees / 1000.0);
    }

    None
}
