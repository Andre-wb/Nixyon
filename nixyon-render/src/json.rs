use nixyon_core::{CoreError, Transcript, to_json};

/// JSON: источник истины, остальные форматы строятся из него.
pub fn render_json(transcript: &Transcript) -> Result<String, CoreError> {
    to_json(transcript)
}
