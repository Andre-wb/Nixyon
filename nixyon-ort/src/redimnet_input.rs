use crate::repeat_pad::repeat_pad;

/// Вход выгрузки soniqo: ровно 6 с звука 16 кГц.
pub const FIXED_SAMPLES: usize = 96_000;
/// Кусок короче идёт в модель с динамическим входом повторённым до этой длины. Граф
/// считается от 0,05 с, но по замеру на клипах повтор коротких кусков до 0,5 с качество
/// не меняет, а запас от нижней границы графа остаётся.
pub const DYNAMIC_MIN_SAMPLES: usize = 8_000;

/// Длина входа модели голоса ReDimNet2: постоянная 6 с или любая.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedimnetInput {
    Fixed,
    Dynamic,
}

impl RedimnetInput {
    /// Режим по длине оси времени у входа модели: 96000 или символьная (отрицательная).
    pub fn from_dim(dim: i64) -> Option<Self> {
        if dim < 0 {
            Some(Self::Dynamic)
        } else if usize::try_from(dim).ok() == Some(FIXED_SAMPLES) {
            Some(Self::Fixed)
        } else {
            None
        }
    }

    /// Вход модели из куска не длиннее 6 с: при постоянном входе повтор до 6 с, при
    /// динамическом кусок как есть, а слишком короткий повторён до наименьшей длины.
    pub fn piece(self, samples: &[f32]) -> Vec<f32> {
        match self {
            Self::Fixed => repeat_pad(samples, FIXED_SAMPLES),
            Self::Dynamic if samples.len() < DYNAMIC_MIN_SAMPLES => {
                repeat_pad(samples, DYNAMIC_MIN_SAMPLES)
            }
            Self::Dynamic => samples.to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbolic_time_axis_means_dynamic_input_and_six_seconds_means_fixed() {
        assert_eq!(RedimnetInput::from_dim(-1), Some(RedimnetInput::Dynamic));
        assert_eq!(RedimnetInput::from_dim(96_000), Some(RedimnetInput::Fixed));
        assert_eq!(RedimnetInput::from_dim(48_000), None);
    }

    #[test]
    fn fixed_input_repeats_a_short_piece_to_six_seconds() {
        let piece = RedimnetInput::Fixed.piece(&[1.0; 16_000]);
        assert_eq!(piece.len(), FIXED_SAMPLES);
    }

    #[test]
    fn dynamic_input_keeps_a_piece_of_half_a_second_and_longer_as_is() {
        let samples: Vec<f32> = (0..24_000).map(|i| i as f32).collect();
        assert_eq!(RedimnetInput::Dynamic.piece(&samples), samples);
        assert_eq!(
            RedimnetInput::Dynamic
                .piece(&[0.5; DYNAMIC_MIN_SAMPLES])
                .len(),
            DYNAMIC_MIN_SAMPLES
        );
    }

    #[test]
    fn dynamic_input_repeats_a_very_short_piece_to_half_a_second() {
        let piece = RedimnetInput::Dynamic.piece(&[1.0, 2.0, 3.0]);
        assert_eq!(piece.len(), DYNAMIC_MIN_SAMPLES);
        assert_eq!(piece[..4], [1.0, 2.0, 3.0, 1.0]);
    }
}
