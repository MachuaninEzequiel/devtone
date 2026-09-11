#![cfg(feature = "window")]

use std::num::NonZeroU32;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use arc_swap::ArcSwap;
use crossbeam_channel::Sender;
use devtone_core::{Command, SpectrumSnap};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{CursorIcon, Window, WindowButtons, WindowId, WindowLevel};

use crate::draw::{draw_notch, hit_bars, hit_quit, NOTCH_H, NOTCH_W};

pub fn run_notch(
    snap: Arc<ArcSwap<SpectrumSnap>>,
    tx: Sender<Command>,
    running: Arc<AtomicBool>,
) -> Result<(), String> {
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    let mut app = NotchApp {
        snap,
        tx,
        running,
        window: None,
        surface: None,
        dots_hover: false,
        last_present: Instant::now(),
        last_bars: [0; 32],
        cursor: (0.0, 0.0),
        dragging: false,
        last_click: None,
    };
    event_loop.run_app(&mut app).map_err(|e| e.to_string())
}

struct NotchApp {
    snap: Arc<ArcSwap<SpectrumSnap>>,
    tx: Sender<Command>,
    running: Arc<AtomicBool>,
    window: Option<Arc<Window>>,
    surface: Option<softbuffer::Surface<Arc<Window>, Arc<Window>>>,
    dots_hover: bool,
    last_present: Instant,
    last_bars: [u8; 32],
    cursor: (f64, f64),
    dragging: bool,
    last_click: Option<Instant>,
}

impl ApplicationHandler for NotchApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let size = LogicalSize::new(NOTCH_W, NOTCH_H);
        let attrs = Window::default_attributes()
            .with_title("devtone")
            .with_decorations(false)
            .with_resizable(false)
            .with_enabled_buttons(WindowButtons::empty())
            .with_inner_size(size)
            .with_min_inner_size(size)
            .with_max_inner_size(size)
            .with_window_level(WindowLevel::AlwaysOnTop);
        match event_loop.create_window(attrs) {
            Ok(window) => {
                window.set_resizable(false);
                window.set_min_inner_size(Some(size));
                window.set_max_inner_size(Some(size));
                window.set_cursor(CursorIcon::Default);
                let window = Arc::new(window);
                match softbuffer::Context::new(window.clone()) {
                    Ok(ctx) => match softbuffer::Surface::new(&ctx, window.clone()) {
                        Ok(surface) => {
                            self.surface = Some(surface);
                            self.window = Some(window);
                        }
                        Err(e) => {
                            eprintln!("devtone: notch surface: {e}");
                            event_loop.exit();
                        }
                    },
                    Err(e) => {
                        eprintln!("devtone: notch context: {e}");
                        event_loop.exit();
                    }
                }
            }
            Err(e) => {
                eprintln!("devtone: notch window: {e}");
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                let _ = self.tx.send(Command::Quit);
                event_loop.exit();
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x, position.y);
                self.dots_hover = hit_quit(position.x, position.y);
                if let Some(w) = &self.window {
                    w.set_cursor(CursorIcon::Default);
                    w.request_redraw();
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let (x, y) = self.cursor;
                if hit_quit(x, y) {
                    let _ = self.tx.send(Command::Quit);
                    event_loop.exit();
                    return;
                }
                if hit_bars(x, y) {
                    let now = Instant::now();
                    if let Some(prev) = self.last_click {
                        if now.duration_since(prev) < Duration::from_millis(350) {
                            let _ = self.tx.send(Command::Mute);
                            self.last_click = None;
                            return;
                        }
                    }
                    self.last_click = Some(now);
                    if let Some(w) = &self.window {
                        let _ = w.drag_window();
                    }
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                self.dragging = false;
                if let Some(w) = &self.window {
                    w.set_cursor(CursorIcon::Default);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.logical_key == Key::Named(NamedKey::Escape) && event.state.is_pressed() {
                    let _ = self.tx.send(Command::Quit);
                    event_loop.exit();
                }
            }
            WindowEvent::RedrawRequested => self.paint(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if !self.running.load(Ordering::SeqCst) {
            event_loop.exit();
            return;
        }
        let snap = **self.snap.load();
        let changed = snap.bars != self.last_bars;
        let idle = snap.peak < 8;
        let min_dt = if idle && !changed {
            Duration::from_millis(500)
        } else {
            Duration::from_millis(83)
        };
        if self.last_present.elapsed() >= min_dt {
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + min_dt));
    }
}

impl NotchApp {
    fn paint(&mut self) {
        let Some(window) = self.window.clone() else {
            return;
        };
        let Some(surface) = self.surface.as_mut() else {
            return;
        };
        let w = NonZeroU32::new(NOTCH_W).unwrap();
        let h = NonZeroU32::new(NOTCH_H).unwrap();
        if surface.resize(w, h).is_err() {
            return;
        }
        let Ok(mut buf) = surface.buffer_mut() else {
            return;
        };
        let snap = **self.snap.load();
        draw_notch(buf.as_mut(), &snap, self.dots_hover);
        self.last_bars = snap.bars;
        let _ = buf.present();
        self.last_present = Instant::now();
        let _ = window;
    }
}
