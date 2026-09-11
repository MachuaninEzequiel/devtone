use devtone_core::SpectrumSnap;
use std::cell::Cell;

pub fn peak_hold(prev: u8, new: u8) -> u8 {
    if new > prev {
        new
    } else {
        ((prev as f32) * 0.86) as u8
    }
}

thread_local! {
    static PREV: Cell<SpectrumSnap> = const { Cell::new(SpectrumSnap { bars: [0; 32], peak: 0, rms: 0 }) };
}

pub fn dummy_spectrum(t_ms: u64) -> SpectrumSnap {
    let prev = PREV.with(|p| p.get());
    let mut bars = [0u8; 32];
    let mut peak = 0u8;
    let mut acc = 0u32;
    for i in 0..32 {
        let phase = t_ms as f32 * 0.01 + i as f32 * 0.4;
        let raw = ((phase.sin() * 0.5 + 0.5) * 255.0) as u8;
        let v = peak_hold(prev.bars[i], raw);
        bars[i] = v;
        peak = peak.max(v);
        acc += v as u32;
    }
    let snap = SpectrumSnap {
        bars,
        peak,
        rms: (acc / 32) as u8,
    };
    PREV.with(|p| p.set(snap));
    snap
}

#[cfg(test)]
mod tests {
    use super::{dummy_spectrum, peak_hold};

    #[test]
    fn peak_hold_decays_by_86_percent() {
        assert_eq!(peak_hold(100, 50), 86);
        assert_eq!(peak_hold(100, 120), 120);
    }

    #[test]
    fn dummy_spectrum_is_32_bars_not_all_equal() {
        let a = dummy_spectrum(0);
        let b = dummy_spectrum(400);
        assert_eq!(a.bars.len(), 32);
        assert_ne!(a.bars, b.bars);
    }
}
