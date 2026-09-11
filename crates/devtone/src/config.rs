use serde::Deserialize;

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub intensity: f32,
    pub notch: bool,
    pub notch_follow_focus: bool,
    pub fps_tui: u32,
    pub fps_notch: u32,
    pub sample_rate: u32,
    pub buffer: u32,
    pub watch_agent_logs: bool,
    pub watch_active_window: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            intensity: 0.8,
            notch: false,
            notch_follow_focus: false,
            fps_tui: 20,
            fps_notch: 12,
            sample_rate: 44100,
            buffer: 512,
            watch_agent_logs: true,
            watch_active_window: true,
        }
    }
}

#[derive(Deserialize, Default)]
struct FileConfig {
    intensity: Option<f32>,
    notch: Option<bool>,
    notch_follow_focus: Option<bool>,
    fps_tui: Option<u32>,
    fps_notch: Option<u32>,
    engine: Option<EngineFile>,
    privacy: Option<PrivacyFile>,
}

#[derive(Deserialize, Default)]
struct EngineFile {
    sample_rate: Option<u32>,
    buffer: Option<u32>,
}

#[derive(Deserialize, Default)]
struct PrivacyFile {
    watch_agent_logs: Option<bool>,
    watch_active_window: Option<bool>,
}

impl Config {
    pub fn load_or_default(text: Option<&str>) -> Self {
        let mut cfg = Self::default();
        let Some(text) = text else {
            return cfg;
        };
        let Ok(file) = toml::from_str::<FileConfig>(text) else {
            return cfg;
        };
        if let Some(v) = file.intensity {
            cfg.intensity = v;
        }
        if let Some(v) = file.notch {
            cfg.notch = v;
        }
        if let Some(v) = file.notch_follow_focus {
            cfg.notch_follow_focus = v;
        }
        if let Some(v) = file.fps_tui {
            cfg.fps_tui = v;
        }
        if let Some(v) = file.fps_notch {
            cfg.fps_notch = v;
        }
        if let Some(e) = file.engine {
            if let Some(v) = e.sample_rate {
                cfg.sample_rate = v;
            }
            if let Some(v) = e.buffer {
                cfg.buffer = v;
            }
        }
        if let Some(p) = file.privacy {
            if let Some(v) = p.watch_agent_logs {
                cfg.watch_agent_logs = v;
            }
            if let Some(v) = p.watch_active_window {
                cfg.watch_active_window = v;
            }
        }
        cfg
    }

    pub fn load_path(path: &std::path::Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::load_or_default(Some(&text)),
            Err(_) => Self::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn broken_toml_falls_back_to_defaults() {
        let cfg = Config::load_or_default(Some("intensity = oops"));
        assert!((cfg.intensity - 0.8).abs() < f32::EPSILON);
    }
}
