Producto
Nombre: devtone

Binario: devtone

Promesa: daemon de audio lofi + telemetría local de agentes, <2% CPU mientras suena, <0.4% en idle, RAM objetivo <35 MB.
Bashdevtone              # audio + TUI en esta terminal + abre el notch
devtone --headless   # audio + notch, sin TUI (útil en tmux / background)
devtone stop         # apaga la instancia viva (socket)
devtone status       # imprime StateFrame
Al ejecutar devtone, en el mismo instante:

arranca el engine de audio
pinta el ecualizador en la TTY
abre el notch always-on-top
el notch tiene un hit-target de apagado (los dots de la izquierda de tu captura)

Clic en apagar del notch = mute + teardown + restore de terminal + exit 0.

2. Principio de costo
Todo lo caro vive fuera del hot path.


CosaFrecuenciaPresupuestocallback de audio44.1 kHz, bloques 512<0.3 ms/bloque, 0 allocvisualizerTUI 20 fps / notch 12 fpslee un snapshot, no calcula músicasensores de archivosinotify/kqueue + debounce 250 ms0 poll ciegomapper4 HzEMA, sin locks en audioventana notch280×36 px, CPU blitsoftbuffer, no GPU, no wgpu
Prohibido en v1: loopback del sistema, rustfft grande, Electron, wgpu, keylogger, leer el source.
El “amplificador” se simula desde energía por capa del synth + un FFT de 256 puntos sobre el propio output buffer (ya está en RAM). Eso se ve como un analizer de verdad y cuesta microsegundos.

3. Crate layout
Un workspace, features para no arrastrar winit si no hace falta.
textdevtone/
  Cargo.toml                 # workspace
  crates/
    devtone-core/            # tipos, mapper, IPC schema
    devtone-sensors/         # tail jsonl + ventana activa
    devtone-engine/          # audio + spectrum snapshot
    devtone-tui/             # ratatui
    devtone-notch/           # winit + softbuffer
    devtone/                 # bin: orquestador
  assets/
    samples/                 # wav/ogg mono, 44.1k, loopables
    palette.toml
Features del binario:
toml[features]
default = ["tui", "notch"]
tui = ["dep:devtone-tui"]
notch = ["dep:devtone-notch"]
El usuario compile devtone y listo. --no-default-features --features tui si alguien no quiere overlay.

4. Proceso y threads
Un proceso. Cero procesos hijos (menos race, menos RAM).
textmain
 ├─ audio thread          (cpal callback, realtime-ish)
 ├─ engine tick thread    (64 Hz: sequencer + capa envelopes)
 ├─ sensor thread         (notify + active window 2 Hz)
 ├─ mapper thread         (4 Hz: StateFrame → MusicParams)
 ├─ tui thread            (20 fps, crossterm)          [si hay TTY]
 └─ notch event loop      (main thread si --notch)     [winit exige main en macOS]
Regla de plataforma: si hay notch, main = winit. TUI se va a un thread. Si no hay notch, main = TUI.
Comunicación: arc_swap::ArcSwap + crossbeam_channel bounded.
textStateFrame     ArcSwap, escrito 4 Hz, leído por TUI/notch/mapper
MusicParams    ArcSwap, escrito 4 Hz, leído por engine a 64 Hz
SpectrumSnap   ArcSwap, escrito por engine ~20 Hz, leído por TUI/notch
Command        channel: Quit | Mute | SetIntensity(f32)
Nada de Mutex en el callback de audio.
IPC externo (devtone stop): Unix socket en Linux/macOS

~/.local/state/devtone/devtone.sock

en Windows: named pipe \\.\pipe\devtone.
Si al arrancar el socket ya existe y responde ping → no levantás otra instancia; reenviás el comando o attachás TUI.

5. Tipos (core, estables)
Rust// crates/devtone-core/src/lib.rs

#[derive(Clone, Debug, Default)]
pub struct StateFrame {
    pub ts_ms: u64,
    pub agent: AgentKind,          // ClaudeCode | Codex | CopilotCli | CursorApprox | None
    pub model: TinyStr,            // "sonnet", "opus", "gpt-..."
    pub out_tps: f32,              // tokens output / s, EMA
    pub in_tok_delta: u32,
    pub cache_read_delta: u32,
    pub tools_per_min: f32,
    pub lang: Lang,                // Rs | Py | Ts | Go | Sql | Other(u8)
    pub focus: Focus,              // Editor | Terminal | Browser | Other
    pub flow: f32,                 // 0..1
    pub stress: f32,               // 0..1  (tests fail, errors)
    pub agent_streaming: bool,
}

#[derive(Clone, Debug)]
pub struct MusicParams {
    pub bpm: f32,                  // 68..92
    pub root_midi: u8,             // 48..=60
    pub scale: Scale,              // MinorPentatonic | Dorian | MajorPent
    pub swing: f32,                // 0.50..0.66
    pub layers: LayerMix,          // gains 0..1
    pub cutoff_hz: f32,            // 400..4200  lpf del pad/hats
    pub reverb: f32,               // 0..0.45
    pub crackle: f32,              // 0..0.2
    pub tension: f32,              // 0..1  (un semitono extra / disonancia leve)
}

#[derive(Clone, Debug, Default)]
pub struct LayerMix {
    pub vinyl: f32,
    pub kick: f32,
    pub hat: f32,
    pub bass: f32,
    pub pad: f32,
    pub lead: f32,
}

#[derive(Clone, Debug)]
pub struct SpectrumSnap {
    pub bars: [u8; 32],            // 0..255 altura
    pub peak: u8,
    pub rms: u8,
}
TinyStr = array [u8; 24] + len. Cero String en el hot path.

6. Mapper (la parte “música según actividad”)
Tick 4 Hz. Toda transición es EMA, nunca un salto.
textx' = x + α (target - x)
α_lento = 0.08    // bpm, root, reverb
α_medio = 0.18    // hats, cutoff
α_rápido = 0.35   // pad cuando el agente streamea
Tabla de targets:


SeñalTargetlang = RsDorian, root D2, cutoff más cerradolang = PyMinor pent, root C2, pad más cálidolang = Ts/JsMajor pent, root F2, hats un poco más abiertoslang = GoDorian, root E2, groove más seco (reverb bajo)agent_streamingpad += 0.35, cutoff += 800, lead pulse 2 compasesout_tps altohat density ↑, no BPM ↑ (BPM casi fijo)cache_read_delta grandereverb ↑, pad más largotools_per_min > 8kick más presente, sidechain +10%focus = Terminal + stresstension ↑, crackle ↑, escala se queda menoridle 20 s (flow bajo, no streaming)hat → 0.05, kick → 0.15, BPM −6test OK / commit (evento puntual)envelope de 12 s: tension → 0, brillo de pad
BPM vive en 72–84 la mayor parte del tiempo. El lofi se rompe si el tempo baila.

7. Engine de audio (liviano de verdad)
No un DAW. Un loop de 4 o 8 compases + 6 voces.
texttick 64 Hz
  avanza fase musical (beats)
  dispara one-shots (kick, snare, hat) en la grilla swing
  actualiza envelopes
callback 44.1 k / 512
  suma:
    vinyl noise (LCG + HP leve, baratísimo)
    samples de drum (tabla wav, interpolación lineal)
    bass: sine + 1er harm, note del grado 1/5
    pad: 2 saws desafinados 7 cents + one-pole LPF
    lead: sine suave, duty 5%
    reverb: schroeder 4 comb + 2 allpass, buffer 48 KB
  write stereo i16 al device
  cada 3 callbacks: compute_spectrum() → SpectrumSnap
Samples: kick, snare, hat, opcional rhodes one-shot. Todo mono 44.1 k, 16-bit, loop o one-shot. Pack total < 1.5 MB.
cpal output default. Si no hay device, el engine sigue y el visualizer se alimenta igual (modo mute).
Spectrum que se ve como amplificador
No 32 FFT bins crudos (queda feo, todo en graves). Receta:

Hann + FFT 256 del bloque mezclado (o 3 bloques pegados)
32 bandas log 40 Hz–8 kHz
cada banda = max(fft_band, layer_proxy[i])
layer_proxy reparte kick→barras 0–3, bass→2–8, pad→8–20, hats→20–31
peak hold con caída:textif new > bar { bar = new }
else { bar *= 0.86 }          // “amp clásico”
quantize a 16 niveles para TUI / 24 para notch

Eso es exactamente el movimiento de tu captura: barras que pegan y caen, no un blob que respira.
Paleta (sacada de la imagen):
textbg        #0B0C10
bar[0]    #B7A9F5  lavanda
bar[1]    #F3C56B  durazno
bar[2]    #E8A0B4  rosa
bar[3]    #A8E0C8  menta
bar[4]    #C9B8F0
dots      #3A3D46  (inactivos)
dot_on    #E8E6EF  (activo / hover apagar)
Rotación: color = PALETTE[i % 5], igual que el screenshot.

8. TUI
ratatui + crossterm. Sin mouse obligatorio. 20 fps tapado con std::thread::sleep del residual, no spin.
Layout:
text┌ devtone  claude · sonnet · rs · 38 t/s · flow 0.72 ──────────── ● running ┐
│  ▂ ▅ ▇ ▃ ▂ ▆ ▇ ▄ ▁ ▅ ▇ ▅ ▃ ▁ ▄ ▆ ▇ ▅ ▂ ▅ ▇ ▄ ▃ ▁ ▆ ▇ ▅ ▂ ▄ ▆ ▇ ▃        │
│  (32 barras, half-blocks ▁▂▃▄▅▆▇█, colores ANSI 24-bit)                  │
│                                                                          │
│  vinyl ████░░  hat ██████  pad ████████  cutoff 2.1k  bpm 78             │
└  space mute   q quit   n hide-notch   i intensity ───────────────────────┘
Las barras ocupan una sola fila de alto al inicio (como tu imagen). Si el terminal es alto, crecen hasta 6 filas, mismo estilo “amp”.
Teclas:

q / Ctrl-C → shutdown limpio
espacio → mute (engine sigue en silencio, visualizer cae a cero)
n → destruye / recrea el notch
1..4 → intensidad 25/50/75/100

Al salir: disable_raw_mode + LeaveAlternateScreen sí o sí, también si el notch manda Quit.

9. Notch (lo que pediste)
No es el notch físico de una Mac. Es un HUD píldora, always-on-top, que se sienta sobre cualquier ventana. Ultra barato.
Geometría
textancho  280 px
alto    36 px
margen  10 px desde el borde superior del monitor que tiene el foco
centro  horizontal de ese monitor
Sin titlebar, sin resize, WindowLevel::AlwaysOnTop, decorations = false, fondo opaco #0B0C10. No transparencia real en v1 (la transparencia fuerza compositor/GPU y se siente en batería).
Hit targets
text[ • • • • • ]  [ barras 32 ] 
     ↑
  cluster 40×36
  clic = Quit
  hover = dots se prenden
Eso replica tu imagen: los 5 dots a la izquierda son el power.
También:

arrastrar desde las barras mueve el notch
doble clic en barras = mute
Esc con el notch focused = Quit

Render: winit + softbuffer. Un framebuffer 280*36*4 ≈ 40 KB. Cada frame:
textclear bg
draw 5 dots
for i in 0..32:
    h = snap.bars[i] * 28 / 255
    fill rect x=48+i*7, y=30-h, w=5, h=h, color=PALETTE[i%5]
present
12 fps, no 60. Si SpectrumSnap no cambió, no presentás. En idle el notch se duerme a 2 fps.
“En cualquier ventana”
v1: anclado al monitor del foco, no parented al HWND/NSWindow de Cursor (parenting es frágil y distinto en Win/Mac/Wayland).
v1.1 opcional: seguir la ventana focused y pegarse a su borde superior-centro (devtone --notch follow-focus). Lo dejamos especificado, no bloquea el MVP.
macOS extra (después): LSUIElement, panel nonactivating, collectionBehavior = canJoinAllSpaces. No activar la app al clicar mute.
Wayland: always-on-top es best-effort (layer-shell si está, si no ventana normal). Documentar.

10. Sensores (sin leer código)
Orden de implementación:
P0 — Claude Code

Tail de ~/.claude/projects/**/*.jsonl con notify. Parsear solo líneas type=assistant que traigan message.usage. Delta vs último uuid visto.
P0 — lenguaje

Linux: xprop / hyprctl / niri / sway (feature por compositor, fallback “unknown”).

macOS: AXFocusedUIElement / NSWorkspace frontmost + título.

Windows: GetForegroundWindow + GetWindowText.

Heurística: último token del título que parezca foo.rs, bar.ts, o el cwd del agente si el jsonl lo trae.
P1 — Codex, Copilot CLI, OpenCode

Mismo patrón, un adapter por path.
P2 — Cursor

Aprox: proceso Cursor vivo + mtime del workspace + (opcional) bytes/s hacia api2.cursor.sh sin payload. Nunca MITM.
Contrato del adapter:
Rusttrait AgentSource {
    fn name(&self) -> AgentKind;
    fn watch_paths(&self) -> Vec<PathBuf>;
    fn ingest_line(&mut self, path: &Path, line: &str) -> Option<TokenDelta>;
}
TokenDelta { out, inn, cache_read, cache_write, model, ts }.

11. CLI y ciclo de vida
textdevtone
devtone --headless
devtone --no-notch
devtone --intensity 0.7
devtone --agent claude
devtone stop
devtone status --json
Startup:

ping socket → si hay instancia, attach TUI o exit “already running”
bind socket
load ~/.config/devtone/config.toml (si no existe, defaults)
spawn engine, sensors, mapper
spawn notch (main)
spawn TUI si hay TTY y no --headless
block en event loop

Shutdown (cualquier camino: q, notch, devtone stop, SIGINT):

Command::Quit
fade audio 120 ms (evita click)
drop stream cpal
restore terminal
unlink socket
exit 0


12. Config
~/.config/devtone/config.toml
tomlintensity = 0.8
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
# nunca se guarda source ni títulos completos, solo lang + focus enum
Nada sale de la máquina.

13. Orden para codearlo (PRs mentales)

core + engine silencio + SpectrumSnap dummy que ya anime 32 barras con LFO. TUI sola. Confirmás look.
engine real con vinyl + kick + hat + pad. Sin sensores. Ya es usable como lofi.
notch winit/softbuffer + power dots + mismo SpectrumSnap.
orquestador + socket + fade + quit.
sensor Claude Code + mapper.
adapters extra y follow-focus.

El paso 1 es el que valida tu imagen. Si las barras no se sienten a amplificador, no sigas: ajustá peak-hold y paleta ahí.

14. Criterio de “ultra liviano” (gates)
Medir con devtone --headless 10 minutos en una laptop en batería:

RSS p50 < 35 MB
CPU proceso p95 < 2% de un core
wakeups/s < 80 (notch 12 fps + audio 86 callbacks/s)
0 alloc en el callback (check con dhat o count-alloc en CI)
binario release strip + lto=thin objetivo < 4 MB sin samples, < 6 MB con samples embebidos

Si el notch obliga a un compositor pesado en algún WM, --no-notch es first-class, no un afterthought.