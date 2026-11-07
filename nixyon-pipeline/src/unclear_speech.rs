use nixyon_core::{SegmentResult, Turn, UNCLEAR_FLAG, VoiceSpan};

use crate::neural_labels::{MIN_SPAN_COVERAGE, coverage};

/// Кусок VAD без распознанных букв остаётся в расшифровке неразборчивой речью, если он
/// не короче этого и нейросегментация слышит в нём голос (покрывает хотя бы
/// `MIN_SPAN_COVERAGE`). Короче бывают кашель и щелчки. По замеру на тех же записях, что
/// для `SPEAKER_UNSURE_COSINE`, все такие реплики (42 с CAM++, 59 с ReDimNet2-B6) пришлись
/// на речь по эталону.
pub const UNCLEAR_MIN_SEC: f64 = 1.0;
/// Ход с текстом помечается неразборчивым, если букв на секунду голоса в нём меньше этого
/// числа. По замеру русская речь синтеза даёт от 8,4 буквы на секунду голоса, речь на
/// языке, которого распознаватель не знает, чаще всего меньше 3. В русских диалогах
/// пометки не получил ни один ход, на встречах на английском 14 из 15 (CAM++) и 6 из 7
/// (ReDimNet2-B6) помеченных пришлись на речь.
pub const MIN_LETTERS_PER_VOICE_SEC: f64 = 3.0;
/// ...и голоса в нём не меньше столько секунд: короткое «да» звучит дольше, чем пишется.
pub const SPARSE_MIN_VOICE_SEC: f64 = 2.0;

/// Кусок без букв, в котором нейросегментация слышит голос: он идёт в расшифровку
/// неразборчивой речью, а не пропадает.
pub(crate) fn unclear_segment(s: &SegmentResult, spans: &[VoiceSpan]) -> bool {
    !has_letters(&s.text)
        && s.end - s.start >= UNCLEAR_MIN_SEC
        && coverage(s.start, s.end, spans) >= MIN_SPAN_COVERAGE
}

/// Помечает неразборчивыми ходы, где букв мало для времени голоса в них. Ходы из кусков
/// без букв помечены уже при сборке.
pub(crate) fn mark_unclear(turns: &mut [Turn], spans: &[VoiceSpan]) {
    for t in turns.iter_mut() {
        if !t.is_unclear() && sparse(t, spans) {
            t.flags.push(UNCLEAR_FLAG.into());
        }
    }
}

// Реплика из одних знаков («.») сюда не попадает: её присоединяет к соседней
// `fix_split_sentences`.
fn sparse(t: &Turn, spans: &[VoiceSpan]) -> bool {
    let letters = t.text.chars().filter(|c| c.is_alphanumeric()).count();
    let voice = coverage(t.start, t.end, spans) * (t.end - t.start);
    let letters = f64::from(u32::try_from(letters).unwrap_or(u32::MAX));
    letters > 0.0 && voice >= SPARSE_MIN_VOICE_SEC && letters / voice < MIN_LETTERS_PER_VOICE_SEC
}

pub(crate) fn has_letters(text: &str) -> bool {
    text.chars().any(char::is_alphanumeric)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{segment, transcript, voice};

    fn span(start: f64, end: f64) -> VoiceSpan {
        VoiceSpan {
            start,
            end,
            embedding: voice(1, 0.0),
        }
    }

    #[test]
    fn segment_without_letters_stays_only_when_long_and_voiced() {
        let spans = vec![span(0.0, 2.0), span(5.0, 5.4)];
        let silent = |start, end, text: &str| segment(start, end, text, voice(1, 0.0));
        assert!(unclear_segment(&silent(0.0, 2.0, ""), &spans));
        assert!(unclear_segment(&silent(0.0, 2.0, " … "), &spans));
        assert!(!unclear_segment(&silent(0.0, 2.0, "да"), &spans));
        // Короткий звук и звук, где сегментация голоса не слышит.
        assert!(!unclear_segment(&silent(0.0, 0.9, ""), &spans));
        assert!(!unclear_segment(&silent(5.0, 6.5, ""), &spans));
    }

    #[test]
    fn turn_is_unclear_when_letters_are_few_for_the_voice_in_it() {
        let spans = vec![span(0.0, 10.0)];
        let mut t = transcript(&[
            (1, 0.0, 3.0, "Угу"),
            (1, 3.0, 4.5, "да"),
            (2, 4.5, 7.0, "Давайте начнём совещание."),
            (2, 7.0, 9.0, "."),
        ])
        .turns;
        mark_unclear(&mut t, &spans);
        let unclear: Vec<bool> = t.iter().map(Turn::is_unclear).collect();
        assert_eq!(unclear, [true, false, false, false]);
        // Без голоса под ходом мало букв не значит, что речь пропала.
        let mut quiet = transcript(&[(1, 20.0, 25.0, "Угу")]).turns;
        mark_unclear(&mut quiet, &spans);
        assert!(!quiet[0].is_unclear());
    }

    #[test]
    fn flag_is_added_once() {
        let mut t = transcript(&[(1, 0.0, 3.0, "Угу")]).turns;
        t[0].flags.push(UNCLEAR_FLAG.into());
        mark_unclear(&mut t, &[span(0.0, 3.0)]);
        assert_eq!(t[0].flags, [UNCLEAR_FLAG]);
    }
}
