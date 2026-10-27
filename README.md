# RRM — Rust-based Resource Monitor

A fast, terminal-based system resource monitor for Linux, powered by `/proc` and `/sys`.


## Install

```sh
cargo install --git https://github.com/faielli/rrm
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

Example:

```toml
[display]
refresh_ms = 1000
accent_color = "#00BFFF"

[processes]
top_n = 15
sort_by = "cpu"
```

## Keybindings

| Key | Action |
|-----|--------|
| ←/→ | Previous/Next tab |
| 1-7 | Jump to tab |
| +/- | Increase/decrease refresh rate (100ms step) |
| q / Ctrl+C | Quit |

## Supported Metrics

- CPU usage (total and per-core), frequency, and temperature
- RAM and swap usage
- Disk I/O rates and partition space
- Network RX/TX rates per interface
- GPU usage, VRAM, and temperature (Intel/AMD via sysfs)
- Battery charge, status, and health
- Top processes by CPU or memory

## License

MIT
