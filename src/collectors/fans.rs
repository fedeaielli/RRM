use super::Collector;

/// A snapshot of a single fan.
pub struct FanSample {
    pub name: String,
    pub rpm: u64,
    pub max_rpm: Option<u64>,
}

/// Collects fan speeds from `/sys/class/hwmon`.
pub struct FansCollector {
    pub fans: Vec<FanSample>,
}

impl FansCollector {
    /// Create a collector and populate it with an initial sample.
    pub fn new() -> anyhow::Result<Self> {
        let mut collector = Self { fans: Vec::new() };
        collector.update()?;
        Ok(collector)
    }
}

impl Collector for FansCollector {
    fn update(&mut self) -> anyhow::Result<()> {
        let mut fans = Vec::new();

        for entry in std::fs::read_dir("/sys/class/hwmon")?.flatten() {
            let path = entry.path();
            let device = read_trim(&path.join("name")).unwrap_or_else(|| "hwmon".to_string());

            let Ok(files) = std::fs::read_dir(&path) else {
                continue;
            };
            for file in files.flatten() {
                let file_name = file.file_name().to_string_lossy().into_owned();
                let Some(stem) = file_name.strip_suffix("_input") else {
                    continue;
                };
                if !stem.starts_with("fan") {
                    continue;
                }
                let Some(rpm) = read_u64(&file.path()) else {
                    continue;
                };
                let name = read_trim(&path.join(format!("{stem}_label")))
                    .unwrap_or_else(|| format!("{device} {stem}"));
                fans.push(FanSample {
                    name,
                    rpm,
                    max_rpm: read_u64(&path.join(format!("{stem}_max"))),
                });
            }
        }

        fans.sort_by(|a, b| a.name.cmp(&b.name));
        self.fans = fans;
        Ok(())
    }
}

fn read_trim(path: &std::path::Path) -> Option<String> {
    let value = std::fs::read_to_string(path).ok()?;
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn read_u64(path: &std::path::Path) -> Option<u64> {
    read_trim(path)?.parse().ok()
}
