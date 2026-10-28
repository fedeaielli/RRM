# RRM — Rust-based Resource Monitor

A fast, terminal-based system resource monitor for Linux, powered by `/proc` and `/sys`.


## Install

```sh
cargo install --git https://github.com/fedeaielli/rrm
```

## Usage

```sh
rrm
```

## Config

Configuration is optional. RRM reads it from `$XDG_CONFIG_HOME/rrm/config.toml`, falling back to `~/.config/rrm/config.toml`. Built-in defaults are used when no file exists.

| Option | Section | Type | Default | Description |
|---|---|---|---|---|
| `refresh_ms` | `display` | integer | `1000` | Refresh interval in milliseconds |
| `accent_color` | `display` | string | `"#00BFFF"` | Accent color as a `#RRGGBB` hex value |
| `top_n` | `processes` | integer | `15` | Number of processes shown in the Processes tab |
| `sort_by` | `processes` | string | `"cpu"` | Process sort column: `"cpu"` or `"mem"` |
| `accent`, `good`, `warn`, `crit`, `rx`, `tx`, `banner`, `background`, `logo`, `key`, `value` | `colors` | string | palette defaults | Palette overrides as `#RRGGBB` (or `transparent`); edited from the Theme tab |

Example:

```toml
[display]
refresh_ms = 1000
accent_color = "#00BFFF"

[processes]
top_n = 15
sort_by = "cpu"

[colors]
accent = "#00BFFF"
good = "#64D282"
warn = "#F0B432"
crit = "#DC4646"
rx = "#50B4DC"
tx = "#B45ADC"
banner = "#B45ADC"
```

## Keybindings

| Key | Action |
|-----|--------|
| ←/→ or Tab/Shift+Tab | Previous/Next tab |
| 1-9 | Jump to tab |
| PageDown | Open the theme tab (nothing selected) |
| ↑/↓ (Theme tab) | Select palette entry |
| ←/→ (Theme tab) | Change the selected color (saved to config.toml) |
| Enter (Theme tab) | Type a hex color |
| +/- | Increase/decrease refresh rate (100ms step) |
| q / Ctrl+C | Quit |

## Supported Metrics

- System overview: distro, host, kernel, uptime, packages, shell, desktop, terminal, CPU/GPU, memory, disk, load
- CPU usage (total and per-core), frequency, and temperature
- RAM and swap usage
- Disk I/O rates and partition space
- Network RX/TX rates per interface
- GPU usage, VRAM/shared memory, clock, driver and temperature (sysfs; Intel usage is estimated from RC6 residency) (Intel/AMD via sysfs)
- Battery charge, status, and health
- Top processes by CPU or memory
- Fan speeds per sensor, scaled to the sensor's maximum rpm when reported

## License

MIT
