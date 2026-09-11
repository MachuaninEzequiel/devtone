pub fn make_kick(sr: f32) -> Vec<f32> {
    let n = (sr * 0.22) as usize;
    let mut v = vec![0.0f32; n.max(64)];
    for (i, s) in v.iter_mut().enumerate() {
        let t = i as f32 / sr;
        let env = (-t * 18.0).exp();
        let hz = 120.0 * (-t * 12.0).exp() + 38.0;
        *s = (t * hz * 2.0 * std::f32::consts::PI).sin() * env;
    }
    v
}

pub fn make_hat(sr: f32) -> Vec<f32> {
    let n = (sr * 0.06) as usize;
    let mut v = vec![0.0f32; n.max(32)];
    let mut lcg = 0xC0FFEEu32;
    let mut hp = 0.0f32;
    for (i, s) in v.iter_mut().enumerate() {
        lcg = lcg.wrapping_mul(1664525).wrapping_add(1013904223);
        let nse = (lcg >> 16) as f32 / 32768.0 - 1.0;
        hp += 0.65 * (nse - hp);
        let t = i as f32 / sr;
        let env = (-t * 55.0).exp();
        *s = (nse - hp) * env * 0.9;
    }
    v
}

pub fn make_snare(sr: f32) -> Vec<f32> {
    let n = (sr * 0.16) as usize;
    let mut v = vec![0.0f32; n.max(32)];
    let mut lcg = 0xBADC0DEu32;
    for (i, s) in v.iter_mut().enumerate() {
        let t = i as f32 / sr;
        lcg = lcg.wrapping_mul(1664525).wrapping_add(1013904223);
        let nse = (lcg >> 16) as f32 / 32768.0 - 1.0;
        let tone = (t * 190.0 * 2.0 * std::f32::consts::PI).sin() * (-t * 22.0).exp();
        let env = (-t * 16.0).exp();
        *s = tone * 0.45 + nse * env * 0.55;
    }
    v
}
