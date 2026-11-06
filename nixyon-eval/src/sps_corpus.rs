use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use crate::{LabeledVoiceClip, VoiceGender};

const CORPUS_PREFIX: &str = "ss-corpus-";
const REPORTED_PREFIX: &str = "ss-reported-audios-";
const AUDIO_DIR: &str = "audios";
/// Возрастные группы Common Voice от 19 лет. Группа teens означает «до 19», в неё могут
/// попасть и дети, поэтому про неё «ребёнок или взрослый» не известно.
const ADULT_AGES: [&str; 8] = [
    "twenties",
    "thirties",
    "forties",
    "fifties",
    "sixties",
    "seventies",
    "eighties",
    "nineties",
];

#[derive(Debug, thiserror::Error)]
pub enum SpsCorpusError {
    #[error("{0}: {1}")]
    Io(PathBuf, std::io::Error),
    #[error("в {0} нет таблицы {CORPUS_PREFIX}<язык>.tsv")]
    NoTable(PathBuf),
    #[error("{0}: нет столбца {1}")]
    NoColumn(PathBuf, &'static str),
    #[error("{0}, строка {1}: число полей не совпадает с шапкой")]
    BadRow(PathBuf, usize),
    #[error("{0}: недопустимое имя звука {1}")]
    BadAudioName(PathBuf, String),
}

/// Клипы набора Common Voice Spontaneous Speech (`sps-corpus-*-<язык>`): говорящий из
/// `client_id`, пол и возраст из анкеты. Клипы из таблицы жалоб пропускаются. Порядок как
/// в таблице.
pub fn read_sps_corpus(dir: &Path) -> Result<Vec<LabeledVoiceClip>, SpsCorpusError> {
    let corpus =
        find_table(dir, CORPUS_PREFIX)?.ok_or_else(|| SpsCorpusError::NoTable(dir.into()))?;
    let reported = match find_table(dir, REPORTED_PREFIX)? {
        Some(path) => reported_files(&path)?,
        None => BTreeSet::new(),
    };
    let rows = read_tsv(&corpus)?;
    let column = |name: &'static str| {
        rows.column(name)
            .ok_or_else(|| SpsCorpusError::NoColumn(corpus.clone(), name))
    };
    let (speaker, file, gender, age) = (
        column("client_id")?,
        column("audio_file")?,
        column("gender")?,
        column("age")?,
    );
    let mut clips = Vec::new();
    for row in &rows.rows {
        let name = &row[file];
        if reported.contains(name) {
            continue;
        }
        if !is_plain_file_name(name) {
            return Err(SpsCorpusError::BadAudioName(corpus.clone(), name.clone()));
        }
        clips.push(LabeledVoiceClip {
            speaker: row[speaker].clone(),
            audio: dir.join(AUDIO_DIR).join(name),
            gender: parse_gender(&row[gender]),
            child: parse_child(&row[age]),
        });
    }
    Ok(clips)
}

/// Пол из анкеты Common Voice; остальные ответы для замера не годятся.
fn parse_gender(value: &str) -> Option<VoiceGender> {
    match value {
        "male_masculine" => Some(VoiceGender::Male),
        "female_feminine" => Some(VoiceGender::Female),
        _ => None,
    }
}

fn parse_child(age: &str) -> Option<bool> {
    ADULT_AGES.contains(&age).then_some(false)
}

fn reported_files(path: &Path) -> Result<BTreeSet<String>, SpsCorpusError> {
    let rows = read_tsv(path)?;
    let file = rows
        .column("audio_file")
        .ok_or_else(|| SpsCorpusError::NoColumn(path.into(), "audio_file"))?;
    Ok(rows.rows.iter().map(|r| r[file].clone()).collect())
}

fn find_table(dir: &Path, prefix: &str) -> Result<Option<PathBuf>, SpsCorpusError> {
    let entries = std::fs::read_dir(dir).map_err(|e| SpsCorpusError::Io(dir.into(), e))?;
    let mut found = Vec::new();
    for entry in entries {
        let path = entry.map_err(|e| SpsCorpusError::Io(dir.into(), e))?.path();
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
        if name.is_some_and(|n| n.starts_with(prefix) && n.ends_with(".tsv")) {
            found.push(path);
        }
    }
    found.sort();
    Ok(found.into_iter().next())
}

struct Tsv {
    columns: HashMap<String, usize>,
    rows: Vec<Vec<String>>,
}

impl Tsv {
    fn column(&self, name: &str) -> Option<usize> {
        self.columns.get(name).copied()
    }
}

// В таблицах Common Voice поля без кавычек, табуляция внутри поля не встречается.
fn read_tsv(path: &Path) -> Result<Tsv, SpsCorpusError> {
    let text = std::fs::read_to_string(path).map_err(|e| SpsCorpusError::Io(path.into(), e))?;
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().unwrap_or_default().split('\t').collect();
    let columns = header
        .iter()
        .enumerate()
        .map(|(i, name)| (name.to_string(), i))
        .collect();
    let mut rows = Vec::new();
    for (i, line) in lines.enumerate() {
        if line.is_empty() {
            continue;
        }
        let row: Vec<String> = line.split('\t').map(str::to_string).collect();
        if row.len() != header.len() {
            return Err(SpsCorpusError::BadRow(path.into(), i + 2));
        }
        rows.push(row);
    }
    Ok(Tsv { columns, rows })
}

fn is_plain_file_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains('/') && !name.contains('\\')
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Dir(PathBuf);

    impl Dir {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("nixyon-sps-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn write(&self, name: &str, text: &str) {
            std::fs::write(self.0.join(name), text).unwrap();
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    const HEADER: &str = "client_id\taudio_file\tage\tgender\n";

    #[test]
    fn speaker_gender_and_age_come_from_the_table() {
        let dir = Dir::new("ok");
        dir.write(
            "ss-corpus-ru.tsv",
            &format!(
                "{HEADER}a\t1.mp3\ttwenties\tmale_masculine\nb\t2.mp3\tteens\tfemale_feminine\nc\t3.mp3\t\t\n"
            ),
        );
        let clips = read_sps_corpus(&dir.0).unwrap();
        assert_eq!(clips.len(), 3);
        assert_eq!(clips[0].speaker, "a");
        assert_eq!(clips[0].audio, dir.0.join("audios/1.mp3"));
        assert_eq!(clips[0].gender, Some(VoiceGender::Male));
        assert_eq!(clips[0].child, Some(false));
        assert_eq!(clips[1].gender, Some(VoiceGender::Female));
        assert_eq!(clips[1].child, None);
        assert_eq!((clips[2].gender, clips[2].child), (None, None));
    }

    #[test]
    fn answers_other_than_male_or_female_give_no_gender() {
        assert_eq!(parse_gender("do_not_wish_to_say"), None);
        assert_eq!(parse_gender("intersex"), None);
    }

    #[test]
    fn reported_clips_are_skipped() {
        let dir = Dir::new("reported");
        dir.write(
            "ss-corpus-ru.tsv",
            &format!(
                "{HEADER}a\t1.mp3\ttwenties\tmale_masculine\na\t2.mp3\ttwenties\tmale_masculine\n"
            ),
        );
        dir.write(
            "ss-reported-audios-ru.tsv",
            "client_id\taudio_file\na\t1.mp3\n",
        );
        let clips = read_sps_corpus(&dir.0).unwrap();
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].audio, dir.0.join("audios/2.mp3"));
    }

    #[test]
    fn a_row_with_a_wrong_field_count_is_an_error() {
        let dir = Dir::new("row");
        dir.write("ss-corpus-ru.tsv", &format!("{HEADER}a\t1.mp3\n"));
        assert!(matches!(
            read_sps_corpus(&dir.0),
            Err(SpsCorpusError::BadRow(_, 2))
        ));
    }

    #[test]
    fn an_audio_name_with_a_folder_is_refused() {
        let dir = Dir::new("path");
        dir.write(
            "ss-corpus-ru.tsv",
            &format!("{HEADER}a\t../x.mp3\ttwenties\tmale_masculine\n"),
        );
        assert!(matches!(
            read_sps_corpus(&dir.0),
            Err(SpsCorpusError::BadAudioName(_, _))
        ));
    }

    #[test]
    fn a_folder_without_the_table_is_an_error() {
        let dir = Dir::new("empty");
        assert!(matches!(
            read_sps_corpus(&dir.0),
            Err(SpsCorpusError::NoTable(_))
        ));
    }
}
