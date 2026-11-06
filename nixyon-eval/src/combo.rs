use crate::{Segmenter, VoiceModel};

/// Сочетание модели голоса, нейросегментации и порога слияния групп в одного человека,
/// которым eval размечает уже распознанную запись: `<имя>=<голос>,<сегментация>,<порог>`.
#[derive(Debug, Clone, PartialEq)]
pub struct Combo {
    pub name: String,
    pub voice: VoiceModel,
    pub segmenter: Segmenter,
    pub same_speaker: f64,
}

impl Combo {
    pub fn parse(value: &str) -> Result<Self, String> {
        let usage = || {
            format!(
                "--combo {value}: <имя>=<голос>,<сегментация>,<порог>, например B=redimnet2,pyannote,0.7"
            )
        };
        let (name, rest) = value.split_once('=').ok_or_else(usage)?;
        let parts: Vec<&str> = rest.split(',').map(str::trim).collect();
        let [voice, segmenter, same] = parts.as_slice() else {
            return Err(usage());
        };
        if !is_valid_name(name) {
            return Err(format!(
                "--combo {value}: имя из латинских букв, цифр, «-» и «_», до 32 знаков"
            ));
        }
        Ok(Self {
            name: name.to_owned(),
            voice: VoiceModel::parse(voice)
                .ok_or_else(|| format!("--combo {value}: нет модели голоса {voice}"))?,
            segmenter: Segmenter::parse(segmenter)
                .ok_or_else(|| format!("--combo {value}: нет нейросегментации {segmenter}"))?,
            same_speaker: same
                .parse::<f64>()
                .ok()
                .filter(|c| c.is_finite() && (-1.0..=1.0).contains(c))
                .ok_or_else(|| format!("--combo {value}: порог, косинус от -1 до 1"))?,
        })
    }
}

// Имя сочетания становится именем папки дампов.
fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 32
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}

#[cfg(test)]
mod tests {
    use nixyon_ort::OrtRole;

    use super::*;

    #[test]
    fn combo_is_parsed_from_name_voice_segmenter_and_threshold() {
        assert_eq!(
            Combo::parse("B=redimnet2,pyannote,0.7"),
            Ok(Combo {
                name: "B".into(),
                voice: VoiceModel::Redimnet2(OrtRole::Redimnet2),
                segmenter: Segmenter::Pyannote,
                same_speaker: 0.7,
            })
        );
        assert_eq!(
            Combo::parse("cam_fp32=campplus, sortformer-fp32, 0.6")
                .unwrap()
                .segmenter,
            Segmenter::SortformerFp32
        );
    }

    #[test]
    fn bad_combos_are_refused_with_the_reason() {
        for (bad, reason) in [
            ("B", "<имя>="),
            ("B=redimnet2,pyannote", "<имя>="),
            ("B=redimnet2,pyannote,0.7,1", "<имя>="),
            ("=redimnet2,pyannote,0.7", "имя"),
            ("../B=redimnet2,pyannote,0.7", "имя"),
            ("B=titanet,pyannote,0.7", "модели голоса"),
            ("B=redimnet2,nemo,0.7", "нейросегментации"),
            ("B=redimnet2,pyannote,1.5", "порог"),
            ("B=redimnet2,pyannote,x", "порог"),
        ] {
            let error = Combo::parse(bad).unwrap_err();
            assert!(error.contains(reason), "{bad}: {error}");
        }
    }
}
