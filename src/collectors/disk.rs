use super::Collector;
use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::time::Instant;

/// Per-device disk I/O rates.
pub struct DiskSample {
    pub device: String,
    pub read_bytes_per_sec: f64,
    pub write_bytes_per_sec: f64,
}

/// Per-mount space usage.
pub struct PartitionSample {
    pub mount: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
}

/// Compressed RAM block device usage.
pub struct ZramSample {
    pub name: String,
    pub used_bytes: u64,
    pub total_bytes: u64,
}

/// Collects disk I/O rates, partition and zram usage.
pub struct DiskCollector {
    pub io: Vec<DiskSample>,
    pub partitions: Vec<PartitionSample>,
    pub zram: Vec<ZramSample>,
    prev_reads: HashMap<String, u64>,
    prev_writes: HashMap<String, u64>,
    prev_time: Instant,
}

struct DiskStat {
    device: String,
    sectors_read: u64,
    sectors_written: u64,
}

impl DiskCollector {
    /// Create a collector seeded with the current `/proc/diskstats` counters.
    pub fn new() -> anyhow::Result<Self> {
        let stats = parse_diskstats()?;

        let mut prev_reads = HashMap::with_capacity(stats.len());
        let mut prev_writes = HashMap::with_capacity(stats.len());
        for stat in stats {
            prev_reads.insert(stat.device.clone(), stat.sectors_read);
            prev_writes.insert(stat.device, stat.sectors_written);
        }

        Ok(Self {
            io: Vec::new(),
            partitions: Vec::new(),
            zram: Vec::new(),
            prev_reads,
            prev_writes,
            prev_time: Instant::now(),
        })
    }
}

impl Collector for DiskCollector {
    fn update(&mut self) -> anyhow::Result<()> {
        let stats = parse_diskstats()?;
        let elapsed_secs = self.prev_time.elapsed().as_secs_f64();

        let mut io = Vec::new();
        let mut reads = HashMap::with_capacity(stats.len());
        let mut writes = HashMap::with_capacity(stats.len());

        for stat in stats {
            let prev_read = self
                .prev_reads
                .get(&stat.device)
                .copied()
                .unwrap_or(stat.sectors_read);
            let prev_write = self
                .prev_writes
                .get(&stat.device)
                .copied()
                .unwrap_or(stat.sectors_written);

            reads.insert(stat.device.clone(), stat.sectors_read);
            writes.insert(stat.device.clone(), stat.sectors_written);

            if !is_relevant_device(&stat.device) {
                continue;
            }

            let read_delta = stat.sectors_read.saturating_sub(prev_read);
            let write_delta = stat.sectors_written.saturating_sub(prev_write);

            let read_rate = rate_bytes_per_sec(read_delta, elapsed_secs);
            let write_rate = rate_bytes_per_sec(write_delta, elapsed_secs);

            io.push(DiskSample {
                device: stat.device,
                read_bytes_per_sec: read_rate,
                write_bytes_per_sec: write_rate,
            });
        }

        io.sort_by(|a, b| a.device.cmp(&b.device));

        self.io = io;
        self.partitions = read_partitions();
        self.zram = read_zram();
        self.prev_reads = reads;
        self.prev_writes = writes;
        self.prev_time = Instant::now();

        Ok(())
    }
}

fn rate_bytes_per_sec(delta_sectors: u64, elapsed_secs: f64) -> f64 {
    if elapsed_secs <= 0.0 {
        return 0.0;
    }
    delta_sectors as f64 * 512.0 / elapsed_secs
}

/// Parse `/proc/diskstats` into sector counters per device.
fn parse_diskstats() -> anyhow::Result<Vec<DiskStat>> {
    let contents = std::fs::read_to_string("/proc/diskstats")?;
    let mut stats = Vec::new();

    for line in contents.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 10 {
            continue;
        }

        let Ok(sectors_read) = fields[5].parse::<u64>() else {
            continue;
        };
        let Ok(sectors_written) = fields[9].parse::<u64>() else {
            continue;
        };

        stats.push(DiskStat {
            device: fields[2].to_string(),
            sectors_read,
            sectors_written,
        });
    }

    Ok(stats)
}

/// Skip pseudo devices and devices without a reported size.
fn is_relevant_device(device: &str) -> bool {
    if device.starts_with("loop") || device.starts_with("ram") || device.starts_with("dm-") {
        return false;
    }

    let path = format!("/sys/block/{device}/size");
    match std::fs::read_to_string(path) {
        Ok(raw) => raw
            .trim()
            .parse::<u64>()
            .map(|size| size > 0)
            .unwrap_or(false),
        Err(_) => false,
    }
}

/// Read zram devices: `mem_used_total` over the configured disk size.
fn read_zram() -> Vec<ZramSample> {
    let Ok(entries) = std::fs::read_dir("/sys/block") else {
        return Vec::new();
    };

    let mut zram = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with("zram") {
            continue;
        }

        let base = format!("/sys/block/{name}");
        let Ok(total_bytes) = read_u64(&format!("{base}/disksize")) else {
            continue;
        };
        let used_bytes = std::fs::read_to_string(format!("{base}/mm_stat"))
            .ok()
            .and_then(|raw| parse_mm_stat(&raw))
            .unwrap_or(0);

        zram.push(ZramSample {
            name,
            used_bytes,
            total_bytes,
        });
    }

    zram.sort_by(|a, b| a.name.cmp(&b.name));
    zram
}

fn read_u64(path: &str) -> anyhow::Result<u64> {
    Ok(std::fs::read_to_string(path)?.trim().parse::<u64>()?)
}

/// `mem_used_total` is the third field of `/sys/block/zram*/mm_stat`.
fn parse_mm_stat(raw: &str) -> Option<u64> {
    raw.split_whitespace().nth(2)?.parse().ok()
}

/// Read supported real filesystems from `/proc/mounts` and stat each mount.
fn read_partitions() -> Vec<PartitionSample> {
    let Ok(contents) = std::fs::read_to_string("/proc/mounts") else {
        return Vec::new();
    };

    let mut partitions = Vec::new();
    for line in contents.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 3 {
            continue;
        }

        let fstype = fields[2];
        if !matches!(fstype, "ext4" | "btrfs" | "xfs" | "vfat" | "ntfs") {
            continue;
        }

        let mount = unescape_mount(fields[1]);
        let Ok(c_mount) = CString::new(mount.as_str()) else {
            continue;
        };

        let mut raw_stats = std::mem::MaybeUninit::<libc::statvfs>::uninit();
        let result = unsafe { libc::statvfs(c_mount.as_ptr(), raw_stats.as_mut_ptr()) };
        if result != 0 {
            continue;
        }
        let stats = unsafe { raw_stats.assume_init() };

        let total = u128::from(stats.f_blocks) * u128::from(stats.f_frsize);
        let free = u128::from(stats.f_bavail) * u128::from(stats.f_frsize);

        partitions.push(PartitionSample {
            mount,
            total_bytes: u64::try_from(total).unwrap_or(u64::MAX),
            free_bytes: u64::try_from(free).unwrap_or(u64::MAX),
        });
    }

    // Bind mounts expose the same filesystem at several paths. Keep the shortest
    // mount path for each identical (total, free) filesystem snapshot.
    partitions.sort_by_key(|partition| partition.mount.len());
    let mut seen = HashSet::new();
    partitions.retain(|partition| seen.insert((partition.total_bytes, partition.free_bytes)));
    partitions.sort_by(|a, b| a.mount.cmp(&b.mount));

    partitions
}

/// Decode the octal escapes used by `/proc/mounts` (e.g. `\040` for space).
fn unescape_mount(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut output = String::with_capacity(input.len());
    let mut index = 0;

    while index < chars.len() {
        if chars[index] == '\\'
            && index + 3 < chars.len()
            && chars[index + 1..=index + 3]
                .iter()
                .all(|c| matches!(c, '0'..='7'))
        {
            let code = chars[index + 1..=index + 3]
                .iter()
                .fold(0u32, |acc, c| acc * 8 + (*c as u32 - '0' as u32));
            if let Some(decoded) = char::from_u32(code) {
                output.push(decoded);
            }
            index += 4;
        } else {
            output.push(chars[index]);
            index += 1;
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::parse_mm_stat;

    #[test]
    fn mm_stat_uses_mem_used_total() {
        assert_eq!(
            parse_mm_stat("4096 1024 2048 0 2048 0 0 0 0 0\n"),
            Some(2048)
        );
        assert_eq!(parse_mm_stat(""), None);
    }
}
