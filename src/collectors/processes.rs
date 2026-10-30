use super::Collector;
use std::collections::HashMap;
use std::time::Instant;

/// Column used to rank processes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortBy {
    Cpu,
    Mem,
    Read,
    Write,
}

/// A single process row.
#[derive(Clone)]
pub struct ProcessSample {
    pub pid: u32,
    pub name: String,
    pub cpu_percent: f32,
    pub mem_kb: u64,
    pub read_bytes_per_sec: f64,
    pub write_bytes_per_sec: f64,
}

/// Collects every process, ranked by the selected column.
pub struct ProcessCollector {
    pub top: Vec<ProcessSample>,
    pub total: usize,
    sort_by: SortBy,
    prev_cpu: HashMap<u32, u64>,
    prev_io: HashMap<u32, (u64, u64)>,
    prev_total_cpu: u64,
    prev_time: Instant,
}

impl ProcessCollector {
    /// Create a collector seeded with the current per-process CPU counters.
    pub fn new(sort_by: SortBy) -> anyhow::Result<Self> {
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
            sort_by,
            prev_cpu,
            prev_io: HashMap::new(),
            prev_total_cpu,
            prev_time: Instant::now(),
        })
    }

    /// Active ranking column.
    pub fn sort_by(&self) -> SortBy {
        self.sort_by
    }

    /// Label for the active process ranking.
    pub fn sort_label(&self) -> &'static str {
        match self.sort_by {
            SortBy::Cpu => "cpu",
            SortBy::Mem => "mem",
            SortBy::Read => "read",
            SortBy::Write => "write",
        }
    }

    /// Sort the current rows by the active column, without re-reading /proc.
    pub fn sort_rows(&mut self) {
        match self.sort_by {
            SortBy::Cpu => self
                .top
                .sort_by(|a, b| b.cpu_percent.total_cmp(&a.cpu_percent)),
            SortBy::Mem => self.top.sort_by_key(|a| std::cmp::Reverse(a.mem_kb)),
            SortBy::Read => self
                .top
                .sort_by(|a, b| b.read_bytes_per_sec.total_cmp(&a.read_bytes_per_sec)),
            SortBy::Write => self
                .top
                .sort_by(|a, b| b.write_bytes_per_sec.total_cmp(&a.write_bytes_per_sec)),
        }
    }

    /// Cycle the ranking column.
    pub fn cycle_sort(&mut self) {
        self.sort_by = match self.sort_by {
            SortBy::Cpu => SortBy::Mem,
            SortBy::Mem => SortBy::Read,
            SortBy::Read => SortBy::Write,
            SortBy::Write => SortBy::Cpu,
        };
    }
}

impl Collector for ProcessCollector {
    fn update(&mut self) -> anyhow::Result<()> {
        let (total_cpu, num_cores) = read_cpu_summary()?;
        let delta_total = total_cpu.saturating_sub(self.prev_total_cpu);

        let elapsed = self.prev_time.elapsed().as_secs_f64().max(0.001);
        let mut top = Vec::new();
        let mut cpu_counters = HashMap::new();
        let mut io_counters = HashMap::new();

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

            // /proc/[pid]/io is owner-only, so other users' processes read as 0.
            let (read_bytes, write_bytes) = read_io(pid).unwrap_or((0, 0));
            let (prev_read, prev_write) = self
                .prev_io
                .get(&pid)
                .copied()
                .unwrap_or((read_bytes, write_bytes));
            io_counters.insert(pid, (read_bytes, write_bytes));
            let read_bytes_per_sec = read_bytes.saturating_sub(prev_read) as f64 / elapsed;
            let write_bytes_per_sec = write_bytes.saturating_sub(prev_write) as f64 / elapsed;

            top.push(ProcessSample {
                pid,
                name,
                cpu_percent,
                mem_kb,
                read_bytes_per_sec,
                write_bytes_per_sec,
            });
        }

        self.total = top.len();
        self.top = top;
        self.sort_rows();
        self.prev_cpu = cpu_counters;
        self.prev_io = io_counters;
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

/// Extras only needed by the process detail overlay.
pub struct DetailExtras {
    pub state: char,
    pub threads: u64,
    pub cmdline: String,
    /// Total GPU engine time in nanoseconds across the process's DRM clients.
    pub gpu_ns: u64,
    /// GPU memory attributed to the process, in KiB.
    pub gpu_mem_kb: u64,
}

/// Read state, threads, command line and DRM counters for one process.
pub fn detail_extras(pid: u32) -> Option<DetailExtras> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let close = stat.rfind(')')?;
    let tail: Vec<&str> = stat[close + 1..].split_whitespace().collect();
    let state = tail.first()?.chars().next()?;
    let threads = tail.get(17)?.parse().ok()?;

    let cmdline = std::fs::read(format!("/proc/{pid}/cmdline"))
        .map(|bytes| {
            String::from_utf8_lossy(&bytes)
                .split('\0')
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();

    let (gpu_ns, gpu_mem_kb) = read_drm_fdinfo(pid);
    Some(DetailExtras {
        state,
        threads,
        cmdline,
        gpu_ns,
        gpu_mem_kb,
    })
}

/// Sum DRM engine nanoseconds and client memory across `/proc/[pid]/fdinfo/*`.
fn read_drm_fdinfo(pid: u32) -> (u64, u64) {
    let Ok(entries) = std::fs::read_dir(format!("/proc/{pid}/fdinfo")) else {
        return (0, 0);
    };
    let mut gpu_ns = 0u64;
    let mut gpu_mem_kb = 0u64;
    for entry in entries.flatten() {
        let Ok(contents) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        for line in contents.lines() {
            if let Some(rest) = line.strip_prefix("drm-engine-") {
                if let Some((_, value)) = rest.split_once(':')
                    && let Some(ns) = value
                        .trim()
                        .strip_suffix(" ns")
                        .and_then(|v| v.trim().parse::<u64>().ok())
                {
                    gpu_ns = gpu_ns.saturating_add(ns);
                }
            } else if let Some(rest) = line.strip_prefix("drm-total-system")
                && let Some((_, value)) = rest.split_once(':')
                && let Some(kb) = value
                    .trim()
                    .strip_suffix(" KiB")
                    .and_then(|v| v.trim().parse::<u64>().ok())
            {
                gpu_mem_kb = gpu_mem_kb.saturating_add(kb);
            }
        }
    }
    (gpu_ns, gpu_mem_kb)
}

/// Read disk I/O counters from `/proc/[pid]/io`; unreadable for other users.
fn read_io(pid: u32) -> Option<(u64, u64)> {
    let contents = std::fs::read_to_string(format!("/proc/{pid}/io")).ok()?;
    parse_io(&contents)
}

fn parse_io(contents: &str) -> Option<(u64, u64)> {
    let mut read_bytes = None;
    let mut write_bytes = None;
    for line in contents.lines() {
        if let Some(rest) = line.strip_prefix("read_bytes:") {
            read_bytes = rest.trim().parse::<u64>().ok();
        } else if let Some(rest) = line.strip_prefix("write_bytes:") {
            write_bytes = rest.trim().parse::<u64>().ok();
        }
    }
    Some((read_bytes?, write_bytes?))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_io_reads_disk_counters() {
        let sample = "rchar: 100\nwchar: 200\nread_bytes: 4096\nwrite_bytes: 8192\n";
        assert_eq!(parse_io(sample), Some((4096, 8192)));
        assert_eq!(parse_io("rchar: 1\n"), None);
    }

    #[test]
    fn detail_extras_read_state_threads_and_cmdline() {
        let extras = detail_extras(std::process::id()).expect("own process");
        assert!(extras.state.is_ascii_alphabetic());
        assert!(extras.threads >= 1);
        assert!(!extras.cmdline.is_empty());
    }

    #[test]
    fn cycle_sort_walks_every_column() {
        let mut collector = ProcessCollector {
            top: Vec::new(),
            total: 0,
            sort_by: SortBy::Cpu,
            prev_cpu: HashMap::new(),
            prev_io: HashMap::new(),
            prev_total_cpu: 0,
            prev_time: Instant::now(),
        };
        let mut labels = Vec::new();
        for _ in 0..4 {
            collector.cycle_sort();
            labels.push(collector.sort_label());
        }
        assert_eq!(labels, vec!["mem", "read", "write", "cpu"]);
    }
}
