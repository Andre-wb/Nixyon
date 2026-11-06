/// Частицы, которые одни эталоны пишут через дефис («что-то»), другие слитно («чтото»),
/// третьи отдельным словом («что то»).
const PARTICLES: &[&str] = &["нибудь", "либо", "таки", "то", "ка"];

/// Слова, после которых частица отделяется. Только они: иначе «это» и «лето» разошлись
/// бы на «э то» и «ле то».
const BASES: &[&str] = &[
    "что",
    "чего",
    "чему",
    "чем",
    "кто",
    "кого",
    "кому",
    "кем",
    "ком",
    "какой",
    "какая",
    "какое",
    "какие",
    "какого",
    "какому",
    "каким",
    "каком",
    "какую",
    "каких",
    "какими",
    "как",
    "где",
    "куда",
    "откуда",
    "когда",
    "почему",
    "зачем",
    "сколько",
    "чей",
    "чья",
    "чье",
    "чьи",
    "чьего",
    "чьей",
    "все",
    "ну",
    "давай",
    "так",
    "такой",
    "такая",
    "такое",
    "такие",
];

/// Слово, разделённое на основу и частицу: «чтото» и «ктонибудь» становятся «что то» и
/// «кто нибудь»; прочие слова остаются как есть.
pub(crate) fn split_particle(word: &str) -> Vec<String> {
    PARTICLES
        .iter()
        .find_map(|particle| {
            let base = word.strip_suffix(particle)?;
            BASES
                .contains(&base)
                .then(|| vec![base.to_owned(), (*particle).to_owned()])
        })
        .unwrap_or_else(|| vec![word.to_owned()])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_particle_after_a_known_base_becomes_its_own_word() {
        assert_eq!(split_particle("чтото"), ["что", "то"]);
        assert_eq!(split_particle("какието"), ["какие", "то"]);
        assert_eq!(split_particle("ктонибудь"), ["кто", "нибудь"]);
        assert_eq!(split_particle("какойлибо"), ["какой", "либо"]);
        assert_eq!(split_particle("всетаки"), ["все", "таки"]);
        assert_eq!(split_particle("нука"), ["ну", "ка"]);
    }

    #[test]
    fn other_words_ending_like_a_particle_stay_whole() {
        assert_eq!(split_particle("это"), ["это"]);
        assert_eq!(split_particle("лето"), ["лето"]);
        assert_eq!(split_particle("рука"), ["рука"]);
        assert_eq!(split_particle("то"), ["то"]);
        assert_eq!(split_particle("порусски"), ["порусски"]);
    }
}
