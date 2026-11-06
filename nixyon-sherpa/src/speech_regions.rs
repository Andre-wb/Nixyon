use sherpa_onnx::VoiceActivityDetector;

/// Кусок речи, найденный VAD: номер первого сэмпла и сами сэмплы.
pub(crate) struct SpeechRegion {
    pub start_sample: i64,
    pub samples: Vec<f32>,
}

// Звук подаётся кусками по 0,1 с, как он шёл бы с микрофона.
pub(crate) const CHUNK: usize = 1_600;

pub(crate) fn speech_regions(vad: &VoiceActivityDetector, samples: &[f32]) -> Vec<SpeechRegion> {
    let mut regions = Vec::new();
    for chunk in samples.chunks(CHUNK) {
        vad.accept_waveform(chunk);
        drain(vad, &mut regions);
    }
    vad.flush();
    drain(vad, &mut regions);
    regions
}

pub(crate) fn drain(vad: &VoiceActivityDetector, regions: &mut Vec<SpeechRegion>) {
    while !vad.is_empty() {
        if let Some(segment) = vad.front() {
            regions.push(SpeechRegion {
                start_sample: segment.start(),
                samples: segment.samples().to_vec(),
            });
        }
        vad.pop();
    }
}
