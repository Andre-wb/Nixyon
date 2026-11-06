mod editing;
mod exporting;
mod media;
mod playback;
mod searching;
mod settings;
mod speaker_names;
mod statistics;
mod summary;
mod transcription;
mod vault;
mod voices;

use nixyon_android::SpeechExt;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(nixyon_android::init())
        .manage(transcription::Running::default())
        .manage(summary::SummaryRunning::default())
        .manage(vault::VaultState::default())
        .invoke_handler(tauri::generate_handler![
            transcription::transcribe,
            transcription::cancel_transcription,
            transcription::last_transcript,
            media::start_recording,
            media::stop_recording,
            media::recovered_recording,
            media::import_audio,
            media::cancel_import,
            media::open_app_settings,
            editing::rename_speaker,
            editing::merge_speakers,
            editing::edit_turn_text,
            statistics::meeting_stats,
            searching::search_transcript,
            playback::open_playback,
            playback::play_audio,
            playback::pause_audio,
            playback::close_playback,
            voices::list_voices,
            voices::add_voice,
            voices::delete_voice,
            voices::discard_voice_recording,
            voices::remember_voice,
            settings::get_settings,
            settings::set_language,
            settings::set_theme,
            settings::set_system_bars,
            settings::set_hotwords,
            exporting::export_transcript,
            exporting::save_export,
            exporting::share_export,
            summary::summary_available,
            summary::summarize_meeting,
            summary::cancel_summary,
            summary::last_summary,
            speaker_names::suggest_speaker_names
        ])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            // Модели грузятся в фоне сразу после старта, чтобы к первой записи
            // они уже были готовы.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                match handle.speech().engine_status().await {
                    Ok(status) => log::info!(
                        "модели: {} ({}), загрузка {} мс, эмбеддинг {}",
                        status.source,
                        status.version,
                        status.load_ms,
                        status.embedding_dim
                    ),
                    Err(e) => log::error!("модели не загружены: {e}"),
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while building tauri application");
}
