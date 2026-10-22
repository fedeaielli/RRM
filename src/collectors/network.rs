use super::Collector;
use std::collections::HashMap;
use std::time::Instant;

/// Per-interface network throughput.
pub struct NetSample {
    pub interface: String,
    pub rx_bytes_per_sec: f64,
    pub tx_bytes_per_sec: f64,
}

/// Collects per-interface RX/TX rates from `/proc/net/dev`.
pub struct NetworkCollector {
    pub latest: Vec<NetSample>,
    prev_rx: HashMap<String, u64>,
    prev_tx: HashMap<String, u64>,
    prev_time: Instant,
}

struct NetStat {
    interface: String,
    rx_bytes: u64,
    tx_bytes: u64,
}

impl NetworkCollector {
    /// Create a collector seeded with the current `/proc/net/dev` counters.
    pub fn new() -> anyhow::Result<Self> {
        let stats = parse_net_dev()?;

        let mut prev_rx = HashMap::with_capacity(stats.len());
        let mut prev_tx = HashMap::with_capacity(stats.len());
        for stat in stats {
            prev_rx.insert(stat.interface.clone(), stat.rx_bytes);
            prev_tx.insert(stat.interface, stat.tx_bytes);
        }

        Ok(Self {
            latest: Vec::new(),
            prev_rx,
            prev_tx,
            prev_time: Instant::now(),
        })
    }
}

impl Collector for NetworkCollector {
    fn update(&mut self) -> anyhow::Result<()> {
        let stats = parse_net_dev()?;
        let elapsed_secs = self.prev_time.elapsed().as_secs_f64();

        let mut latest = Vec::with_capacity(stats.len());
        let mut rx_counters = HashMap::with_capacity(stats.len());
        let mut tx_counters = HashMap::with_capacity(stats.len());

        for stat in stats {
            let prev_rx = self
                .prev_rx
                .get(&stat.interface)
                .copied()
                .unwrap_or(stat.rx_bytes);
            let prev_tx = self
                .prev_tx
                .get(&stat.interface)
                .copied()
                .unwrap_or(stat.tx_bytes);

            let rx_delta = stat.rx_bytes.saturating_sub(prev_rx);
            let tx_delta = stat.tx_bytes.saturating_sub(prev_tx);

            latest.push(NetSample {
                interface: stat.interface.clone(),
                rx_bytes_per_sec: rate_bytes_per_sec(rx_delta, elapsed_secs),
                tx_bytes_per_sec: rate_bytes_per_sec(tx_delta, elapsed_secs),
            });

            rx_counters.insert(stat.interface.clone(), stat.rx_bytes);
            tx_counters.insert(stat.interface, stat.tx_bytes);
        }

        latest.sort_by(|a, b| a.interface.cmp(&b.interface));

        self.latest = latest;
        self.prev_rx = rx_counters;
        self.prev_tx = tx_counters;
        self.prev_time = Instant::now();

        Ok(())
    }
}

fn rate_bytes_per_sec(delta_bytes: u64, elapsed_secs: f64) -> f64 {
    if elapsed_secs <= 0.0 {
        return 0.0;
    }
    delta_bytes as f64 / elapsed_secs
}

/// Parse `/proc/net/dev`, skipping the header lines and the loopback interface.
fn parse_net_dev() -> anyhow::Result<Vec<NetStat>> {
    let contents = std::fs::read_to_string("/proc/net/dev")?;
    let mut stats = Vec::new();

    for line in contents.lines() {
        // Header lines contain no ':', so this also skips them.
        let Some((name, rest)) = line.split_once(':') else {
            continue;
        };
        let interface = name.trim();
        if interface.is_empty() || interface == "lo" {
            continue;
        }

        let fields: Vec<&str> = rest.split_whitespace().collect();
        if fields.len() < 10 {
            continue;
        }

        let Ok(rx_bytes) = fields[0].parse::<u64>() else {
            continue;
        };
        let Ok(tx_bytes) = fields[9].parse::<u64>() else {
            continue;
        };

        stats.push(NetStat {
            interface: interface.to_string(),
            rx_bytes,
            tx_bytes,
        });
    }

    Ok(stats)
}
