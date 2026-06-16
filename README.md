
# Zenith — Ultra-lightweight Linux system monitor

Zenith is a modern, minimal terminal monitor built for Linux distributions without systemd. It reads `/proc` and `/sys` directly, renders a compact TUI in `ratatui`, and keeps runtime overhead extremely low.

<p align="center">
  <strong>Fast.</strong>
  &nbsp;|&nbsp;
  <strong>Minimal.</strong>
  &nbsp;|&nbsp;
  <strong>Beautiful.</strong>
</p>

<a name="features"></a>
## Key Features

- **Direct Linux integration** via `/proc` and `/sys` parsing.
- **Low-memory TUI** using `ratatui` + `crossterm`.
- **CPU + memory gauges** with compact, readable visuals.
- **Top process list** sorted by RSS.
- **Details pane** with load averages, per-core usage, and process inspect mode.
- **Keyboard + mouse support** for selection, details, and process actions.
- **Kill actions** with transient status messages.
- **Installable command** by copying the release binary into `~/.local/bin`.

<a name="usage"></a>
## Quick Start

```bash
# Build a release binary
cargo build --release

# Run it locally
./target/release/zenith
```

> **Tip:** For the best appearance, run Zenith in a UTF-8 terminal such as `alacritty`, `kitty`, or `gnome-terminal`.

<a name="controls"></a>
## Controls

| Key | Action |
| --- | ------ |
| `q` | Quit |
| `d` / `F4` | Toggle Details pane |
| `Enter` | Toggle process inspect/details mode |
| `↑` / `↓` | Move selection |
| `k` | Send `SIGTERM` to selected process |
| `K` | Send `SIGKILL` to selected process |
| Mouse click | Select row |
| Double click | Toggle process details |

<a name="layout"></a>
## Layout Overview

- **Top panel** — aggregate CPU usage.
- **Middle panel** — memory usage gauge.
- **Bottom panel** — top process table.
- **Details pane** — optional side panel showing per-core gauges or selected process details.

<a name="install"></a>
## Install as a Command

Run the installer script to build and install the binary for your user:

```bash
chmod +x install.sh
./install.sh
```

Then add `~/.local/bin` to your `PATH` if it is not already there:

```bash
export PATH="$HOME/.local/bin:$PATH"
```

Now you can launch Zenith from anywhere:

```bash
zenith
```

<details>
<summary><strong>System-wide install</strong></summary>

```bash
./install.sh --system
```

</details>

<a name="notes"></a>
## Notes

> **Note:** Zenith avoids heavy runtime dependencies and is designed to behave well on non-systemd distros like Slackware, Arch, Gentoo, and Alpine.

> **Heads-up:** If the UI fails to start, verify that your terminal supports raw mode and that you can read from `/proc`.

<a name="contributing"></a>
## Contributing

Zenith is built around small, lightweight improvements. Contributions are welcome if they keep the project fast and simple.

- Prefer direct `/proc` parsing over large libraries.
- Keep new features optional and low-cost.
- Document keyboard + mouse behavior clearly.

<a name="license"></a>
## License

MIT

