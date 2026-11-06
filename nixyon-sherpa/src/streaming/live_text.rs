use sherpa_onnx::{OnlineRecognizer, OnlineStream};

use super::StreamModel;
use crate::SherpaError;

/// Живой текст по мере поступления звука: законченные фразы (граница по тишине) и
/// текущая, ещё меняющаяся фраза.
pub struct LiveText {
    recognizer: OnlineRecognizer,
    stream: OnlineStream,
    tail_sec: f32,
    phrases: Vec<String>,
}

impl LiveText {
    pub fn new(model: &StreamModel, threads: i32, sample_rate: i32) -> Result<Self, SherpaError> {
        let recognizer = OnlineRecognizer::create(&model.config(threads))
            .ok_or(SherpaError::CreateFailed("потоковый распознаватель"))?;
        let stream = recognizer.create_stream();
        let (lead_sec, tail_sec) = model.padding_sec();
        stream.accept_waveform(sample_rate, &silence(sample_rate, lead_sec));
        Ok(Self {
            recognizer,
            stream,
            tail_sec,
            phrases: Vec::new(),
        })
    }

    /// Принимает кусок звука и распознаёт всё, что уже можно.
    pub fn accept(&mut self, sample_rate: i32, samples: &[f32]) {
        self.stream.accept_waveform(sample_rate, samples);
        self.decode();
        if self.recognizer.is_endpoint(&self.stream) {
            self.close_phrase();
            self.recognizer.reset(&self.stream);
        }
    }

    /// Текущая, ещё не законченная фраза.
    pub fn partial(&self) -> String {
        self.recognizer
            .get_result(&self.stream)
            .map(|r| r.text.trim().to_string())
            .unwrap_or_default()
    }

    /// Звук кончился: дораспознаёт хвост и отдаёт все фразы.
    pub fn finish(mut self, sample_rate: i32) -> Vec<String> {
        self.stream
            .accept_waveform(sample_rate, &silence(sample_rate, self.tail_sec));
        self.stream.input_finished();
        self.decode();
        self.close_phrase();
        self.phrases
    }

    fn decode(&self) {
        while self.recognizer.is_ready(&self.stream) {
            self.recognizer.decode(&self.stream);
        }
    }

    fn close_phrase(&mut self) {
        let text = self.partial();
        if !text.is_empty() {
            self.phrases.push(text);
        }
    }
}

fn silence(sample_rate: i32, sec: f32) -> Vec<f32> {
    // Частота и секунды малы и неотрицательны, приведение без потерь по смыслу.
    vec![0.0; (sample_rate as f32 * sec).round() as usize]
}
