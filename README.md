# RRM — Rust-based Resource Monitor

A fast, terminal-based system resource monitor for Linux, powered by `/proc` and `/sys`.

![screenshot](assets/screenshot.png)


## Install

```sh
cargo install --git https://github.com/fedeaielli/rrm
```

## Usage

```sh
rrm
```

## Tabs

`system` · `cpu` · `memory` · `disk` · `network` · `gpu` · `battery` · `processes` · `fans` · `theme`

The charts are drawn with braille glyphs, so the terminal font must cover the U+2800 block.

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
background = "#0A0A0A"
logo = "#00BFFF"
key = "#00BFFF"
value = "#64D282"
```

`background` also accepts `transparent` to let the terminal background show through.

## Keybindings

| Key | Action |
|-----|--------|
| ←/→ or Tab/Shift+Tab | Previous/Next tab |
| 1-9 | Jump to tab: 1 system, 2 cpu, 3 memory, 4 disk, 5 network, 6 gpu, 7 battery, 8 processes, 9 fans |
| PageDown | Open the theme tab (nothing selected) |
| ↑/↓ (Theme tab) | Select palette entry |
| ←/→ (Theme tab) | Change the selected color when an entry is selected, otherwise change tab |
| Enter (Theme tab) | Type a hex color |
| Esc (Theme tab) | Cancel the hex editor, then deselect and hand the arrows back to the tab bar |
| +/- | Increase/decrease refresh rate (100ms step) |
| q / Ctrl+C | Quit |

## Supported Metrics

- System overview: distro, host, kernel, uptime, packages, shell, desktop, terminal, CPU/GPU, memory, disk, load
- CPU usage (total and per-core), frequency, and temperature
- RAM and swap usage
- Disk I/O rates, partition space and zram usage
- Network RX/TX rates per interface, plus a dedicated block for Tailscale
- GPU usage, VRAM/shared memory, clock, driver and temperature (Intel/AMD via sysfs; Intel usage is estimated from RC6 residency)
- Battery charge, status, power draw and health
- Top processes by CPU or memory
- Fan speeds per sensor, scaled to the sensor's maximum rpm when reported
- Editable palette, saved to `config.toml`

## License

MIT
