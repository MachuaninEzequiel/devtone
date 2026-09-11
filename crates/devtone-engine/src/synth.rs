use devtone_core::{MusicParams, SpectrumSnap};

use crate::spectrum::dummy_spectrum;

pub struct Engine {
    sample_rate: u32,
    mute: bool,
    params: MusicParams,
    frames: u64,
    snap: SpectrumSnap,
}

impl Engine {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            mute: false,
            params: MusicParams::default(),
            frames: 0,
            snap: SpectrumSnap::default(),
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn set_mute(&mut self, mute: bool) {
        self.mute = mute;
    }

    pub fn set_params(&mut self, params: MusicParams) {
        self.params = params;
    }

    pub fn params(&self) -> MusicParams {
        self.params
    }

    pub fn fade_out(&mut self, _frames: usize) {
        self.mute = true;
    }

    pub fn spectrum(&self) -> SpectrumSnap {
        self.snap
    }

    pub fn render(&mut self, interleaved_i16: &mut [i16]) {
        if self.mute {
            interleaved_i16.fill(0);
            self.snap = SpectrumSnap::default();
            return;
        }
        interleaved_i16.fill(0);
        self.frames += (interleaved_i16.len() / 2) as u64;
        let t_ms = self.frames * 1000 / self.sample_rate.max(1) as u64;
        self.snap = dummy_spectrum(t_ms);
    }
}

#[cfg(test)]
mod tests {
    use super::Engine;

    #[test]
    fn silent_engine_writes_zeros() {
        let mut e = Engine::new(44100);
        e.set_mute(true);
        let mut buf = vec![1i16; 1024];
        e.render(&mut buf);
        assert!(buf.iter().all(|&x| x == 0));
    }
}
