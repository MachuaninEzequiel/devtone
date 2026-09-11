use devtone_core::{LayerMix, SpectrumSnap};
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

const SR: f32 = 44100.0;
const N: usize = 256;

pub fn compute_spectrum(mix: &[f32], layers: &LayerMix, prev: &SpectrumSnap) -> SpectrumSnap {
    let mut buf = [0.0f32; N];
    let n = mix.len().min(N);
    for i in 0..n {
        let hann = 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / N as f32).cos());
        buf[i] = mix[i] * hann;
    }
    let spec = microfft::real::rfft_256(&mut buf);
    let mut mag = [0.0f32; 32];
    let f_min = 40.0f32;
    let f_max = 8000.0f32;
    let ratio = (f_max / f_min).powf(1.0 / 32.0);
    for band in 0..32 {
        let lo = f_min * ratio.powi(band as i32);
        let hi = f_min * ratio.powi(band as i32 + 1);
        let mut peak = 0.0f32;
        for (k, c) in spec.iter().enumerate() {
            let f = k as f32 * SR / N as f32;
            if f >= lo && f < hi {
                let m = (c.re * c.re + c.im * c.im).sqrt();
                if m > peak {
                    peak = m;
                }
            }
        }
        mag[band] = peak;
    }
    let max_mag = mag.iter().copied().fold(1e-6f32, f32::max);
    let mut bars = [0u8; 32];
    let mut peak = 0u8;
    let mut acc = 0u32;
    for i in 0..32 {
        let fft_u = ((mag[i] / max_mag).sqrt() * 255.0) as u8;
        let proxy = layer_proxy(i, layers);
        let new = fft_u.max(proxy);
        let v = peak_hold(prev.bars[i], new);
        bars[i] = v;
        peak = peak.max(v);
        acc += v as u32;
    }
    SpectrumSnap {
        bars,
        peak,
        rms: (acc / 32) as u8,
    }
}

fn layer_proxy(i: usize, layers: &LayerMix) -> u8 {
    let mut g = 0.0f32;
    if i <= 3 {
        g = g.max(layers.kick);
    }
    if (2..=8).contains(&i) {
        g = g.max(layers.bass);
    }
    if (8..=20).contains(&i) {
        g = g.max(layers.pad);
    }
    if i >= 20 {
        g = g.max(layers.hat);
    }
    (g.clamp(0.0, 1.0) * 200.0) as u8
}

#[cfg(test)]
mod tests {
    use super::{compute_spectrum, dummy_spectrum, peak_hold};
    use devtone_core::{LayerMix, SpectrumSnap};

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

    #[test]
    fn kick_energy_lifts_low_bars() {
        let mut mix = [0.0f32; 256];
        for (i, s) in mix.iter_mut().enumerate() {
            *s = (i as f32 * 2.0 * std::f32::consts::PI * 60.0 / 44100.0).sin();
        }
        let layers = LayerMix {
            kick: 1.0,
            ..Default::default()
        };
        let snap = compute_spectrum(&mix, &layers, &SpectrumSnap::default());
        let low: u32 = snap.bars[0..4].iter().map(|&x| x as u32).sum();
        let high: u32 = snap.bars[24..32].iter().map(|&x| x as u32).sum();
        assert!(low > high, "low={low} high={high}");
    }
}
