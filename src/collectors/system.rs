use super::Collector;

/// Static identity of the machine plus the live uptime/load.
pub struct SystemSample {
    pub distro: String,
    pub distro_id: String,
    pub user: String,
    pub host: String,
    pub kernel: String,
    pub shell: String,
    pub desktop: String,
    pub terminal: String,
    pub packages: Option<usize>,
    pub package_manager: String,
    pub uptime_secs: u64,
    pub load: [f32; 3],
}

/// Reads distribution and host information from `/proc` and `/etc`.
pub struct SystemCollector {
    pub latest: SystemSample,
}

impl SystemCollector {
    pub fn new() -> anyhow::Result<Self> {
        let os_release = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
        let distro = os_release_value(&os_release, "PRETTY_NAME")
            .or_else(|| os_release_value(&os_release, "NAME"))
            .unwrap_or_else(|| "unknown".to_string());
        let distro_id = os_release_value(&os_release, "ID")
            .unwrap_or_default()
            .to_lowercase();
        let (packages, package_manager) = package_count();

        let mut collector = Self {
            latest: SystemSample {
                distro,
                distro_id,
                user: std::env::var("USER").unwrap_or_else(|_| "user".to_string()),
                host: read_trim("/proc/sys/kernel/hostname").unwrap_or_default(),
                kernel: read_trim("/proc/sys/kernel/osrelease").unwrap_or_default(),
                shell: basename(&std::env::var("SHELL").unwrap_or_default()),
                desktop: desktop_name(),
                terminal: std::env::var("TERM").unwrap_or_default(),
                packages,
                package_manager,
                uptime_secs: 0,
                load: [0.0; 3],
            },
        };
        collector.update()?;
        Ok(collector)
    }
}

impl Collector for SystemCollector {
    fn update(&mut self) -> anyhow::Result<()> {
        let uptime = std::fs::read_to_string("/proc/uptime")?;
        self.latest.uptime_secs = uptime
            .split_whitespace()
            .next()
            .and_then(|value| value.parse::<f64>().ok())
            .unwrap_or(0.0) as u64;

        let load = std::fs::read_to_string("/proc/loadavg")?;
        self.latest.load = parse_load(&load);
        Ok(())
    }
}

fn read_trim(path: &str) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
}

fn os_release_value(contents: &str, key: &str) -> Option<String> {
    contents.lines().find_map(|line| {
        let (name, value) = line.split_once('=')?;
        (name.trim() == key).then(|| value.trim().trim_matches('"').to_string())
    })
}

fn basename(path: &str) -> String {
    path.rsplit('/').next().unwrap_or_default().to_string()
}

fn desktop_name() -> String {
    for key in [
        "XDG_CURRENT_DESKTOP",
        "DESKTOP_SESSION",
        "XDG_SESSION_DESKTOP",
    ] {
        if let Ok(value) = std::env::var(key)
            && !value.is_empty()
        {
            return value;
        }
    }
    "unknown".to_string()
}

/// `(count, manager)` from pacman, falling back to dpkg.
fn package_count() -> (Option<usize>, String) {
    if let Ok(entries) = std::fs::read_dir("/var/lib/pacman/local") {
        let count = entries
            .flatten()
            .filter(|entry| entry.path().is_dir())
            .count();
        if count > 0 {
            return (Some(count), "pacman".to_string());
        }
    }
    if let Ok(contents) = std::fs::read_to_string("/var/lib/dpkg/status") {
        let count = contents
            .lines()
            .filter(|line| line.starts_with("Package: "))
            .count();
        if count > 0 {
            return (Some(count), "dpkg".to_string());
        }
    }
    (None, String::new())
}

fn parse_load(contents: &str) -> [f32; 3] {
    let mut load = [0.0; 3];
    for (slot, token) in load.iter_mut().zip(contents.split_whitespace()) {
        *slot = token.parse().unwrap_or(0.0);
    }
    load
}

pub fn humanize_uptime(secs: u64) -> String {
    let days = secs / 86_400;
    let hours = secs % 86_400 / 3600;
    let minutes = secs % 3600 / 60;
    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

#[cfg(test)]
mod tests {
    use super::{humanize_uptime, os_release_value, parse_load};

    #[test]
    fn os_release_strips_quotes_and_ignores_missing_keys() {
        let contents = "NAME=\"Arch Linux\"\nID=arch\n";
        assert_eq!(
            os_release_value(contents, "NAME").as_deref(),
            Some("Arch Linux")
        );
        assert_eq!(os_release_value(contents, "ID").as_deref(), Some("arch"));
        assert_eq!(os_release_value(contents, "VERSION"), None);
    }

    #[test]
    fn uptime_is_humanized() {
        assert_eq!(humanize_uptime(0), "0m");
        assert_eq!(humanize_uptime(60), "1m");
        assert_eq!(humanize_uptime(3_660), "1h 1m");
        assert_eq!(humanize_uptime(90_000), "1d 1h");
    }

    #[test]
    fn load_reads_three_values() {
        assert_eq!(parse_load("0.50 0.25 0.10 1/100 123"), [0.5, 0.25, 0.1]);
        assert_eq!(parse_load("garbage"), [0.0; 3]);
    }
}
