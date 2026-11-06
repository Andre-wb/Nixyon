/// Предел пика после сведения, как у выравнивания громкости в nixyon-audio.
const MAX_PEAK: f32 = 0.99;

/// Складывает клипы в одну запись. Если наложения дали пик выше 0,99 и выше пика
/// самих клипов, вся запись ослабляется одинаково, чтобы не было клиппинга. Записи
/// без наложений остаются байт в байт как клипы, даже если клип уже упирается в 1,0.
pub fn mix(clips: &[&[f32]], offsets: &[usize], total: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; total];
    for (clip, &offset) in clips.iter().zip(offsets) {
        for (dst, &src) in out[offset..offset + clip.len()].iter_mut().zip(clip.iter()) {
            *dst += src;
        }
    }
    let limit = clips.iter().map(|c| peak(c)).fold(MAX_PEAK, f32::max);
    let mixed_peak = peak(&out);
    if mixed_peak > limit {
        let gain = limit / mixed_peak;
        out.iter_mut().for_each(|x| *x *= gain);
    }
    out
}

fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0f32, |m, x| m.max(x.abs()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clips_land_at_offsets_without_changes() {
        let a = [0.1, 0.2];
        let b = [0.3];
        assert_eq!(mix(&[&a, &b], &[1, 4], 6), [0.0, 0.1, 0.2, 0.0, 0.3, 0.0]);
    }

    #[test]
    fn clip_already_at_full_scale_is_not_touched_without_overlap() {
        let a = [1.0, -1.0];
        assert_eq!(mix(&[&a], &[0], 3), [1.0, -1.0, 0.0]);
    }

    #[test]
    fn loud_overlap_is_scaled_down_for_the_whole_record() {
        let a = [0.9, 0.9];
        let b = [0.9];
        let out = mix(&[&a, &b], &[0, 1], 2);
        assert!((out[1] - 0.99).abs() < 1e-6);
        assert!((out[0] - 0.9 * 0.99 / 1.8).abs() < 1e-6);
    }
}
