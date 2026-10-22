use super::Collector;
use std::collections::HashMap;
use std::time::Instant;

/// Column used to rank processes.
#[derive(Debug, Clone, Copy)]
pub enum SortBy {
    Cpu,
    Mem,
}

/// A single process row.
pub struct ProcessSample {
    pub pid: u32,
    pub name: String,
    pub cpu_percent: f32,
    pub mem_kb: u64,
}

/// Collects the top-N processes by CPU or memory usage.
pub struct ProcessCollector {
    pub top: Vec<ProcessSample>,
    pub total: usize,
    top_n: usize,
    sort_by: SortBy,
    prev_cpu: HashMap<u32, u64>,
    prev_total_cpu: u64,
    prev_time: Instant,
}

impl ProcessCollector {
    /// Create a collector seeded with the current per-process CPU counters.
    pub fn new(top_n: usize, sort_by: SortBy) -> anyhow::Result<Self> {
        let mut prev_cpu = HashMap::new();
        for pid in read_pids()? {
            let Ok(contents) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
                continue;
            };
            if let Some((pid, _, ticks)) = parse_process_stat(&contents) {
                prev_cpu.insert(pid, ticks);
            }
        }

        let (prev_total_cpu, _) = read_cpu_summary()?;

        Ok(Self {
            top: Vec::new(),
            total: 0,
            top_n,
            sort_by,
            prev_cpu,
            prev_total_cpu,
            prev_time: Instant::now(),
        })
    }

    /// Label for the active process ranking.
    pub fn sort_label(&self) -> &'static str {
        match self.sort_by {
            SortBy::Cpu => "cpu",
            SortBy::Mem => "mem",
        }
    }
}

impl Collector for ProcessCollector {
    fn update(&mut self) -> anyhow::Result<()> {
        let (total_cpu, num_cores) = read_cpu_summary()?;
        let delta_total = total_cpu.saturating_sub(self.prev_total_cpu);

        let mut top = Vec::new();
        let mut cpu_counters = HashMap::new();

        for pid in read_pids()? {
            // The process may have exited since /proc was listed.
            let Ok(contents) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
                continue;
            };
            let Some((pid, name, ticks)) = parse_process_stat(&contents) else {
                continue;
            };

            let prev_ticks = self.prev_cpu.get(&pid).copied().unwrap_or(ticks);
            let delta_ticks = ticks.saturating_sub(prev_ticks);
            cpu_counters.insert(pid, ticks);

            let cpu_percent = if delta_total == 0 || num_cores == 0 {
                0.0
            } else {
                (delta_ticks as f64 / delta_total as f64 * num_cores as f64 * 100.0) as f32
            };

            let mem_kb = read_vm_rss(pid).unwrap_or(0);

            top.push(ProcessSample {
                pid,
                name,
                cpu_percent,
                mem_kb,
            });
        }

        match self.sort_by {
            SortBy::Cpu => top.sort_by(|a, b| b.cpu_percent.total_cmp(&a.cpu_percent)),
            SortBy::Mem => top.sort_by_key(|a| std::cmp::Reverse(a.mem_kb)),
        }
        self.total = top.len();
        top.truncate(self.top_n);

        self.top = top;
        self.prev_cpu = cpu_counters;
        self.prev_total_cpu = total_cpu;
        self.prev_time = Instant::now();

        Ok(())
    }
}

/// List numeric process IDs from `/proc`.
fn read_pids() -> anyhow::Result<Vec<u32>> {
    let entries = std::fs::read_dir("/proc")?;
    let pids = entries
        .flatten()
        .filter_map(|entry| entry.file_name().to_str()?.parse::<u32>().ok())
        .collect();
    Ok(pids)
}

/// Read aggregate CPU ticks and the number of per-core lines from `/proc/stat`.
fn read_cpu_summary() -> anyhow::Result<(u64, usize)> {
    let contents = std::fs::read_to_string("/proc/stat")?;
    let mut total = None;
    let mut num_cores = 0;

    for line in contents.lines() {
        let mut fields = line.split_whitespace();
        let Some(label) = fields.next() else {
            continue;
        };

        if label == "cpu" {
            let mut sum = 0u64;
            let mut count = 0;
            for field in fields.take(8) {
                if let Ok(value) = field.parse::<u64>() {
                    sum = sum.saturating_add(value);
                    count += 1;
                }
            }
            if count > 0 {
                total = Some(sum);
            }
        } else if let Some(suffix) = label.strip_prefix("cpu")
            && !suffix.is_empty()
            && suffix.chars().all(|c| c.is_ascii_digit())
        {
            num_cores += 1;
        }
    }

    let total =
        total.ok_or_else(|| anyhow::anyhow!("aggregate CPU line missing from /proc/stat"))?;
    Ok((total, num_cores))
}

/// Parse `pid` and `(comm)` without assuming the command name has no spaces.
fn parse_process_stat(contents: &str) -> Option<(u32, String, u64)> {
    let open = contents.find('(')?;
    let close = contents.rfind(')')?;
    if close <= open {
        return None;
    }

    let pid = contents[..open].trim().parse::<u32>().ok()?;
    let name = contents[open + 1..close].to_string();

    // Tokens after `comm`: state is token 0, utime is token 11, stime is token 12.
    let tail: Vec<&str> = contents[close + 1..].split_whitespace().collect();
    if tail.len() < 13 {
        return None;
    }

    let utime = tail[11].parse::<u64>().ok()?;
    let stime = tail[12].parse::<u64>().ok()?;

    Some((pid, name, utime.saturating_add(stime)))
}

/// Read `VmRSS` (kB) from `/proc/[pid]/status`.
fn read_vm_rss(pid: u32) -> Option<u64> {
    let contents = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    for line in contents.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            return rest.split_whitespace().next()?.parse::<u64>().ok();
        }
    }
    None
}
