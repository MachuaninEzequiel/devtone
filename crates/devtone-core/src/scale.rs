use crate::Scale;

pub fn scale_intervals(scale: Scale) -> &'static [i8] {
    match scale {
        Scale::MinorPentatonic => &[0, 3, 5, 7, 10],
        Scale::Dorian => &[0, 2, 3, 5, 7, 9, 10],
        Scale::MajorPent => &[0, 2, 4, 7, 9],
    }
}

pub fn degree_midi(root: u8, scale: Scale, degree: usize, octave_offset: i8) -> f32 {
    let iv = scale_intervals(scale);
    let n = iv.len().max(1);
    let oct = (degree / n) as i8 + octave_offset;
    let step = iv[degree % n];
    (root as i16 + step as i16 + oct as i16 * 12) as f32
}

#[cfg(test)]
mod tests {
    use super::degree_midi;
    use crate::Scale;

    #[test]
    fn dorian_degree_zero_is_root() {
        assert!((degree_midi(50, Scale::Dorian, 0, 0) - 50.0).abs() < 0.01);
    }

    #[test]
    fn major_pent_second_degree_is_a_tone() {
        assert!((degree_midi(60, Scale::MajorPent, 1, 0) - 62.0).abs() < 0.01);
    }
}
