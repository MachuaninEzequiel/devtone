use devtone_core::{degree_midi, Groove, MusicParams, SpectrumSnap};

use crate::drum::{make_hat, make_kick, make_snare};
use crate::spectrum::compute_spectrum;

const COMB_MS: [f32; 4] = [29.7, 37.1, 41.1, 43.7];
const AP_MS: [f32; 2] = [5.0, 1.7];

pub struct Engine {
    sample_rate: f32,
    mute: bool,
    params: MusicParams,
    intensity: f32,
    frames: u64,
    snap: SpectrumSnap,
    lcg: u32,
    vinyl_hp: f32,
    last_step: i64,
    kick: Vec<f32>,
    hat: Vec<f32>,
    snare: Vec<f32>,
    kick_pos: Option<f32>,
    hat_pos: Option<f32>,
    snare_pos: Option<f32>,
    bass_phase: f32,
    pad_phase: [f32; 2],
    pad_lp: f32,
    lead_phase: f32,
    lead_env: f32,
    lead_midi: f32,
    bass_midi: f32,
    comb: [Vec<f32>; 4],
    comb_i: [usize; 4],
    ap: [Vec<f32>; 2],
    ap_i: [usize; 2],
    fade_gain: f32,
    fade_step: f32,
    mix_hist: [f32; 256],
    mix_i: usize,
    blocks: u32,
}

impl Engine {
    pub fn new(sample_rate: u32) -> Self {
        let sr = sample_rate.max(8000) as f32;
        let comb = COMB_MS.map(|ms| vec![0.0f32; ((ms * 0.001 * sr) as usize).max(8)]);
        let ap = AP_MS.map(|ms| vec![0.0f32; ((ms * 0.001 * sr) as usize).max(4)]);
        Self {
            sample_rate: sr,
            mute: false,
            params: MusicParams::default(),
            intensity: 0.8,
            frames: 0,
            snap: SpectrumSnap::default(),
            lcg: 0xDEC0DE,
            vinyl_hp: 0.0,
            last_step: -1,
            kick: make_kick(sr),
            hat: make_hat(sr),
            snare: make_snare(sr),
            kick_pos: None,
            hat_pos: None,
            snare_pos: None,
            bass_phase: 0.0,
            pad_phase: [0.0, 0.0],
            pad_lp: 0.0,
            lead_phase: 0.0,
            lead_env: 0.0,
            lead_midi: 74.0,
            bass_midi: 50.0,
            comb,
            comb_i: [0; 4],
            ap,
            ap_i: [0; 2],
            fade_gain: 1.0,
            fade_step: 0.0,
            mix_hist: [0.0; 256],
            mix_i: 0,
            blocks: 0,
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate as u32
    }

    pub fn set_mute(&mut self, mute: bool) {
        self.mute = mute;
        if mute {
            self.fade_gain = 0.0;
            self.fade_step = 0.0;
        } else {
            self.fade_gain = 1.0;
        }
    }

    pub fn set_intensity(&mut self, intensity: f32) {
        self.intensity = intensity.clamp(0.0, 1.0);
    }

    pub fn set_params(&mut self, params: MusicParams) {
        self.params = params;
    }

    pub fn params(&self) -> MusicParams {
        self.params
    }

    pub fn fade_out(&mut self, frames: usize) {
        let n = frames.max(1) as f32;
        self.fade_gain = self.fade_gain.max(0.0001);
        self.fade_step = self.fade_gain / n;
        self.mute = false;
    }

    pub fn spectrum(&self) -> SpectrumSnap {
        self.snap
    }

    pub fn tick(&mut self) {
        // Musical phase is sample-accurate inside render.
    }

    pub fn render(&mut self, interleaved_i16: &mut [i16]) {
        if self.mute || self.fade_gain <= 0.0 && self.fade_step == 0.0 {
            interleaved_i16.fill(0);
            self.snap = SpectrumSnap::default();
            return;
        }

        let bpm = self.params.bpm.clamp(68.0, 92.0);
        let swing = self.params.swing.clamp(0.5, 0.66);
        let layers = self.params.layers;

        for pair in interleaved_i16.chunks_mut(2) {
            self.advance_sequencer(bpm, swing);

            let vinyl = self.vinyl_sample() * layers.vinyl;
            let drums = self.drum_sample() * 1.15;
            let bass = self.bass_sample() * layers.bass;
            let pad = self.pad_sample() * layers.pad;
            let lead = self.lead_sample() * layers.lead;
            let dry = (vinyl + drums + bass + pad + lead) * self.intensity;
            let wet = self.reverb_sample(dry) * self.params.reverb;
            let mut s = dry + wet;
            let spread = vinyl * 0.15;

            if self.fade_step > 0.0 {
                self.fade_gain -= self.fade_step;
                if self.fade_gain <= 0.0 {
                    self.fade_gain = 0.0;
                    self.fade_step = 0.0;
                    self.mute = true;
                }
            }
            s *= self.fade_gain;
            let spread = spread * self.fade_gain;
            s = s.clamp(-1.0, 1.0);

            let i16s = (s * 28000.0) as i16;
            if pair.len() == 2 {
                pair[0] = ((s + spread).clamp(-1.0, 1.0) * 28000.0) as i16;
                pair[1] = ((s - spread).clamp(-1.0, 1.0) * 28000.0) as i16;
            } else {
                pair[0] = i16s;
            }
            self.mix_hist[self.mix_i] = s;
            self.mix_i = (self.mix_i + 1) % 256;
            self.frames += 1;
        }

        self.blocks += 1;
        if self.blocks % 3 == 0 {
            let mut ordered = [0.0f32; 256];
            for i in 0..256 {
                ordered[i] = self.mix_hist[(self.mix_i + i) % 256];
            }
            self.snap = compute_spectrum(&ordered, &self.params.layers, &self.snap);
        }
    }

    fn advance_sequencer(&mut self, bpm: f32, swing: f32) {
        let beats = self.frames as f64 * bpm as f64 / (60.0 * self.sample_rate as f64);
        let raw_steps = beats * 4.0;
        let step = raw_steps.floor() as i64;
        if step == self.last_step {
            return;
        }
        self.last_step = step;
        let s = step.rem_euclid(16) as i32;
        if s % 2 == 1 {
            let frac = raw_steps - step as f64;
            let delay = (swing as f64 - 0.5) * 2.0;
            if frac < delay {
                self.last_step = step - 1;
                return;
            }
        }
        let bar = (step.div_euclid(16)).rem_euclid(8) as i32;
        let (kick_m, snare_m, hat_m) = groove_masks(self.params.groove);
        if bit(kick_m, s) {
            self.kick_pos = Some(0.0);
        }
        if bit(snare_m, s) || (bar == 7 && s >= 14 && self.params.layers.lead > 0.3) {
            self.snare_pos = Some(0.0);
        }
        if bit(hat_m, s) {
            self.hat_pos = Some(0.0);
        }
        let chord = [0usize, 2, 4, 0][(bar as usize) % 4];
        self.bass_midi = degree_midi(self.params.root_midi, self.params.scale, chord, 0);
        if self.params.layers.lead > 0.08 && matches!(s, 0 | 3 | 6 | 8 | 11 | 14) {
            let deg = (bar as usize * 2 + s as usize / 3) % 8;
            self.lead_midi = degree_midi(self.params.root_midi, self.params.scale, deg, 2);
            self.lead_env = 1.0;
        }
    }

    fn vinyl_sample(&mut self) -> f32 {
        self.lcg = self.lcg.wrapping_mul(1664525).wrapping_add(1013904223);
        let n = (self.lcg >> 16) as f32 / 32768.0 - 1.0;
        let crack = if (self.lcg & 0xFFFF) < (self.params.crackle * 400.0) as u32 {
            0.35
        } else {
            0.0
        };
        let x = n * 0.08 + crack;
        self.vinyl_hp += 0.04 * (x - self.vinyl_hp);
        x - self.vinyl_hp
    }

    fn drum_sample(&mut self) -> f32 {
        let kick = take_table(&mut self.kick_pos, &self.kick) * self.params.layers.kick;
        let hat = take_table(&mut self.hat_pos, &self.hat) * self.params.layers.hat;
        let snare = take_table(&mut self.snare_pos, &self.snare) * (self.params.layers.kick * 0.55);
        kick + hat + snare
    }

    fn bass_sample(&mut self) -> f32 {
        let hz = midi_hz(self.bass_midi);
        self.bass_phase += hz / self.sample_rate;
        self.bass_phase -= self.bass_phase.floor();
        let t = self.bass_phase * 2.0 * std::f32::consts::PI;
        (t.sin() + 0.35 * (2.0 * t).sin()) * 0.32
    }

    fn pad_sample(&mut self) -> f32 {
        let beats = self.frames as f32 * self.params.bpm / (60.0 * self.sample_rate);
        let lfo = 1.0 + 0.18 * (beats * std::f32::consts::PI / 8.0).sin();
        let midi_a = degree_midi(self.params.root_midi, self.params.scale, 0, 1);
        let midi_b = degree_midi(self.params.root_midi, self.params.scale, 2, 1);
        let hz = midi_hz(midi_a);
        let hz2 = midi_hz(midi_b) * 2f32.powf(7.0 / 1200.0);
        self.pad_phase[0] += hz / self.sample_rate;
        self.pad_phase[1] += hz2 / self.sample_rate;
        self.pad_phase[0] -= self.pad_phase[0].floor();
        self.pad_phase[1] -= self.pad_phase[1].floor();
        let saw = |p: f32| p * 2.0 - 1.0;
        let mixed = (saw(self.pad_phase[0]) + saw(self.pad_phase[1])) * 0.16;
        let tense = if self.params.tension > 0.05 {
            (beats * 2.2).sin() * self.params.tension * 0.08
        } else {
            0.0
        };
        let cutoff = (self.params.cutoff_hz * lfo).clamp(400.0, 4200.0);
        let coeff = 1.0 - (-2.0 * std::f32::consts::PI * cutoff / self.sample_rate).exp();
        self.pad_lp += coeff.clamp(0.001, 0.99) * (mixed + tense - self.pad_lp);
        self.pad_lp
    }

    fn lead_sample(&mut self) -> f32 {
        if self.lead_env <= 0.0001 {
            return 0.0;
        }
        self.lead_env *= 0.9994;
        let hz = midi_hz(self.lead_midi);
        self.lead_phase += hz / self.sample_rate;
        self.lead_phase -= self.lead_phase.floor();
        let t = self.lead_phase * 2.0 * std::f32::consts::PI;
        t.sin() * self.lead_env * 0.22
    }

    fn reverb_sample(&mut self, x: f32) -> f32 {
        let mut sum = 0.0;
        for i in 0..4 {
            let len = self.comb[i].len();
            let idx = self.comb_i[i];
            let y = self.comb[i][idx];
            self.comb[i][idx] = x + y * 0.82;
            self.comb_i[i] = (idx + 1) % len;
            sum += y;
        }
        sum *= 0.25;
        for i in 0..2 {
            let len = self.ap[i].len();
            let idx = self.ap_i[i];
            let buf = self.ap[i][idx];
            let y = -sum + buf;
            self.ap[i][idx] = sum + buf * 0.7;
            self.ap_i[i] = (idx + 1) % len;
            sum = y;
        }
        sum
    }
}

fn take_table(pos: &mut Option<f32>, table: &[f32]) -> f32 {
    let Some(p) = pos else { return 0.0 };
    let i = *p as usize;
    if i + 1 >= table.len() {
        *pos = None;
        return 0.0;
    }
    let frac = *p - i as f32;
    let s = table[i] * (1.0 - frac) + table[i + 1] * frac;
    *p += 1.0;
    s
}

fn midi_hz(midi: f32) -> f32 {
    440.0 * 2f32.powf((midi - 69.0) / 12.0)
}

fn bit(mask: u16, step: i32) -> bool {
    mask & (1 << (step.rem_euclid(16) as u16)) != 0
}

fn groove_masks(groove: Groove) -> (u16, u16, u16) {
    // bit0 = step 0. (kick, snare, hat)
    match groove {
        Groove::Tight => (0x0101, 0x1010, 0x5555),
        Groove::Warm => (0x0081, 0x0010, 0x1111),
        Groove::Busy => (0x0521, 0x2910, 0xFFFF),
        Groove::Dry => (0x0101, 0x1010, 0x1111),
        Groove::Sparse => (0x0001, 0x0100, 0x0001),
    }
}

#[cfg(test)]
mod tests {
    use super::Engine;
    use devtone_core::{LayerMix, MusicParams};

    #[test]
    fn silent_engine_writes_zeros() {
        let mut e = Engine::new(44100);
        e.set_mute(true);
        let mut buf = vec![1i16; 1024];
        e.render(&mut buf);
        assert!(buf.iter().all(|&x| x == 0));
    }

    #[test]
    fn unmuted_render_is_not_silence() {
        let mut e = Engine::new(44100);
        e.set_mute(false);
        e.set_params(MusicParams {
            bpm: 78.0,
            layers: LayerMix {
                vinyl: 0.4,
                kick: 0.8,
                hat: 0.6,
                bass: 0.5,
                pad: 0.4,
                lead: 0.0,
            },
            ..Default::default()
        });
        let mut buf = vec![0i16; 44100 * 2];
        e.render(&mut buf);
        let peak = buf.iter().map(|x| x.abs()).max().unwrap();
        assert!(peak > 100, "expected audible energy, peak={peak}");
    }

    #[test]
    fn fade_out_reaches_near_zero() {
        let mut e = Engine::new(44100);
        e.set_params(MusicParams {
            layers: LayerMix {
                vinyl: 0.5,
                kick: 0.7,
                hat: 0.5,
                bass: 0.4,
                pad: 0.3,
                lead: 0.0,
            },
            ..Default::default()
        });
        let mut warm = vec![0i16; 2048];
        e.render(&mut warm);
        assert!(warm.iter().map(|x| x.abs()).max().unwrap() > 100);
        e.fade_out(44100 * 120 / 1000);
        let mut cold = vec![0i16; 44100 * 2 / 5];
        e.render(&mut cold);
        let tail = &cold[cold.len().saturating_sub(4096)..];
        let peak = tail.iter().map(|x| x.abs()).max().unwrap();
        assert!(peak < 50, "fade leftover peak={peak}");
    }
}
