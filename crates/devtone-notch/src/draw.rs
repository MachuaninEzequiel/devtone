use devtone_core::{palette, SpectrumSnap};

pub const NOTCH_W: u32 = 280;
pub const NOTCH_H: u32 = 36;

pub fn hit_quit(x: f64, y: f64) -> bool {
    x >= 0.0 && x < 40.0 && y >= 0.0 && y < f64::from(NOTCH_H)
}

pub fn hit_bars(x: f64, y: f64) -> bool {
    x >= 40.0 && x < f64::from(NOTCH_W) && y >= 0.0 && y < f64::from(NOTCH_H)
}

pub fn draw_notch(buf: &mut [u32], snap: &SpectrumSnap, dots_hover: bool) {
    let w = NOTCH_W as usize;
    let h = NOTCH_H as usize;
    if buf.len() < w * h {
        return;
    }
    for px in buf.iter_mut().take(w * h) {
        *px = palette::BG;
    }
    let dot_color = if dots_hover { palette::DOT_ON } else { palette::DOT };
    for i in 0..5 {
        let cx = 8 + i * 7;
        let cy = 18;
        fill_circle(buf, w, h, cx, cy, 2, dot_color);
    }
    for i in 0..32 {
        let bh = (snap.bars[i] as u32 * 28 / 255).max(if snap.bars[i] > 0 { 1 } else { 0 });
        if bh == 0 {
            continue;
        }
        let x = 48 + i as u32 * 7;
        let y = 30u32.saturating_sub(bh);
        fill_rect(buf, w, h, x, y, 5, bh, palette::bar_color(i));
    }
}

fn fill_rect(buf: &mut [u32], w: usize, h: usize, x: u32, y: u32, rw: u32, rh: u32, color: u32) {
    for yy in y..y.saturating_add(rh).min(h as u32) {
        for xx in x..x.saturating_add(rw).min(w as u32) {
            buf[yy as usize * w + xx as usize] = color;
        }
    }
}

fn fill_circle(buf: &mut [u32], w: usize, h: usize, cx: i32, cy: i32, r: i32, color: u32) {
    for yy in (cy - r)..=(cy + r) {
        for xx in (cx - r)..=(cx + r) {
            if xx < 0 || yy < 0 || xx >= w as i32 || yy >= h as i32 {
                continue;
            }
            let dx = xx - cx;
            let dy = yy - cy;
            if dx * dx + dy * dy <= r * r {
                buf[yy as usize * w + xx as usize] = color;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{draw_notch, hit_quit, NOTCH_H, NOTCH_W};
    use devtone_core::{palette, SpectrumSnap};

    #[test]
    fn draw_fills_background_and_bars() {
        let mut buf = vec![0u32; (NOTCH_W * NOTCH_H) as usize];
        let mut snap = SpectrumSnap::default();
        snap.bars[0] = 255;
        draw_notch(&mut buf, &snap, false);
        assert_eq!(buf[0], palette::BG);
        let bar_x = 48;
        let bar_y = 30 - 28;
        let idx = (bar_y * NOTCH_W + bar_x) as usize;
        assert_eq!(buf[idx], palette::BARS[0]);
    }

    #[test]
    fn quit_hit_target_is_left_cluster() {
        assert!(hit_quit(10.0, 18.0));
        assert!(!hit_quit(100.0, 18.0));
    }
}
