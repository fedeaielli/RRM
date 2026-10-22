use super::Collector;

/// A snapshot of RAM and swap usage, in kilobytes.
pub struct MemorySample {
    pub total_kb: u64,
    #[allow(dead_code)]
    pub available_kb: u64,
    pub used_kb: u64,
    pub swap_total_kb: u64,
    pub swap_used_kb: u64,
}

/// Collects RAM and swap usage from `/proc/meminfo`.
pub struct MemoryCollector {
    pub latest: MemorySample,
}

impl MemoryCollector {
    /// Create a collector and populate it with an initial sample.
    pub fn new() -> anyhow::Result<Self> {
        let mut collector = Self {
            latest: MemorySample {
                total_kb: 0,
                available_kb: 0,
                used_kb: 0,
                swap_total_kb: 0,
                swap_used_kb: 0,
            },
        };
        collector.update()?;
        Ok(collector)
    }
}

impl Collector for MemoryCollector {
    fn update(&mut self) -> anyhow::Result<()> {
        let contents = std::fs::read_to_string("/proc/meminfo")?;

        let mut total_kb = None;
        let mut available_kb = None;
        let mut swap_total_kb = None;
        let mut swap_free_kb = None;

        for line in contents.lines() {
            let Some((key, rest)) = line.split_once(':') else {
                continue;
            };
            let value = parse_kb(rest);

            match key {
                "MemTotal" => total_kb = value,
                "MemAvailable" => available_kb = value,
                "SwapTotal" => swap_total_kb = value,
                "SwapFree" => swap_free_kb = value,
                _ => {}
            }
        }

        let total_kb =
            total_kb.ok_or_else(|| anyhow::anyhow!("MemTotal missing from /proc/meminfo"))?;
        let available_kb = available_kb
            .ok_or_else(|| anyhow::anyhow!("MemAvailable missing from /proc/meminfo"))?;
        let swap_total_kb =
            swap_total_kb.ok_or_else(|| anyhow::anyhow!("SwapTotal missing from /proc/meminfo"))?;
        let swap_free_kb =
            swap_free_kb.ok_or_else(|| anyhow::anyhow!("SwapFree missing from /proc/meminfo"))?;

        self.latest = MemorySample {
            total_kb,
            available_kb,
            used_kb: total_kb.saturating_sub(available_kb),
            swap_total_kb,
            swap_used_kb: swap_total_kb.saturating_sub(swap_free_kb),
        };

        Ok(())
    }
}

/// Parse the leading numeric value (kB) from a `/proc/meminfo` field.
fn parse_kb(value: &str) -> Option<u64> {
    value.split_whitespace().next()?.parse::<u64>().ok()
}
