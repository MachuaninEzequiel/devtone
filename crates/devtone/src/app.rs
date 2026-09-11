use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use arc_swap::ArcSwap;
use crossbeam_channel::{bounded, Sender};
use devtone_core::{Command, Mapper, MusicParams, SpectrumSnap, StateFrame};
use devtone_engine::Engine;

use crate::cli::{Cli, CommandMode};
use crate::config::Config;
use crate::ipc_server::{ping, send_request, socket_path, IpcServer};

pub struct App;

impl App {
    pub fn run(cli: Cli, cfg: Config) -> ExitCode {
        match cli.command {
            CommandMode::Stop => return client_stop(),
            CommandMode::Status { json } => return client_status(json),
            CommandMode::Run => {}
        }

        let sock = socket_path();
        if ping(&sock) {
            if cli.headless {
                eprintln!("devtone: already running");
                return ExitCode::from(1);
            }
            eprintln!("devtone: already running (attach TUI not yet in this build)");
            return ExitCode::from(1);
        }

        let server = match IpcServer::bind(&sock) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("devtone: socket: {e}");
                return ExitCode::from(1);
            }
        };

        let running = Arc::new(AtomicBool::new(true));
        let muted = Arc::new(AtomicBool::new(false));
        let (tx, rx) = bounded::<Command>(32);
        let state = Arc::new(ArcSwap::from_pointee(StateFrame::default()));
        let params = Arc::new(ArcSwap::from_pointee(MusicParams::default()));
        let snap = Arc::new(ArcSwap::from_pointee(SpectrumSnap::default()));

        let r2 = running.clone();
        let _ = ctrlc::set_handler(move || {
            r2.store(false, Ordering::SeqCst);
        });

        let audio = spawn_audio(cfg.clone(), cli.intensity.unwrap_or(cfg.intensity), params.clone(), snap.clone(), muted.clone(), running.clone());
        let mapper = spawn_mapper(state.clone(), params.clone(), running.clone());
        let sensors = spawn_sensors(cli.agent, cfg.clone(), state.clone(), running.clone());

        #[cfg(feature = "tui")]
        let tui_join = {
            use std::io::IsTerminal;
            if !cli.headless && std::io::stdout().is_terminal() {
                Some(spawn_tui(state.clone(), params.clone(), snap.clone(), muted.clone(), tx.clone(), running.clone(), cfg.fps_tui))
            } else {
                None
            }
        };
        #[cfg(not(feature = "tui"))]
        let tui_join = None::<thread::JoinHandle<()>>;

        let notch_wanted = cfg.notch && !cli.no_notch && !cli.headless;

        if notch_wanted {
            #[cfg(feature = "notch")]
            {
                let snap_n = snap.clone();
                let tx_n = tx.clone();
                let running_n = running.clone();
                let muted_n = muted.clone();
                let state_n = state.clone();
                let tx_i = tx.clone();
                let handle = thread::spawn(move || {
                    ipc_loop(server, tx_i, rx, state_n, muted_n, running_n);
                });
                if let Err(e) = devtone_notch::run_notch(snap_n, tx_n, running.clone()) {
                    eprintln!("devtone: notch disabled: {e}");
                }
                running.store(false, Ordering::SeqCst);
                let _ = handle.join();
            }
            #[cfg(not(feature = "notch"))]
            {
                ipc_loop(server, tx, rx, state.clone(), muted.clone(), running.clone());
            }
        } else {
            ipc_loop(server, tx, rx, state.clone(), muted.clone(), running.clone());
        }

        thread::sleep(Duration::from_millis(150));
        drop(audio);
        drop(mapper);
        drop(sensors);
        drop(tui_join);
        let _ = sock;
        ExitCode::SUCCESS
    }
}

fn ipc_loop(
    server: IpcServer,
    tx: Sender<Command>,
    rx: crossbeam_channel::Receiver<Command>,
    state: Arc<ArcSwap<StateFrame>>,
    muted: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
) {
    while running.load(Ordering::SeqCst) {
        let frame = **state.load();
        let _ = server.poll(&tx, frame);
        while let Ok(cmd) = rx.try_recv() {
            match cmd {
                Command::Quit => running.store(false, Ordering::SeqCst),
                Command::Mute => {
                    let next = !muted.load(Ordering::SeqCst);
                    muted.store(next, Ordering::SeqCst);
                }
                Command::SetIntensity(_) => {}
                Command::ToggleNotch => {}
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn client_stop() -> ExitCode {
    match send_request(&socket_path(), &devtone_core::IpcRequest::Stop) {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("devtone: stop: {e}");
            ExitCode::from(1)
        }
    }
}

fn client_status(json: bool) -> ExitCode {
    match send_request(&socket_path(), &devtone_core::IpcRequest::Status) {
        Ok(devtone_core::IpcResponse::State(st)) => {
            if json {
                match serde_json::to_string_pretty(&st) {
                    Ok(s) => println!("{s}"),
                    Err(e) => {
                        eprintln!("{e}");
                        return ExitCode::from(1);
                    }
                }
            } else {
                println!(
                    "agent={} model={} tps={:.1} lang={} flow={:.2} streaming={}",
                    st.agent.as_str(),
                    st.model.as_str(),
                    st.out_tps,
                    st.lang.as_str(),
                    st.flow,
                    st.agent_streaming
                );
            }
            ExitCode::SUCCESS
        }
        Ok(other) => {
            eprintln!("devtone: unexpected {other:?}");
            ExitCode::from(1)
        }
        Err(e) => {
            eprintln!("devtone: status: {e}");
            ExitCode::from(1)
        }
    }
}

fn spawn_audio(
    cfg: Config,
    intensity: f32,
    params: Arc<ArcSwap<MusicParams>>,
    snap: Arc<ArcSwap<SpectrumSnap>>,
    muted: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
) -> thread::JoinHandle<()> {
    thread::Builder::new()
        .name("devtone-audio".into())
        .spawn(move || {
            let mut engine = Engine::new(cfg.sample_rate);
            engine.set_intensity(intensity);
            if let Some(_stream) = start_cpal(
                &cfg,
                intensity,
                params.clone(),
                snap.clone(),
                muted.clone(),
                running.clone(),
            ) {
                while running.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_millis(20));
                }
                return;
            }
            let frames = cfg.buffer.max(64) as usize;
            let mut buf = vec![0i16; frames * 2];
            let budget = Duration::from_secs_f64(frames as f64 / cfg.sample_rate.max(1) as f64);
            while running.load(Ordering::SeqCst) {
                let t0 = Instant::now();
                engine.set_params(**params.load());
                engine.set_mute(muted.load(Ordering::SeqCst));
                engine.render(&mut buf);
                snap.store(Arc::new(engine.spectrum()));
                if let Some(sleep) = budget.checked_sub(t0.elapsed()) {
                    thread::sleep(sleep);
                }
            }
            engine.fade_out((cfg.sample_rate as usize) * 120 / 1000);
            engine.render(&mut buf);
        })
        .expect("audio thread")
}

fn start_cpal(
    cfg: &Config,
    intensity: f32,
    params: Arc<ArcSwap<MusicParams>>,
    snap: Arc<ArcSwap<SpectrumSnap>>,
    muted: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
) -> Option<cpal::Stream> {
    if std::env::var_os("DEVTONE_NO_CPAL").is_some() {
        return None;
    }
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    let host = cpal::default_host();
    let device = host.default_output_device()?;
    let mut config = device.default_output_config().ok()?.config();
    config.sample_rate = cpal::SampleRate(cfg.sample_rate);
    config.buffer_size = cpal::BufferSize::Fixed(cfg.buffer);
    let mut engine = Engine::new(cfg.sample_rate);
    engine.set_intensity(intensity);
    let mut i16buf = vec![0i16; 8192];
    let stream = device
        .build_output_stream(
            &config,
            move |data: &mut [f32], _| {
                if !running.load(Ordering::Relaxed) {
                    data.fill(0.0);
                    return;
                }
                engine.set_params(**params.load());
                engine.set_mute(muted.load(Ordering::Relaxed));
                let n = data.len().min(i16buf.len());
                engine.render(&mut i16buf[..n]);
                for (o, s) in data.iter_mut().zip(&i16buf[..n]) {
                    *o = *s as f32 / 32768.0;
                }
                snap.store(Arc::new(engine.spectrum()));
            },
            |err| eprintln!("devtone: audio: {err}"),
            None,
        )
        .ok()?;
    stream.play().ok()?;
    Some(stream)
}

fn spawn_sensors(
    override_agent: Option<devtone_core::AgentKind>,
    cfg: Config,
    state: Arc<ArcSwap<StateFrame>>,
    running: Arc<AtomicBool>,
) -> thread::JoinHandle<()> {
    thread::Builder::new()
        .name("devtone-sensors".into())
        .spawn(move || {
            let mut sup = devtone_sensors::Supervisor::new(
                override_agent,
                cfg.watch_agent_logs,
                cfg.watch_active_window,
            );
            while running.load(Ordering::SeqCst) {
                let frame = sup.tick();
                state.store(Arc::new(frame));
                thread::sleep(Duration::from_millis(250));
            }
        })
        .expect("sensor thread")
}

fn spawn_mapper(
    state: Arc<ArcSwap<StateFrame>>,
    params: Arc<ArcSwap<MusicParams>>,
    running: Arc<AtomicBool>,
) -> thread::JoinHandle<()> {
    thread::Builder::new()
        .name("devtone-mapper".into())
        .spawn(move || {
            let mut mapper = Mapper::new();
            while running.load(Ordering::SeqCst) {
                let frame = **state.load();
                let p = mapper.tick(&frame);
                params.store(Arc::new(p));
                thread::sleep(Duration::from_millis(250));
            }
        })
        .expect("mapper thread")
}

#[cfg(feature = "tui")]
fn spawn_tui(
    state: Arc<ArcSwap<StateFrame>>,
    params: Arc<ArcSwap<MusicParams>>,
    snap: Arc<ArcSwap<SpectrumSnap>>,
    muted: Arc<AtomicBool>,
    tx: Sender<Command>,
    running: Arc<AtomicBool>,
    fps: u32,
) -> thread::JoinHandle<()> {
    thread::Builder::new()
        .name("devtone-tui".into())
        .spawn(move || {
            use crossterm::event::{self, Event, KeyCode, KeyModifiers};
            use crossterm::execute;
            use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
            use ratatui::backend::CrosstermBackend;
            use ratatui::Terminal;
            use std::io::stdout;

            if enable_raw_mode().is_err() {
                return;
            }
            let mut out = stdout();
            let _ = execute!(out, EnterAlternateScreen);
            let backend = CrosstermBackend::new(out);
            let mut terminal = match Terminal::new(backend) {
                Ok(t) => t,
                Err(_) => {
                    let _ = disable_raw_mode();
                    return;
                }
            };
            let frame_dt = Duration::from_millis((1000 / fps.max(1)) as u64);
            while running.load(Ordering::SeqCst) {
                let t0 = Instant::now();
                let st = **state.load();
                let p = **params.load();
                let sp = **snap.load();
                let _ = devtone_tui::render(&mut terminal, &st, &p, &sp, muted.load(Ordering::Relaxed));
                while event::poll(Duration::from_millis(0)).unwrap_or(false) {
                    if let Ok(Event::Key(k)) = event::read() {
                        match k.code {
                            KeyCode::Char('q') => {
                                let _ = tx.send(Command::Quit);
                            }
                            KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                                let _ = tx.send(Command::Quit);
                            }
                            KeyCode::Char(' ') => {
                                let _ = tx.send(Command::Mute);
                            }
                            KeyCode::Char('n') => {
                                let _ = tx.send(Command::ToggleNotch);
                            }
                            KeyCode::Char('1') => {
                                let _ = tx.send(Command::SetIntensity(0.25));
                            }
                            KeyCode::Char('2') => {
                                let _ = tx.send(Command::SetIntensity(0.5));
                            }
                            KeyCode::Char('3') => {
                                let _ = tx.send(Command::SetIntensity(0.75));
                            }
                            KeyCode::Char('4') => {
                                let _ = tx.send(Command::SetIntensity(1.0));
                            }
                            _ => {}
                        }
                    }
                }
                if let Some(sleep) = frame_dt.checked_sub(t0.elapsed()) {
                    thread::sleep(sleep);
                }
            }
            let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
            let _ = disable_raw_mode();
        })
        .expect("tui thread")
}

#[cfg(test)]
mod tests {
    use super::App;
    use crate::cli::parse_cli;
    use crate::config::Config;
    use crate::ipc_server::send_request;
    use devtone_core::{AgentKind, IpcRequest, IpcResponse};
    use std::process::ExitCode;
    use std::sync::Mutex;

    static ENV: Mutex<()> = Mutex::new(());

    fn lock_env() -> std::sync::MutexGuard<'static, ()> {
        ENV.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn isolate_roots(tmp: &std::path::Path) {
        std::env::set_var("DEVTONE_PI_ROOT", tmp.join("pi"));
        std::env::set_var("DEVTONE_CLAUDE_ROOT", tmp.join("claude"));
        std::env::set_var("DEVTONE_CODEX_ROOT", tmp.join("codex"));
        std::env::set_var("DEVTONE_OPENCODE_DB", tmp.join("opencode.db"));
        let _ = std::fs::create_dir_all(tmp.join("pi"));
    }

    #[test]
    fn headless_no_notch_starts_and_stop_exits() {
        let _g = lock_env();
        let dir = tempfile::tempdir().unwrap();
        isolate_roots(dir.path());
        let path = dir.path().join("devtone.sock");
        std::env::set_var("DEVTONE_SOCK", &path);
        std::env::set_var("DEVTONE_NO_CPAL", "1");
        let handle = std::thread::spawn(|| {
            let cli = parse_cli(["devtone", "--headless", "--no-notch"]);
            App::run(cli, Config::default())
        });
        let mut ok = false;
        for _ in 0..50 {
            std::thread::sleep(std::time::Duration::from_millis(40));
            if let Ok(IpcResponse::State(_)) = send_request(&path, &IpcRequest::Status) {
                ok = true;
                break;
            }
        }
        assert!(ok, "status did not answer on {}", path.display());
        let _ = send_request(&path, &IpcRequest::Stop);
        let code = handle.join().unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn appending_pi_jsonl_updates_status_agent() {
        let _g = lock_env();
        let tmp = tempfile::tempdir().unwrap();
        isolate_roots(tmp.path());
        let sock = tmp.path().join("devtone.sock");
        let pi_root = tmp.path().join("pi");
        let jsonl = pi_root.join("s.jsonl");
        std::fs::write(&jsonl, "").unwrap();
        std::env::set_var("DEVTONE_SOCK", &sock);
        std::env::set_var("DEVTONE_NO_CPAL", "1");
        let mut cfg = Config::default();
        cfg.watch_active_window = false;
        let handle = std::thread::spawn(move || {
            App::run(
                parse_cli(["devtone", "--headless", "--no-notch"]),
                cfg,
            )
        });
        std::thread::sleep(std::time::Duration::from_millis(300));
        std::fs::write(
            &jsonl,
            "{\"type\":\"message\",\"message\":{\"role\":\"assistant\",\"model\":\"sonnet\",\"usage\":{\"input\":1,\"output\":20,\"cacheRead\":0,\"cacheWrite\":0}}}\n",
        )
        .unwrap();
        let mut agent = AgentKind::None;
        let mut tps = 0.0f32;
        for _ in 0..20 {
            std::thread::sleep(std::time::Duration::from_millis(80));
            if let Ok(IpcResponse::State(st)) = send_request(&sock, &IpcRequest::Status) {
                agent = st.agent;
                tps = st.out_tps;
                if agent == AgentKind::Pi && tps > 0.0 {
                    break;
                }
            }
        }
        let _ = send_request(&sock, &IpcRequest::Stop);
        let _ = handle.join();
        assert_eq!(agent, AgentKind::Pi);
        assert!(tps > 0.0, "out_tps={tps}");
    }
}
