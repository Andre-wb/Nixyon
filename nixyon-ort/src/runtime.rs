use std::path::Path;
use std::sync::{Mutex, OnceLock};

use ort::environment::Environment;
use ort::session::Session;

use crate::OrtError;

// ONNX Runtime разрешает одно окружение на процесс, поэтому оно общее для всех моделей.
static ENVIRONMENT: OnceLock<Environment> = OnceLock::new();
static CREATING: Mutex<()> = Mutex::new(());

fn environment() -> Result<&'static Environment, OrtError> {
    if let Some(env) = ENVIRONMENT.get() {
        return Ok(env);
    }
    // Два потока не должны создавать окружение одновременно: второе создание ort отвергает.
    let _guard = CREATING.lock().map_err(|_| OrtError::Poisoned)?;
    if let Some(env) = ENVIRONMENT.get() {
        return Ok(env);
    }
    let env = ort::init().with_name("nixyon").build()?;
    Ok(ENVIRONMENT.get_or_init(|| env))
}

/// Сессия модели из файла; `threads`: потоки внутри одного вызова модели.
pub fn session(model: &Path, threads: usize) -> Result<Session, OrtError> {
    let session = Session::builder(environment()?)?
        .with_intra_threads(threads.max(1))?
        .commit_from_file(model)
        .map_err(|e| OrtError::Runtime(format!("{}: {e}", model.display())))?;
    Ok(session)
}

/// Сессия модели из байтов файла: так грузится модель, которая лежит внутри APK.
/// ONNX Runtime копирует байты, после создания сессии их можно освободить.
pub fn session_from_bytes(model: &[u8], threads: usize) -> Result<Session, OrtError> {
    let session = Session::builder(environment()?)?
        .with_intra_threads(threads.max(1))?
        .commit_from_memory(model)?;
    Ok(session)
}
