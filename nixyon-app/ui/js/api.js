import { K, t } from "./i18n.js";

const core = () => window.__TAURI__.core;

export function channel(onMessage) {
  const ch = new (core().Channel)();
  ch.onmessage = onMessage;
  return ch;
}

export const invoke = (command, args) => core().invoke(command, args);

// Начало записи с микрофона: тексты уведомления «Идёт запись» на языке интерфейса.
// sample: образец голоса, он не заменяет запись последней встречи.
export function startMicrophone(onEvent, sample = false) {
  return invoke("start_recording", {
    onEvent,
    sample,
    notificationTitle: t(K.recording_notification_title),
    notificationChannel: t(K.recording_notification_channel),
  });
}

// Код ошибки из Rust или Kotlin превращается в понятный текст. Явная таблица: опечатку
// в ключе поймает сборка, а неиспользуемый ключ поймает тест словаря.
const ERRORS = {
  already_recording: K.error_already_recording,
  bad_audio: K.error_bad_audio,
  bad_path: K.error_bad_path,
  bad_position: K.error_bad_position,
  bad_speakers: K.error_bad_speakers,
  bad_voice_name: K.error_bad_voice_name,
  biometric_unavailable: K.error_biometric_unavailable,
  busy: K.error_busy,
  engine_failed: K.error_engine_failed,
  export_failed: K.error_export_failed,
  hotword_too_long: K.error_hotword_too_long,
  import_failed: K.error_import_failed,
  last_turn: K.error_last_turn,
  llm_broken: K.error_llm_broken,
  llm_busy: K.error_llm_busy,
  llm_failed: K.error_llm_failed,
  llm_load_failed: K.error_llm_load_failed,
  llm_no_memory: K.error_llm_no_memory,
  llm_not_found: K.error_llm_not_found,
  llm_too_long: K.error_llm_too_long,
  locked: K.error_locked,
  mic_denied: K.error_mic_denied,
  mic_unavailable: K.error_mic_unavailable,
  models_bad_manifest: K.error_models_bad_manifest,
  models_corrupt: K.error_models_corrupt,
  models_download_corrupt: K.error_models_download_corrupt,
  models_download_failed: K.error_models_download_failed,
  models_incomplete: K.error_models_incomplete,
  models_load_failed: K.error_models_load_failed,
  models_not_found: K.error_models_not_found,
  models_storage_low: K.error_models_storage_low,
  name_control_chars: K.error_name_control_chars,
  name_too_long: K.error_name_too_long,
  no_audio_track: K.error_no_audio_track,
  no_audio: K.error_no_audio,
  no_speech: K.error_no_speech,
  no_transcript: K.error_no_transcript,
  no_voice_center: K.error_no_voice_center,
  not_playing: K.error_not_playing,
  not_recording: K.error_not_recording,
  playback_failed: K.error_playback_failed,
  same_speaker: K.error_same_speaker,
  sample_too_short: K.error_sample_too_short,
  save_failed: K.error_save_failed,
  share_failed: K.error_share_failed,
  silent: K.error_silent,
  speaker_unnamed: K.error_speaker_unnamed,
  storage_failed: K.error_storage_failed,
  storage_key_lost: K.error_storage_key_lost,
  storage_low: K.error_storage_low,
  too_long: K.error_too_long,
  too_many_hotwords: K.error_too_many_hotwords,
  too_short: K.error_too_short,
  text_control_chars: K.error_text_control_chars,
  text_empty: K.error_text_empty,
  text_too_long: K.error_text_too_long,
  unknown_speaker: K.error_unknown_speaker,
  unknown_turn: K.error_unknown_turn,
  unknown_voice: K.error_unknown_voice,
  unsupported_audio: K.error_unsupported_audio,
  voice_name_taken: K.error_voice_name_taken,
};

export function errorCode(error) {
  return typeof error === "object" && error && typeof error.code === "string" ? error.code : "unknown";
}

export function errorText(error) {
  return t(ERRORS[errorCode(error)] ?? K.error_unknown);
}
