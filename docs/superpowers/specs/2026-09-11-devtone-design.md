# DevTone v1 design

Date: 2026-09-11
Status: approved in conversation; waiting on spec review
Product: `devtone` — daemon de audio lofi + telemetría local de agentes de coding

This spec is the source of truth for implementation. `concepto.md` is the originating note; where they differ, this document wins.

## Goal

One local process that:

1. Plays a light lofi loop from a tiny synth (not a DAW, not system loopback).
2. Visualizes the mix as a 32-bar “amp” in a TTY and in a always-on-top notch HUD.
3. Maps coding-agent activity to musical parameters without reading source or prompts.
4. Detects **exactly which CLI is in use** among: **pi**, **Claude Code**, **Codex CLI**, **OpenCode**.

Promise: `<2%` CPU of one core while playing, `<0.4%` idle, RSS p50 `<35 MB`.

## Non-goals (v1)

- System audio loopback / capturing other apps
- `rustfft` (use `microfft` 256)
- Electron, wgpu, GPU compositor tricks
- Keylogging, reading source, storing full window titles or prompts
- Cursor / Copilot CLI adapters
- Notch follow-focus (`--notch follow-focus` is specified as v1.1, not built)
- Claiming macOS/Windows as tested (stubs allowed in code)

## Platform of record

Linux, KDE Plasma 6, KWin + Wayland, PipeWire, this machine. Other Linux compositors get best-effort focus adapters. macOS/Windows compile stubs only.

## Crate layout

One Cargo workspace. Default features of the binary: `tui` + `notch`.

```
devtone/
  Cargo.toml                 # workspace
  crates/
    devtone-core/            # types, mapper, IPC schema
    devtone-sensors/         # jsonl/sqlite adapters + active window
    devtone-engine/          # audio + spectrum snapshot
    devtone-tui/             # ratatui
    devtone-notch/           # winit + softbuffer
    devtone/                 # bin: orchestrator
  assets/
    samples/                 # tiny mono 44.1k wav (kick/hat/snare), generated or original
    palette.toml
```

Binary features:

```toml
[features]
default = ["tui", "notch"]
tui = ["dep:devtone-tui"]
notch = ["dep:devtone-notch"]
```

`--no-default-features --features tui` must still build a usable headless-or-TUI daemon.

## Process and threads

One process. Zero child processes.

```
main
 ├─ audio thread          (cpal callback)
 ├─ engine tick thread    (64 Hz: sequencer + envelopes)
 ├─ sensor thread         (notify + active window 2 Hz)
 ├─ mapper thread         (4 Hz: StateFrame → MusicParams)
 ├─ tui thread            (20 fps, crossterm)          [TTY and not --headless]
 └─ notch event loop      (main thread if notch on)
```

If notch is enabled, main = winit. TUI runs on a thread. If notch is off, main = TUI (or a socket wait in `--headless --no-notch`).

Communication:

- `StateFrame` — `ArcSwap`, written 4 Hz, read by TUI/notch/mapper
- `MusicParams` — `ArcSwap`, written 4 Hz, read by engine at 64 Hz
- `SpectrumSnap` — `ArcSwap`, written by engine ~20 Hz, read by TUI/notch
- `Command` — bounded `crossbeam_channel`: `Quit | Mute | SetIntensity(f32) | ToggleNotch`

No `Mutex` in the audio callback. No allocations in the audio callback.

External IPC:

- Linux/macOS: Unix socket `~/.local/state/devtone/devtone.sock`
- Windows stub: named pipe `\\.\pipe\devtone`

If the socket exists and answers ping: do not start a second daemon. Frame protocol (length-prefixed JSON):

- `ping` → `pong`
- `stop` → daemon `Command::Quit`
- `status` → one `StateFrame` JSON
- `subscribe` → stream `{state, params, spectrum}` at TUI fps until the client disconnects
- `cmd` (`mute` / `intensity` / `toggle_notch` / `quit`) → `Command`

`devtone stop`/`status` are one-shot clients. A second interactive TUI is a `subscribe` client, not a second engine.

## Types (core)

```rust
pub enum AgentKind { Pi, ClaudeCode, CodexCli, OpenCode, None }

pub struct StateFrame {
    pub ts_ms: u64,
    pub agent: AgentKind,
    pub model: TinyStr,            // "sonnet", "opus", "gpt-...", "pi-..."
    pub out_tps: f32,
    pub in_tok_delta: u32,
    pub cache_read_delta: u32,
    pub tools_per_min: f32,
    pub lang: Lang,                // Rs | Py | Ts | Go | Sql | Other
    pub focus: Focus,              // Editor | Terminal | Browser | AgentCli | Other
    pub flow: f32,                 // 0..1
    pub stress: f32,               // 0..1
    pub agent_streaming: bool,
}

pub struct MusicParams {
    pub bpm: f32,                  // 68..92, mostly 72..84
    pub root_midi: u8,             // 48..=60
    pub scale: Scale,              // MinorPentatonic | Dorian | MajorPent
    pub swing: f32,                // 0.50..0.66
    pub layers: LayerMix,
    pub cutoff_hz: f32,            // 400..4200
    pub reverb: f32,               // 0..0.45
    pub crackle: f32,              // 0..0.2
    pub tension: f32,              // 0..1
}

pub struct LayerMix { pub vinyl: f32, pub kick: f32, pub hat: f32, pub bass: f32, pub pad: f32, pub lead: f32 }

pub struct SpectrumSnap { pub bars: [u8; 32], pub peak: u8, pub rms: u8 }

pub struct TokenDelta {
    pub agent: AgentKind,
    pub out: u32,
    pub inn: u32,
    pub cache_read: u32,
    pub cache_write: u32,
    pub model: TinyStr,
    pub ts_ms: u64,
    pub tools: u32,
}
```

`TinyStr` = `[u8; 24]` + length. No `String` on the hot path.

`Focus::AgentCli` means the focused window’s process is one of the four CLIs.

## CLI detection (exact)

`--agent pi|claude|codex|opencode` always wins.

Otherwise, every mapper tick (4 Hz):

1. If the focused window resolves to one of the four CLIs → that `AgentKind`.
2. Else if the newest `TokenDelta` is younger than 90 seconds → that delta’s agent.
3. Else `AgentKind::None`.

Resolving the focused window on Linux:

- Read the active window from KWin (then hypr/niri/sway, then xprop).
- If the window process *is* `pi` / `claude` / `codex` / `opencode` (or a node cmdline whose last component is one of those / `pi-coding-agent`), use that.
- If the window is a terminal emulator (Konsole, Kitty, Ghostty, Alacritty, WezTerm, foot, …), inspect the **foreground process group of that TTY** via `/proc` and match those same binary names. This is the common case: pi runs inside Konsole, not as the window itself.
- Never match `devtone`.

Telemetry for the **selected** agent still comes from that agent’s adapter. Focus alone can set `agent` with zeros if logs are quiet.

## Sensors

```rust
trait AgentSource {
    fn name(&self) -> AgentKind;
    fn watch_paths(&self) -> Vec<PathBuf>;
    fn ingest(&mut self, path: &Path, kind: IngestKind) -> Option<TokenDelta>;
}

enum IngestKind { Line(String), FileChanged }
```

Watch with `notify` + 250 ms debounce. No blind polling of file contents. OpenCode may inotify the db/WAL and then do a cheap readonly SQL read (that is not a poll loop).

### Pi

- Paths: `~/.pi/agent/sessions/**/*.jsonl`
- Parse `type == "message"` with `usage: { input, output, cacheRead, cacheWrite, ... }` and `model`
- Delta vs last seen usage on that session id
- Ignore `content`, thinking, tool payloads

### Claude Code

- Paths: `~/.claude/projects/**/*.jsonl`
- Parse `type == "assistant"` (and equivalent) with `message.usage`
- Delta vs last uuid / last usage
- Ignore prompts and tool args

### Codex CLI

- Paths: `~/.codex/sessions/**/rollout-*.jsonl`
- Parse `type == "event_msg"` / `payload.type == "token_count"` using `last_token_usage` (preferred) or delta of `total_token_usage`
- Ignore message text in `response_item`

### OpenCode

- Path: `~/.local/share/opencode/opencode.db` (+ `-wal`)
- Readonly URI `file:...?mode=ro`
- Read `session.tokens_input/output/reasoning/cache_read/cache_write`, `model`, `time_updated`
- Never SELECT `session_input.prompt` or message bodies
- If the schema is empty or locked, skip the tick

### Language and focus

Linux focus, in order:

1. KWin/Plasma 6 (this host)
2. `hyprctl` / `niri` / `sway`
3. `xprop` (X11 / XWayland)
4. Fallback: running agent PIDs from `/proc` (does not set `Focus`, only helps last-event)

Language heuristic: last path-like token in the title (`foo.rs`, `bar.ts`) or the agent session cwd if the log metadata has it (pi session lines include `cwd`). Store `Lang` only, never the title string.

Privacy flags in config disable log watching and/or window watching independently.

## Mapper

Tick 4 Hz. Every field:

```
x' = x + α (target - x)
α_slow  = 0.08   // bpm, root, reverb
α_mid   = 0.18   // hats, cutoff
α_fast  = 0.35   // pad when agent_streaming
```

Vendor does not pick the scale. **Language and flow do.** Pi vs Claude with `Lang::Rs` both go Dorian.

| Signal | Target |
|---|---|
| lang = Rs | Dorian, root D2, cutoff more closed |
| lang = Py | Minor pent, root C2, warmer pad |
| lang = Ts | Major pent, root F2, hats slightly more open |
| lang = Go | Dorian, root E2, dry (reverb low) |
| agent_streaming | pad += 0.35, cutoff += 800, lead pulse 2 bars |
| out_tps high | hat density up, **not** BPM up |
| cache_read_delta large | reverb up, longer pad |
| tools_per_min > 8 | kick more present, sidechain +10% |
| focus = Terminal or AgentCli, plus stress | tension up, crackle up, stay minor |
| idle 20 s (low flow, not streaming) | hat → 0.05, kick → 0.15, BPM −6 |
| test OK / commit (punctual) | 12 s envelope: tension → 0, pad brighter |

BPM lives in 72–84 most of the time.

`agent_streaming` is true when the selected agent produced a token delta in the last 1.5 s.

## Engine

Not a DAW. 4 or 8 bar loop, 6 voices.

Tick 64 Hz: advance musical phase, fire swung one-shots, update envelopes.

Callback 44.1 kHz / 512 frames:

- vinyl: LCG + light HP
- drums: wav table, linear interpolation
- bass: sine + 1st harmonic, degrees 1/5
- pad: 2 saws detuned 7 cents + one-pole LPF
- lead: soft sine, 5% duty
- reverb: Schroeder 4 comb + 2 allpass, ~48 KB
- write stereo i16
- every 3 callbacks: `compute_spectrum()` → `SpectrumSnap`

Samples: kick, snare, hat, optional rhodes one-shot. Mono 44.1 k, 16-bit. Pack `< 1.5 MB`. Prefer original/procedural assets, no third-party copyrighted packs.

cpal default output. If no device: stay in mute, keep feeding the visualizer.

### Spectrum (“amp”, not a blob)

1. Hann + 256-point FFT (`microfft`) of mixed block (or 3 blocks concatenated)
2. 32 log bands 40 Hz–8 kHz
3. `band = max(fft_band, layer_proxy[i])`
4. layer_proxy: kick → bars 0–3, bass → 2–8, pad → 8–20, hats → 20–31
5. Peak hold: if `new > bar { bar = new } else { bar *= 0.86 }`
6. Quantize 16 levels TUI / 24 notch

Palette (from the reference look):

```
bg        #0B0C10
bar[0]    #B7A9F5
bar[1]    #F3C56B
bar[2]    #E8A0B4
bar[3]    #A8E0C8
bar[4]    #C9B8F0
dots      #3A3D46
dot_on    #E8E6EF
```

`color = PALETTE[i % 5]`.

## TUI

ratatui + crossterm. 20 fps, sleep the residual, no spin.

```
┌ devtone  pi · sonnet · rs · 38 t/s · flow 0.72 ──────────── ● running ┐
│  ▂ ▅ ▇ ▃ … 32 half-blocks, 24-bit ANSI                                   │
│  vinyl ████░░  hat ██████  pad ████████  cutoff 2.1k  bpm 78             │
└  space mute   q quit   n hide-notch   i intensity ───────────────────────┘
```

Bars: one row by default; up to 6 rows if the terminal is tall.

Keys:

- `q` / Ctrl-C → clean shutdown
- space → mute (engine keeps running silent; visualizer decays to 0)
- `n` → destroy / recreate notch
- `1..4` → intensity 25/50/75/100

On exit, always `disable_raw_mode` + `LeaveAlternateScreen`, including when the notch sends `Quit`.

## Notch

Not a Mac hardware notch. A 280×36 pill HUD, opaque `#0B0C10`, no titlebar, no resize, `AlwaysOnTop`, 10 px from the top of the focused monitor, horizontally centered.

Hit targets:

- Left 40×36 cluster of 5 dots → `Quit` (hover lights dots)
- Drag on bars → move window
- Double-click bars → mute
- Esc while focused → `Quit`

Render: winit + softbuffer, ~40 KB framebuffer, 12 fps. If `SpectrumSnap` unchanged, skip present. Idle → 2 fps.

Wayland/KWin: best-effort always-on-top. If spawn fails, log once and continue without HUD. `--no-notch` is first-class.

v1.1 (not built): `--notch follow-focus`.

## CLI and lifecycle

```
devtone
devtone --headless
devtone --no-notch
devtone --intensity 0.7
devtone --agent pi|claude|codex|opencode
devtone stop
devtone status
devtone status --json
```

Startup: ping socket → bind → load `~/.config/devtone/config.toml` (defaults if missing) → spawn engine/sensors/mapper → notch on main if enabled → TUI if TTY and not headless → block on event loop.

Shutdown (q, notch dots, `devtone stop`, SIGINT): `Command::Quit` → 120 ms audio fade → drop cpal → restore terminal → unlink socket → exit 0.

## Config

`~/.config/devtone/config.toml`

```toml
intensity = 0.8
notch = true
notch_follow_focus = false
fps_tui = 20
fps_notch = 12

[engine]
sample_rate = 44100
buffer = 512

[privacy]
watch_agent_logs = true
watch_active_window = true
```

Nothing leaves the machine. Broken TOML → defaults + stderr warning, no abort.

## Error policy

Long-lived threads do not panic-abort the product.

| Failure | Behaviour |
|---|---|
| No audio device | Mute engine, spectrum continues, TUI `muted · no-device` |
| Notch fails on Wayland | One warning; audio+TUI continue; `n` retries |
| Unreadable jsonl / sqlite locked | Skip that ingest; no log spam |
| Log schema drift | Ignore the line; `agent` may still come from focus/`--agent` |
| Live socket | `stop`/`status` are forwarded and the second process exits 0. A second interactive `devtone` (TTY, not headless) attaches a TUI to the existing daemon. A second `--headless` exits 1 `already running`. |
| Stale socket file | Unlink and bind |
| SIGINT / TUI panic hook | Always restore terminal |
| `--agent` with no logs | `StateFrame.agent` set, telemetry zeros, idle music |

Devtone file logging is off by default. No writing prompts, source, or full titles.

## Testing

Unit, no I/O:

- Mapper EMA, lang table, idle 20 s, test-ok envelope
- Spectrum peak-hold, 32 log bands, layer_proxy
- Adapters on fixtures (pi/claude/codex jsonl + tiny OpenCode sqlite) → `TokenDelta` without leaking prompt text
- Arbitration: `--agent` > focus > last event < 90 s > `None`
- TinyStr and `status` JSON

Engine:

- Deterministic tick into `Vec<i16>`
- Fade ~0 in 120 ms
- Zero-alloc callback check (optional CI)

Linux integration (this machine):

- `status --json` / `stop` over the socket
- `--headless --no-notch` for 10 s does not crash
- Appending a fixture pi jsonl line moves `agent` / `out_tps` in `status`
- TUI restore is a manual smoke

Weight gates (measured here before publish, not a CI blocker at first):

- RSS p50 < 35 MB
- CPU p95 < 2% of one core on `--headless` 10 min
- Binary strip + `lto=thin` < 6 MB with samples

Notch framebuffer can be unit-tested offscreen. Always-on-top is manual on KWin.

## Implementation order

1. core + silent engine + dummy `SpectrumSnap` LFO + TUI (validate the amp look)
2. real engine: vinyl + kick + hat + pad (usable lofi, no sensors)
3. notch + power dots + same `SpectrumSnap`
4. orchestrator + socket + fade + quit
5. four adapters + focus/arbitration + mapper
6. extras that must exist in the repo: `--agent`, privacy flags, macOS/Windows stubs, README

Step 1 is the visual gate: if bars do not feel like an amp, fix peak-hold and palette before continuing.

## Decisions already locked

- Path: architectural; approach 2 (concepto adapted to this Linux host)
- v1 scope: full concept in the repo; Linux is the tested platform
- Agents: pi, Claude Code, Codex CLI, OpenCode — not Cursor
- Arbitration: focus if CLI, else last event; `--agent` override
- Notch: winit + softbuffer, not layer-shell
- FFT: `microfft` 256
- Music color from language/flow, not vendor name
