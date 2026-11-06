//! Граница JNI (VoiceModelNative.kt):
//! - nativeOpen читает модель из открытого дескриптора с заданного смещения и длины
//!   и возвращает указатель на модель как long; 0 не возвращается никогда, ошибка
//!   приходит исключением IllegalStateException. Дескриптор дублируется только на время
//!   чтения, вызывающий закрывает свой сам.
//! - Владелец указателя Kotlin: ровно один вызов nativeClose освобождает модель, после
//!   этого указатель не используется. nativeEmbed и nativeDim не пересекаются с nativeClose.
//! - Звук и эмбеддинги в лог и в текст ошибок не попадают.

use jni::JNIEnv;
use jni::objects::{JFloatArray, JObject};
use jni::sys::{jfloatArray, jint, jlong, jsize};
use nixyon_core::VoiceEmbedder;
use nixyon_ort::Redimnet2;

use crate::jni_support::{model_bytes, throw};

fn open(fd: jint, offset: jlong, length: jlong, threads: jint) -> Result<Redimnet2, String> {
    let threads = usize::try_from(threads).unwrap_or(1).max(1);
    let bytes = model_bytes(fd, offset, length)?;
    Redimnet2::from_bytes(&bytes, threads).map_err(|e| e.to_string())
}

/// Модель по указателю из nativeOpen; 0 даёт None.
fn model<'a>(handle: jlong) -> Option<&'a Redimnet2> {
    let ptr = std::ptr::with_exposed_provenance::<Redimnet2>(handle as usize);
    // SAFETY: ненулевой handle получен из nativeOpen и ещё не освобождён nativeClose
    // (контракт VoiceModelNative.kt), поэтому указывает на живую модель.
    unsafe { ptr.as_ref() }
}

fn embed<'local>(
    env: &mut JNIEnv<'local>,
    handle: jlong,
    samples: &JFloatArray<'_>,
) -> Result<JFloatArray<'local>, String> {
    let model = model(handle).ok_or("модель голоса не открыта")?;
    let length = env.get_array_length(samples).map_err(|e| e.to_string())?;
    let mut buffer = vec![0f32; usize::try_from(length).unwrap_or(0)];
    env.get_float_array_region(samples, 0, &mut buffer)
        .map_err(|e| e.to_string())?;
    let embedding = model.embed(&buffer).map_err(|e| e.to_string())?;
    let size = jsize::try_from(embedding.len()).map_err(|_| "эмбеддинг слишком длинный")?;
    let out = env.new_float_array(size).map_err(|e| e.to_string())?;
    env.set_float_array_region(&out, 0, &embedding)
        .map_err(|e| e.to_string())?;
    Ok(out)
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_demo_nixyon_speech_VoiceModelNative_nativeOpen<'local>(
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

/// Эмбеддинг куска 16 кГц; пустой массив, если кусок слишком короткий для модели.
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_demo_nixyon_speech_VoiceModelNative_nativeEmbed<'local>(
    mut env: JNIEnv<'local>,
    _this: JObject<'local>,
    handle: jlong,
    samples: JFloatArray<'local>,
) -> jfloatArray {
    match embed(&mut env, handle, &samples) {
        Ok(out) => out.into_raw(),
        Err(message) => {
            throw(&mut env, &message);
            std::ptr::null_mut()
        }
    }
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_demo_nixyon_speech_VoiceModelNative_nativeDim<'local>(
    mut env: JNIEnv<'local>,
    _this: JObject<'local>,
    handle: jlong,
) -> jint {
    match model(handle).map(|m| jint::try_from(m.dim())) {
        Some(Ok(dim)) => dim,
        _ => {
            throw(&mut env, "модель голоса не открыта");
            0
        }
    }
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_demo_nixyon_speech_VoiceModelNative_nativeClose<'local>(
    _env: JNIEnv<'local>,
    _this: JObject<'local>,
    handle: jlong,
) {
    let ptr = std::ptr::with_exposed_provenance_mut::<Redimnet2>(handle as usize);
    if ptr.is_null() {
        return;
    }
    // SAFETY: ненулевой handle получен из nativeOpen через Box::into_raw, и Kotlin
    // освобождает его ровно одним nativeClose (контракт VoiceModelNative.kt).
    drop(unsafe { Box::from_raw(ptr) });
}
