# devtone

Local lofi for people who live in coding agents.

`devtone` is a single-process daemon: a tiny synth in your headphones, a 32-bar amp in the terminal, and an optional always-on-top HUD. The mix follows **which CLI you are actually using** — [pi](https://github.com/badlogic/pi-mono), [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex CLI](https://github.com/openai/codex), or [OpenCode](https://github.com/sst/opencode) — from local logs only.

Nothing leaves the machine. Prompts, source, and window titles are never stored.

```text
┌ devtone  pi · sonnet · rs · 38 t/s · flow 0.72 ──────── ● running ┐
│  ▂ ▅ ▇ ▃ ▂ ▆ ▇ ▄ ▁ ▅ ▇ ▅ ▃ ▁ ▄ ▆ ▇ ▅ ▂ ▅ ▇ ▄ ▃ ▁ ▆ ▇ ▅ ▂ ▄ ▆ ▇ ▃ │
│  vinyl ████░░  hat ██████  pad ████████  cutoff 2.1k  bpm 78     │
└  space mute   q quit   n hide-notch   1-4 intensity ─────────────┘
```

## Why

Agent sessions already have a pulse: tokens, cache hits, tool bursts, idle. DevTone turns that into lofi (BPM stays in the 72–84 pocket; hats and pad move, tempo does not bounce). It is a companion, not a DAW and not a keylogger.

## Install

Linux, Rust stable, PipeWire or ALSA.

```bash
git clone https://github.com/MachuaninEzequiel/devtone
cd devtone
cargo install --path crates/devtone
```

Or run from the repo:

```bash
cargo build -p devtone --release
./target/release/devtone --no-notch
```

macOS / Windows compile stubs exist; **Linux is the tested platform**.

## Use

```bash
devtone                 # audio + TUI (no overlay on Wayland)
devtone --no-notch      # never open the HUD
devtone --notch         # force the always-on-top HUD (can steal clicks on KWin)
devtone --headless      # audio + socket, no TUI (tmux / background)
devtone --agent pi      # force the active CLI
devtone --intensity 0.7
devtone status
devtone status --json
devtone stop            # 120 ms fade, restore the terminal, exit 0
```

Only one daemon. A second `devtone` talks to the socket at `~/.local/state/devtone/devtone.sock` (override with `DEVTONE_SOCK`).

| Key | Action |
|-----|--------|
| `space` | mute |
| `q` / Ctrl-C | quit |
| `n` | destroy / recreate the notch |
| `1` `2` `3` `4` | intensity 25 / 50 / 75 / 100 |

Notch HUD (280×36, always-on-top): left dots quit, double-click bars mute, drag bars to move, Esc quits. **Off by default on Wayland.** An undecorated 36px window looks like a resize edge to KWin and can grab the pointer (crosshair cursor, clicks go nowhere). Use `--notch` only if you want to try it; `--no-notch` always wins.

## How it knows which CLI is active

`--agent pi|claude|codex|opencode` always wins.

Otherwise, each mapper tick:

1. If the focused window (or the foreground process of that terminal) is one of the four CLIs → that agent.
2. Else the newest usage event younger than 90 seconds.
3. Else idle music.

It **tails** logs. Historical sessions are not replayed.

| CLI | What is read |
|-----|----------------|
| pi | `~/.pi/agent/sessions/**/*.jsonl` — `usage.input/output/cacheRead`, `model` |
| Claude Code | `~/.claude/projects/**/*.jsonl` — `message.usage` |
| Codex CLI | `~/.codex/sessions/**/rollout-*.jsonl` — `token_count` |
| OpenCode | `~/.local/share/opencode/opencode.db` — `session.tokens_*`, `model` |

Never selected: message text, thinking, tool arguments, `session_input.prompt`.

Language (Rust / Python / TS / Go / SQL) colors the scale and root. Vendor name does not. Idle 20 s drops hats and a few BPM.

## Config

`~/.config/devtone/config.toml` — missing or broken file → defaults, no abort.

```toml
intensity = 0.8
notch = true
notch_follow_focus = false   # reserved (v1.1)
fps_tui = 20
fps_notch = 12

[engine]
sample_rate = 44100
buffer = 512

[privacy]
watch_agent_logs = true
watch_active_window = true
```

Set a flag to `false` and that sensor does not start.

## Cost

One process, no children. Audio callback: no `Mutex`, no heap. Spectrum is a 256-point FFT (`microfft`) plus peak-hold, not a giant analyzer.

Targets (headless, laptop on battery):

- RSS p50 &lt; 35 MB
- CPU p95 &lt; 2% of one core while playing
- Release binary, strip + thin LTO, currently ~6 MB

Build without the overlay: `--no-default-features --features tui`.

## Layout

```
crates/
  devtone-core      types, mapper, IPC schema
  devtone-engine    lofi voices + 32-bar spectrum
  devtone-sensors   adapters + focus + arbitration
  devtone-tui       ratatui amp
  devtone-notch     winit + softbuffer HUD
  devtone           binary: lifecycle, socket, cpal
```

## License

MIT. See [LICENSE](LICENSE).
