//! Amp palette from the v1 spec. Values are 0x00RRGGBB.

pub const BG: u32 = 0x000B0C10;
pub const BARS: [u32; 5] = [0x00B7A9F5, 0x00F3C56B, 0x00E8A0B4, 0x00A8E0C8, 0x00C9B8F0];
pub const DOT: u32 = 0x003A3D46;
pub const DOT_ON: u32 = 0x00E8E6EF;

pub fn bar_color(i: usize) -> u32 {
    BARS[i % BARS.len()]
}

pub fn rgb(color: u32) -> (u8, u8, u8) {
    let r = ((color >> 16) & 0xFF) as u8;
    let g = ((color >> 8) & 0xFF) as u8;
    let b = (color & 0xFF) as u8;
    (r, g, b)
}
