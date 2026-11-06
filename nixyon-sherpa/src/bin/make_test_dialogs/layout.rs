/// Клип для раскладки: длина и границы речи внутри него, в сэмплах.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClipShape {
    pub speaker: usize,
    pub len: usize,
    pub speech: (usize, usize),
}

/// Как ставить клипы друг за другом.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Joining {
    /// Клип за клипом с паузой между концами клипов (тишина клипов остаётся).
    Natural,
    /// При смене голоса речь следующего начинается сразу после конца речи предыдущего.
    NoPause,
    /// При смене голоса речь следующего начинается раньше конца речи предыдущего
    /// на очередное значение из списка, но не больше половины более короткой речи.
    Overlap(&'static [f64]),
}

/// Начало каждого клипа в записи и её длина, в сэмплах.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub offsets: Vec<usize>,
    pub total: usize,
}

/// Раскладка клипов. Паузы (`pauses`, секунды) идут по кругу; при одном голосе подряд
/// всегда пауза. Ограничение на половину речи гарантирует, что одновременно говорят
/// не больше двух.
pub fn layout(
    clips: &[ClipShape],
    joining: Joining,
    rate: u32,
    lead_sec: f64,
    pauses: &[f64],
) -> Result<Layout, String> {
    let samples = |sec: f64| (sec * f64::from(rate)).round() as i64;
    let mut offsets: Vec<usize> = Vec::with_capacity(clips.len());
    let mut changes = 0;
    for (i, clip) in clips.iter().enumerate() {
        let offset = match (i, offsets.last()) {
            (0, _) | (_, None) => samples(lead_sec),
            (_, Some(&prev_offset)) => {
                // Пауза после клипа i - 1, как в прежней сборке записей.
                let pause = samples(pauses[(i - 1) % pauses.len()]);
                let prev = clips[i - 1];
                let prev_offset = to_i64(prev_offset);
                let prev_speech_end = prev_offset + to_i64(prev.speech.1);
                let speech_start = to_i64(clip.speech.0);
                let changed = prev.speaker != clip.speaker;
                match joining {
                    Joining::Natural => prev_offset + to_i64(prev.len) + pause,
                    _ if !changed => prev_speech_end + pause - speech_start,
                    Joining::NoPause => prev_speech_end - speech_start,
                    Joining::Overlap(amounts) => {
                        let wanted = samples(amounts[changes % amounts.len()]);
                        changes += 1;
                        let half_shorter = to_i64(speech_len(&prev).min(speech_len(clip))) / 2;
                        prev_speech_end - wanted.min(half_shorter) - speech_start
                    }
                }
            }
        };
        offsets.push(
            usize::try_from(offset).map_err(|_| format!("клип {i} уходит до начала записи"))?,
        );
    }
    let total = match (clips.last(), offsets.last()) {
        (Some(last), Some(&offset)) => {
            // Естественная запись кончается паузой, как раньше; синтетика тишиной вступления.
            let tail = match joining {
                Joining::Natural => samples(pauses[(clips.len() - 1) % pauses.len()]),
                _ => samples(lead_sec),
            };
            offset + last.len + usize::try_from(tail).unwrap_or(0)
        }
        _ => 0,
    };
    Ok(Layout { offsets, total })
}

fn speech_len(clip: &ClipShape) -> usize {
    clip.speech.1 - clip.speech.0
}

// Длины клипов в сэмплах далеко от i64::MAX.
fn to_i64(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 10;

    fn clip(speaker: usize, len: usize, speech: (usize, usize)) -> ClipShape {
        ClipShape {
            speaker,
            len,
            speech,
        }
    }

    fn clips() -> Vec<ClipShape> {
        vec![
            clip(1, 50, (5, 45)),
            clip(2, 60, (10, 50)),
            clip(2, 30, (2, 28)),
        ]
    }

    #[test]
    fn natural_puts_pause_between_clip_ends() {
        let l = layout(&clips(), Joining::Natural, RATE, 1.0, &[0.5, 1.0]).unwrap();
        assert_eq!(l.offsets, [10, 65, 135]);
        assert_eq!(l.total, 135 + 30 + 5);
    }

    #[test]
    fn no_pause_starts_next_voice_right_where_previous_speech_ends() {
        let c = clips();
        let l = layout(&c, Joining::NoPause, RATE, 1.0, &[0.5, 1.0]).unwrap();
        assert_eq!(l.offsets[1] + c[1].speech.0, l.offsets[0] + c[0].speech.1);
        // Тот же голос подряд: обычная пауза между речью.
        assert_eq!(
            l.offsets[2] + c[2].speech.0,
            l.offsets[1] + c[1].speech.1 + 10
        );
    }

    #[test]
    fn overlap_is_limited_by_half_of_the_shorter_speech() {
        let c = vec![
            clip(1, 50, (5, 45)),
            clip(2, 30, (0, 10)),
            clip(1, 50, (0, 40)),
        ];
        let l = layout(&c, Joining::Overlap(&[3.0, 0.4]), RATE, 1.0, &[0.5]).unwrap();
        let end0 = l.offsets[0] + c[0].speech.1;
        assert_eq!(l.offsets[1] + c[1].speech.0, end0 - 5);
        let end1 = l.offsets[1] + c[1].speech.1;
        assert_eq!(l.offsets[2] + c[2].speech.0, end1 - 4);
        // Третий начинается после конца первого: втроём не говорят.
        assert!(l.offsets[2] + c[2].speech.0 >= end0);
    }

    #[test]
    fn clip_that_would_start_before_zero_is_an_error() {
        let c = vec![clip(1, 10, (0, 4)), clip(2, 100, (90, 100))];
        assert!(layout(&c, Joining::NoPause, RATE, 0.0, &[0.5]).is_err());
    }
}
