use crate::{Focus, Lang, LayerMix, MusicParams, Scale, StateFrame};

pub const ALPHA_SLOW: f32 = 0.08;
pub const ALPHA_MID: f32 = 0.18;
pub const ALPHA_FAST: f32 = 0.35;

pub fn ema(x: f32, target: f32, alpha: f32) -> f32 {
    x + alpha * (target - x)
}

fn clamp(x: f32, lo: f32, hi: f32) -> f32 {
    x.max(lo).min(hi)
}

#[derive(Clone, Debug)]
pub struct Mapper {
    params: MusicParams,
    last_active_ms: u64,
    saw_tick: bool,
}

impl Default for Mapper {
    fn default() -> Self {
        Self::new()
    }
}

impl Mapper {
    pub fn new() -> Self {
        Self {
            params: MusicParams::default(),
            last_active_ms: 0,
            saw_tick: false,
        }
    }

    pub fn tick(&mut self, state: &StateFrame) -> MusicParams {
        if state.flow > 0.15 || state.agent_streaming {
            self.last_active_ms = state.ts_ms;
        } else if !self.saw_tick {
            self.last_active_ms = state.ts_ms;
        }
        self.saw_tick = true;

        let idle = state.ts_ms.saturating_sub(self.last_active_ms) >= 20_000
            && !state.agent_streaming
            && state.flow <= 0.15;

        let (scale, root, mut cutoff, mut reverb, mut pad, mut hat) = lang_targets(state.lang);

        if state.agent_streaming {
            pad += 0.35;
            cutoff += 800.0;
        }
        if state.out_tps > 12.0 {
            hat += 0.20;
        } else if state.out_tps > 4.0 {
            hat += 0.10;
        }
        if state.cache_read_delta > 2_000 {
            reverb += 0.12;
            pad += 0.10;
        }

        let mut kick = 0.45;
        if state.tools_per_min > 8.0 {
            kick += 0.20;
        }

        let mut bpm = 78.0;
        let mut crackle = 0.04;
        let mut tension = 0.0;
        let mut lead = if state.agent_streaming { 0.22 } else { 0.0 };
        let vinyl = 0.35;
        let bass = 0.35;

        if matches!(state.focus, Focus::Terminal | Focus::AgentCli) && state.stress > 0.4 {
            tension += state.stress;
            crackle += 0.08;
        }

        if idle {
            hat = 0.05;
            kick = 0.15;
            bpm -= 6.0;
            lead = 0.0;
            pad *= 0.7;
        }

        pad = clamp(pad, 0.0, 1.0);
        hat = clamp(hat, 0.0, 1.0);
        kick = clamp(kick, 0.0, 1.0);
        reverb = clamp(reverb, 0.0, 0.45);
        cutoff = clamp(cutoff, 400.0, 4200.0);
        bpm = clamp(bpm, 68.0, 92.0);
        tension = clamp(tension, 0.0, 1.0);
        crackle = clamp(crackle, 0.0, 0.2);
        lead = clamp(lead, 0.0, 1.0);

        let p = &mut self.params;
        p.scale = scale;
        p.root_midi = ema(p.root_midi as f32, root as f32, ALPHA_SLOW).round() as u8;
        p.bpm = ema(p.bpm, bpm, ALPHA_SLOW);
        p.reverb = ema(p.reverb, reverb, ALPHA_SLOW);
        p.layers.hat = ema(p.layers.hat, hat, ALPHA_MID);
        p.cutoff_hz = ema(p.cutoff_hz, cutoff, ALPHA_MID);
        let pad_alpha = if state.agent_streaming { ALPHA_FAST } else { ALPHA_MID };
        p.layers.pad = ema(p.layers.pad, pad, pad_alpha);
        p.layers.kick = ema(p.layers.kick, kick, ALPHA_MID);
        p.layers.vinyl = ema(p.layers.vinyl, vinyl, ALPHA_SLOW);
        p.layers.bass = ema(p.layers.bass, bass, ALPHA_MID);
        p.layers.lead = ema(p.layers.lead, lead, ALPHA_FAST);
        p.crackle = ema(p.crackle, crackle, ALPHA_MID);
        p.tension = ema(p.tension, tension, ALPHA_MID);
        p.swing = ema(p.swing, 0.55, ALPHA_SLOW);

        p.bpm = clamp(p.bpm, 68.0, 92.0);
        p.layers = LayerMix {
            vinyl: clamp(p.layers.vinyl, 0.0, 1.0),
            kick: clamp(p.layers.kick, 0.0, 1.0),
            hat: clamp(p.layers.hat, 0.0, 1.0),
            bass: clamp(p.layers.bass, 0.0, 1.0),
            pad: clamp(p.layers.pad, 0.0, 1.0),
            lead: clamp(p.layers.lead, 0.0, 1.0),
        };
        p.reverb = clamp(p.reverb, 0.0, 0.45);
        *p
    }
}

fn lang_targets(lang: Lang) -> (Scale, u8, f32, f32, f32, f32) {
    // scale, root_midi, cutoff, reverb, pad, hat
    match lang {
        Lang::Rs => (Scale::Dorian, 50, 1400.0, 0.16, 0.28, 0.38),
        Lang::Py => (Scale::MinorPentatonic, 48, 1900.0, 0.22, 0.42, 0.36),
        Lang::Ts => (Scale::MajorPent, 53, 2200.0, 0.18, 0.32, 0.50),
        Lang::Go => (Scale::Dorian, 52, 1600.0, 0.08, 0.26, 0.40),
        Lang::Sql => (Scale::MinorPentatonic, 48, 1200.0, 0.20, 0.30, 0.30),
        Lang::Other => (Scale::Dorian, 50, 1800.0, 0.18, 0.30, 0.40),
    }
}

#[cfg(test)]
mod tests {
    use super::{ema, Mapper};
    use crate::{Lang, Scale, StateFrame};

    #[test]
    fn ema_moves_fraction_toward_target() {
        let y = ema(0.0, 1.0, 0.08);
        assert!((y - 0.08).abs() < 1e-6);
    }

    #[test]
    fn rust_lang_targets_dorian_d2() {
        let mut m = Mapper::new();
        let mut s = StateFrame::default();
        s.lang = Lang::Rs;
        s.ts_ms = 1;
        let p = m.tick(&s);
        assert_eq!(p.scale, Scale::Dorian);
        assert_eq!(p.root_midi, 50);
    }

    #[test]
    fn idle_20s_drops_hats_and_bpm() {
        let mut m = Mapper::new();
        let mut s = StateFrame {
            flow: 0.8,
            agent_streaming: true,
            ts_ms: 0,
            ..Default::default()
        };
        let hot = m.tick(&s);
        s.flow = 0.0;
        s.agent_streaming = false;
        s.ts_ms = 21_000;
        let cold = m.tick(&s);
        assert!(cold.layers.hat < hot.layers.hat);
        assert!(cold.bpm < hot.bpm);
    }
}
