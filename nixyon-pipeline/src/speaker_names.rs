use nixyon_core::{Speaker, Turn};

const MAX_NAME_PART_CHARS: usize = 30;
const MAX_NAME_PARTS: usize = 3;

/// Слова с заглавной буквы, которые после «меня зовут» именем не бывают.
const NOT_NAMES: [&str; 11] = [
    "спикер",
    "speaker",
    "никак",
    "так",
    "как",
    "тоже",
    "меня",
    "вас",
    "вы",
    "не",
    "null",
];

pub(crate) fn is_dash(w: &str) -> bool {
    !w.is_empty() && w.chars().all(|c| matches!(c, '—' | '–' | '-' | ':'))
}

/// Имя из строки: от одного до трёх слов, каждое с заглавной буквы, только буквы и дефис.
/// Знаки по краям слов отбрасываются. Иначе None.
pub fn clean_name(raw: &str) -> Option<String> {
    let parts: Vec<&str> = raw.split_whitespace().collect();
    if parts.is_empty() || parts.len() > MAX_NAME_PARTS {
        return None;
    }
    let mut cleaned = Vec::with_capacity(parts.len());
    for part in parts {
        let part = part.trim_matches(|c: char| !c.is_alphabetic());
        let ok = (2..=MAX_NAME_PART_CHARS).contains(&part.chars().count())
            && part.chars().all(|c| c.is_alphabetic() || c == '-')
            && part.chars().next().is_some_and(char::is_uppercase);
        if !ok {
            return None;
        }
        cleaned.push(part);
    }
    let name = cleaned.join(" ");
    if NOT_NAMES.contains(&name.to_lowercase().as_str()) {
        return None;
    }
    Some(name)
}

/// Имена по самопредставлению: «меня зовут Игорь», «зовут меня Анна», «моё имя Борис».
/// Берётся первое представление каждого спикера. Итог: (номер спикера, имя).
pub fn intro_names(turns: &[Turn]) -> Vec<(u32, String)> {
    let mut found: Vec<(u32, String)> = Vec::new();
    for turn in turns {
        if found.iter().any(|(speaker, _)| *speaker == turn.speaker) {
            continue;
        }
        if let Some(name) = intro_in(&turn.text) {
            found.push((turn.speaker, name));
        }
    }
    found
}

fn intro_in(text: &str) -> Option<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let lower: Vec<String> = words
        .iter()
        .map(|w| w.trim_matches(|c: char| !c.is_alphabetic()).to_lowercase())
        .collect();
    for i in 0..lower.len() {
        let next = lower.get(i + 1).map(String::as_str);
        let skip = match (lower[i].as_str(), next) {
            ("меня", Some("зовут")) | ("зовут", Some("меня")) | ("моё" | "мое", Some("имя")) => {
                2
            }
            _ => continue,
        };
        let name = words[i + skip..]
            .iter()
            .find(|w| !is_dash(w))
            .and_then(|w| clean_name(w));
        if name.is_some() {
            return name;
        }
    }
    None
}

/// Ставит имена только спикерам без имени и только такие, которых ещё нет у других:
/// имя по образцу голоса и ручное переименование не перезаписываются.
/// Возвращает, сколько имён поставлено.
pub fn apply_names(speakers: &mut [Speaker], names: Vec<(u32, String)>) -> usize {
    let mut applied = 0;
    for (id, name) in names {
        let lower = name.to_lowercase();
        let taken = speakers
            .iter()
            .any(|s| s.name.as_deref().is_some_and(|n| n.to_lowercase() == lower));
        if taken {
            continue;
        }
        if let Some(speaker) = speakers.iter_mut().find(|s| s.id == id && s.name.is_none()) {
            speaker.name = Some(name);
            applied += 1;
        }
    }
    applied
}

/// Есть ли имя в тексте разговора: слово с заглавной буквы с тем же началом (имя может
/// стоять в другом падеже: «Игорь» и «Игоря»). Защита от выдумок языковой модели.
pub fn name_is_grounded(name: &str, turns: &[Turn]) -> bool {
    let Some(first) = name.split_whitespace().next() else {
        return false;
    };
    let n = first.chars().count();
    let keep = match n {
        0..=3 => n,
        4 => 3,
        _ => n - 2,
    };
    let stem: String = first.to_lowercase().chars().take(keep).collect();
    turns.iter().any(|t| {
        t.text.split_whitespace().any(|w| {
            let w = w.trim_matches(|c: char| !c.is_alphabetic());
            w.chars().next().is_some_and(char::is_uppercase)
                && w.to_lowercase().starts_with(&stem)
                && w.chars().count() <= n + 3
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::transcript;

    fn speaker(id: u32, name: Option<&str>) -> Speaker {
        Speaker {
            id,
            name: name.map(String::from),
            talk_time_sec: 0.0,
            turns: 0,
            words: 0,
            age: None,
            gender: None,
        }
    }

    #[test]
    fn dialog_from_the_screenshot_gives_both_names() {
        let t = transcript(&[
            (1, 0.0, 1.0, "— Здравствуйте! —"),
            (2, 1.0, 2.0, "Добрый день."),
            (1, 2.0, 3.0, "— Как вас зовут? —"),
            (2, 3.0, 4.0, "Меня зовут Игорь. — А как вас зовут?"),
            (1, 4.0, 6.0, "— Меня зовут Анна. — Вы говорите по-русски? —"),
        ]);
        assert_eq!(
            intro_names(&t.turns),
            [(2, "Игорь".to_string()), (1, "Анна".to_string())]
        );
    }

    #[test]
    fn other_phrasings_and_dashes_work() {
        for (text, expected) in [
            ("Моё имя — Борис.", "Борис"),
            ("мое имя Вера", "Вера"),
            ("Зовут меня Анна-Мария", "Анна-Мария"),
            ("Здравствуйте, меня зовут Олег, рад знакомству", "Олег"),
        ] {
            let t = transcript(&[(1, 0.0, 1.0, text)]);
            assert_eq!(intro_names(&t.turns), [(1, expected.to_string())], "{text}");
        }
    }

    #[test]
    fn questions_lowercase_and_non_names_give_nothing() {
        for text in [
            "Как вас зовут?",
            "Меня зовут игорь",
            "Меня зовут",
            "Меня зовут так",
            "Я Игорь",
        ] {
            let t = transcript(&[(1, 0.0, 1.0, text)]);
            assert!(intro_names(&t.turns).is_empty(), "{text}");
        }
    }

    #[test]
    fn first_introduction_of_a_speaker_wins() {
        let t = transcript(&[
            (1, 0.0, 1.0, "Меня зовут Анна."),
            (1, 1.0, 2.0, "Меня зовут Вера."),
        ]);
        assert_eq!(intro_names(&t.turns), [(1, "Анна".to_string())]);
    }

    #[test]
    fn clean_name_accepts_names_and_rejects_the_rest() {
        assert_eq!(clean_name("Игорь,").as_deref(), Some("Игорь"));
        assert_eq!(
            clean_name("Анна Петровна").as_deref(),
            Some("Анна Петровна")
        );
        for bad in ["", "Я", "игорь", "Иг1орь", "Вас", "Спикер", "А Б В Г"]
        {
            assert_eq!(clean_name(bad), None, "{bad}");
        }
    }

    #[test]
    fn apply_names_fills_only_free_unnamed_speakers() {
        let mut speakers = vec![
            speaker(1, Some("Борис")),
            speaker(2, None),
            speaker(3, None),
        ];
        let applied = apply_names(
            &mut speakers,
            vec![
                (1, "Анна".into()),  // у спикера уже есть имя
                (2, "борис".into()), // имя уже занято, регистр не важен
                (3, "Вера".into()),
                (9, "Олег".into()), // такого спикера нет
            ],
        );
        assert_eq!(applied, 1);
        let names: Vec<Option<&str>> = speakers.iter().map(|s| s.name.as_deref()).collect();
        assert_eq!(names, [Some("Борис"), None, Some("Вера")]);
    }

    #[test]
    fn grounded_name_must_appear_in_the_text_in_any_case_form() {
        let t = transcript(&[
            (1, 0.0, 1.0, "Игорь, посмотри. Анне пора."),
            (2, 1.0, 2.0, "Хорошо"),
        ]);
        assert!(name_is_grounded("Игорь", &t.turns));
        assert!(name_is_grounded("Анна", &t.turns));
        assert!(!name_is_grounded("Борис", &t.turns));
    }
}
