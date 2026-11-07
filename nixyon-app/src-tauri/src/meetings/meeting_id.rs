use nixyon_core::rfc3339_utc;
use serde::{Deserialize, Serialize};

/// id встречи: время создания её расшифровки, `ГГГГ-ММ-ДДTчч:мм:ссZ`. Приходит из интерфейса и
/// становится частью пути к папке встречи, поэтому принимается только строго этой формы: в
/// нём одни цифры и разделители на своих местах, выйти из папки архива им нельзя.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct MeetingId(String);

const SHAPE: &[u8; 20] = b"0000-00-00T00:00:00Z";

impl MeetingId {
    pub fn parse(text: &str) -> Option<Self> {
        let bytes = text.as_bytes();
        let fits = bytes.len() == SHAPE.len()
            && bytes.iter().zip(SHAPE).all(|(&b, &s)| {
                if s == b'0' {
                    b.is_ascii_digit()
                } else {
                    b == s
                }
            });
        fits.then(|| Self(text.to_owned()))
    }

    pub fn from_unix_secs(secs: u64) -> Self {
        Self(rfc3339_utc(secs))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Имя папки встречи: id без `-` и `:`, например `20261107T100000Z`.
    pub fn dir_name(&self) -> String {
        self.0.chars().filter(|c| *c != '-' && *c != ':').collect()
    }
}

impl TryFrom<String> for MeetingId {
    type Error = String;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::parse(&text).ok_or_else(|| "id встречи не той формы".to_owned())
    }
}

impl From<MeetingId> for String {
    fn from(id: MeetingId) -> Self {
        id.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_of_the_right_shape_is_accepted_and_gives_a_folder_name_without_separators() {
        let id = MeetingId::parse("2026-11-07T10:00:05Z").unwrap();
        assert_eq!(id.as_str(), "2026-11-07T10:00:05Z");
        assert_eq!(id.dir_name(), "20261107T100005Z");
        assert_eq!(
            MeetingId::from_unix_secs(0).as_str(),
            "1970-01-01T00:00:00Z"
        );
    }

    #[test]
    fn path_traversal_and_near_misses_are_rejected_cwe_22() {
        for bad in [
            "",
            "..",
            "../../../etc/passwd",
            "2026-11-07T10:00:05Z/..",
            "2026-11-07T10:00:0/Z",
            "2026-11-07T10:00:0\\Z",
            "2026-11-07T10:00:0\u{0}Z",
            "2026-11-07T10:00:05",
            "2026-11-07 10:00:05Z",
            "2026-11-07T10:00:05z",
            "2026-11-07T10:00:05ZZ",
            "2026-11-07T10:00:05+03",
            "2026-11-0٧T10:00:05Z",
            "x026-11-07T10:00:05Z",
        ] {
            assert_eq!(MeetingId::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn later_meeting_has_the_greater_id() {
        let ids: Vec<MeetingId> = [9, 59, 3_599, 86_399, 86_400, 1_000_000_000, 2_000_000_000]
            .into_iter()
            .map(MeetingId::from_unix_secs)
            .collect();
        assert!(ids.windows(2).all(|w| w[0] < w[1]));
    }
}
