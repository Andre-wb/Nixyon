mod editing;
mod exporting;
mod fillers;
mod media;
mod model_setup;
mod playback;
mod reprocessing;
mod searching;
mod settings;
mod speaker_names;
mod statistics;
mod summary;
mod transcription;
mod vault;
mod voices;

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
            editing::delete_turn,
            editing::apply_turn_texts,
            fillers::remove_filler_words,
            vault::unlock_storage,
            editing::set_turn_emotion,
            statistics::meeting_stats,
            searching::search_transcript,
            playback::open_playback,
            playback::play_audio,
            playback::pause_audio,
            playback::close_playback,
            reprocessing::reprocess_turn,
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
            settings::set_models_prompt_seen,
            model_setup::models_status,
            model_setup::download_models,
            model_setup::cancel_model_download,
            model_setup::set_models_disabled,
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
            // они уже были готовы. Пока набор речи не скачан, грузить нечего.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if model_setup::speech_ready(&handle).await {
                    model_setup::warm_up(&handle).await;
                } else {
                    log::info!("модели речи не скачаны, загрузка при старте пропущена");
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while building tauri application");
}
