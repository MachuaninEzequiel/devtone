# devtone

A tiny **lofi daemon** that plays in the background while you code, plus a local amp visualizer that reacts to **pi**, **Claude Code**, **Codex CLI**, and **OpenCode**. Nothing leaves the machine. Source and prompts are never read.

## Install

```bash
cargo install --path crates/devtone
```

Needs Rust 1.74+, ALSA/PipeWire on Linux. Wayland notch is best-effort; use `--no-notch` if the overlay misbehaves.

## Commands

```bash
devtone                 # audio + TUI (and notch, if the feature is on)
devtone --headless      # audio + socket, no TUI
devtone --no-notch      # skip the always-on-top HUD
devtone --agent pi      # force the active CLI
devtone status          # print StateFrame
devtone status --json
devtone stop            # fade 120 ms and exit 0
```

Keys in the TUI: `space` mute, `q` quit, `n` toggle notch, `1`–`4` intensity.

## What it watches

| CLI | Signal |
|---|---|
| pi | `~/.pi/agent/sessions/**/*.jsonl` (`usage` only) |
| Claude Code | `~/.claude/projects/**/*.jsonl` |
| Codex CLI | `~/.codex/sessions/**/rollout-*.jsonl` |
| OpenCode | `~/.local/share/opencode/opencode.db` token columns |

Active CLI: `--agent` > focused terminal/window if it is one of the four > last usage event younger than 90 s.

Config: `~/.config/devtone/config.toml` (created conceptually; missing file uses defaults). Privacy flags `watch_agent_logs` / `watch_active_window` disable sensors.

## Promise

Single process. No child processes. No `Mutex` in the audio callback. Target: `<2%` of one core while playing, RSS p50 `<35 MB`.
