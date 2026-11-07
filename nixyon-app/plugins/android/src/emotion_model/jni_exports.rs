//! Граница JNI (EmotionModelNative.kt):
//! - nativeOpen читает модель из открытого дескриптора с заданного смещения и длины
//!   (модель внутри APK), nativeOpenFile открывает её прямо с диска по пути. Оба
//!   возвращают указатель как long; 0 не возвращается никогда, ошибка приходит
//!   исключением IllegalStateException. Дескриптор дублируется только на время чтения,
//!   вызывающий закрывает свой сам.
//! - Владелец указателя Kotlin: ровно один вызов nativeClose освобождает модель, после
//!   этого указатель не используется. nativeTurnEmotion не пересекается с nativeClose.
//! - Звук и вероятности в лог и в текст ошибок не попадают.

use jni::JNIEnv;
use jni::objects::{JFloatArray, JObject, JString};
use jni::sys::{jint, jlong, jstring};
use nixyon_core::TurnEmotion;
use nixyon_ort::{EmotionModel, EmotionProbs};

use crate::jni_support::{model_bytes, throw};

fn threads(threads: jint) -> usize {
    usize::try_from(threads).unwrap_or(1).max(1)
}

fn open(fd: jint, offset: jlong, length: jlong, count: jint) -> Result<EmotionModel, String> {
    let bytes = model_bytes(fd, offset, length)?;
    EmotionModel::from_bytes(&bytes, threads(count)).map_err(|e| e.to_string())
}

fn open_file(
    env: &mut JNIEnv<'_>,
    path: &JString<'_>,
    count: jint,
) -> Result<EmotionModel, String> {
    let path: String = env.get_string(path).map_err(|e| e.to_string())?.into();
    EmotionModel::load(std::path::Path::new(&path), threads(count)).map_err(|e| e.to_string())
}

/// Модель по указателю из nativeOpen; 0 даёт None.
fn model<'a>(handle: jlong) -> Option<&'a EmotionModel> {
    let ptr = std::ptr::with_exposed_provenance::<EmotionModel>(handle as usize);
    // SAFETY: ненулевой handle получен из nativeOpen и ещё не освобождён nativeClose
    // (контракт EmotionModelNative.kt), поэтому указывает на живую модель.
    unsafe { ptr.as_ref() }
}

/// Ключ эмоции в том виде, в каком он лежит в расшифровке.
fn key(emotion: TurnEmotion) -> Result<String, String> {
    match serde_json::to_value(emotion).map_err(|e| e.to_string())? {
        serde_json::Value::String(key) => Ok(key),
        other => Err(format!("ключ эмоции {other}")),
    }
}

fn emotion(
    env: &mut JNIEnv<'_>,
    handle: jlong,
    samples: &JFloatArray<'_>,
) -> Result<Option<String>, String> {
    let model = model(handle).ok_or("модель эмоций не открыта")?;
    let length = env.get_array_length(samples).map_err(|e| e.to_string())?;
    let mut buffer = vec![0f32; usize::try_from(length).unwrap_or(0)];
    env.get_float_array_region(samples, 0, &mut buffer)
        .map_err(|e| e.to_string())?;
    let probs = model.probs(&buffer).map_err(|e| e.to_string())?;
    probs
        .and_then(|p| p.emotion(EmotionProbs::THRESHOLD))
        .map(key)
        .transpose()
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_demo_nixyon_speech_EmotionModelNative_nativeOpen<'local>(
    mut env: JNIEnv<'local>,
    _this: JObject<'local>,
    fd: jint,
    offset: jlong,
    length: jlong,
    threads: jint,
) -> jlong {
    match open(fd, offset, length, threads) {
        Ok(model) => Box::into_raw(Box::new(model)).expose_provenance() as jlong,
        Err(message) => {
            throw(&mut env, &message);
            0
        }
    }
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_demo_nixyon_speech_EmotionModelNative_nativeOpenFile<'local>(
    mut env: JNIEnv<'local>,
    _this: JObject<'local>,
    path: JString<'local>,
    threads: jint,
) -> jlong {
    match open_file(&mut env, &path, threads) {
        Ok(model) => Box::into_raw(Box::new(model)).expose_provenance() as jlong,
        Err(message) => {
            throw(&mut env, &message);
            0
        }
    }
}

/// Звук одной реплики, 16 кГц: ключ эмоции или null, если ответа нет; null и исключение
/// при ошибке.
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_demo_nixyon_speech_EmotionModelNative_nativeTurnEmotion<'local>(
    mut env: JNIEnv<'local>,
    _this: JObject<'local>,
    handle: jlong,
    samples: JFloatArray<'local>,
) -> jstring {
    let result = emotion(&mut env, handle, &samples).and_then(|key| match key {
        Some(key) => env
            .new_string(key)
            .map(|s| s.into_raw())
            .map_err(|e| e.to_string()),
        None => Ok(std::ptr::null_mut()),
    });
    match result {
        Ok(out) => out,
        Err(message) => {
            throw(&mut env, &message);
            std::ptr::null_mut()
        }
    }
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_demo_nixyon_speech_EmotionModelNative_nativeClose<'local>(
    _env: JNIEnv<'local>,
    _this: JObject<'local>,
    handle: jlong,
) {
    let ptr = std::ptr::with_exposed_provenance_mut::<EmotionModel>(handle as usize);
    if ptr.is_null() {
        return;
    }
    // SAFETY: ненулевой handle получен из nativeOpen через Box::into_raw, и Kotlin
    // освобождает его ровно одним nativeClose (контракт EmotionModelNative.kt).
    drop(unsafe { Box::from_raw(ptr) });
}
