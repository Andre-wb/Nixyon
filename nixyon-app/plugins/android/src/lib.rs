use tauri::{
    Manager, Runtime,
    plugin::{Builder, TauriPlugin},
};

pub use models::*;

#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;

#[cfg(target_os = "android")]
mod age_model;
#[cfg(target_os = "android")]
mod emotion_model;
mod error;
#[cfg(target_os = "android")]
#[allow(unsafe_code)]
mod jni_support;
#[cfg(any(test, target_os = "android"))]
mod model_slice;
mod models;
mod segments;
mod spans;
#[cfg(target_os = "android")]
mod voice_model;

pub use error::{Error, Result};
pub use segments::{SegmentsError, read_segments};
pub use spans::read_spans;

#[cfg(desktop)]
use desktop::Speech;
#[cfg(mobile)]
use mobile::Speech;

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the speech APIs.
pub trait SpeechExt<R: Runtime> {
    fn speech(&self) -> &Speech<R>;
}

impl<R: Runtime, T: Manager<R>> crate::SpeechExt<R> for T {
    fn speech(&self) -> &Speech<R> {
        self.state::<Speech<R>>().inner()
    }
}

/// Initializes the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("nixyon-android")
        .setup(|app, api| {
            #[cfg(mobile)]
            let speech = mobile::init(app, api)?;
            #[cfg(desktop)]
            let speech = desktop::init(app, api)?;
            app.manage(speech);
            Ok(())
        })
        .build()
}
