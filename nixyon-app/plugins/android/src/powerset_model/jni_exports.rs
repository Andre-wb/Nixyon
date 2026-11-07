//! Граница JNI (PowersetModelNative.kt):
//! - nativeOpen читает модель из открытого дескриптора с заданного смещения и длины
//!   и возвращает указатель на модель как long; 0 не возвращается никогда, ошибка
//!   приходит исключением IllegalStateException. Дескриптор дублируется только на время
//!   чтения, вызывающий закрывает свой сам.
//! - Владелец указателя Kotlin: ровно один вызов nativeClose освобождает модель, после
//!   этого указатель не используется. nativeScan и nativeClose не пересекаются.
//! - nativeScan зовёт у `progress` метод `keepGoing(double): boolean` после каждого окна
//!   первого прохода; false отменяет обход, тогда nativeScan возвращает false и файл не
//!   пишет. Исключение из keepGoing тоже останавливает обход и доходит до Kotlin.
//! - Звук в лог и в текст ошибок не попадает.

use std::path::Path;

use jni::JNIEnv;
use jni::objects::{JObject, JString, JValue};
use jni::sys::{JNI_FALSE, JNI_TRUE, jboolean, jdouble, jfloat, jint, jlong};
use nixyon_ort::{PyannotePowerset, scan_overlaps};

use crate::jni_support::{model_bytes, throw};
use crate::overlaps::write_overlaps;

fn open(fd: jint, offset: jlong, length: jlong, threads: jint) -> Result<PyannotePowerset, String> {
    let threads = usize::try_from(threads).unwrap_or(1).max(1);
    let bytes = model_bytes(fd, offset, length)?;
    PyannotePowerset::from_bytes(&bytes, threads).map_err(|e| e.to_string())
}

/// Модель по указателю из nativeOpen; 0 даёт None.
fn model<'a>(handle: jlong) -> Option<&'a PyannotePowerset> {
    let ptr = std::ptr::with_exposed_provenance::<PyannotePowerset>(handle as usize);
    // SAFETY: ненулевой handle получен из nativeOpen и ещё не освобождён nativeClose
    // (контракт PowersetModelNative.kt), поэтому указывает на живую модель.
    unsafe { ptr.as_ref() }
}

struct ScanArgs<'a> {
    wav: &'a JString<'a>,
    threshold: jfloat,
    min_sec: jdouble,
    out: &'a JString<'a>,
    progress: &'a JObject<'a>,
}

// Ok(true): перекрытия записаны; Ok(false): обход отменён или keepGoing бросил исключение.
fn scan(env: &mut JNIEnv<'_>, handle: jlong, args: &ScanArgs<'_>) -> Result<bool, String> {
    let model = model(handle).ok_or("модель сегментации не открыта")?;
    let wav: String = env.get_string(args.wav).map_err(|e| e.to_string())?.into();
    let out: String = env.get_string(args.out).map_err(|e| e.to_string())?.into();
    let mut keep_going = |done_sec: f64| {
        env.call_method(
            args.progress,
            "keepGoing",
            "(D)Z",
            &[JValue::Double(done_sec)],
        )
        .and_then(|v| v.z())
        .unwrap_or(false)
    };
    let found = scan_overlaps(
        model,
        Path::new(&wav),
        args.threshold,
        args.min_sec,
        &mut keep_going,
    )
    .map_err(|e| e.to_string())?;
    match found {
        Some(overlaps) => {
            write_overlaps(Path::new(&out), &overlaps).map_err(|e| e.to_string())?;
            Ok(true)
        }
        None => Ok(false),
    }
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_demo_nixyon_speech_PowersetModelNative_nativeOpen<'local>(
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

/// Перекрытия по записи WAV 16 кГц в файл `out`. true: записаны; false: отменено.
#[allow(non_snake_case, clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_demo_nixyon_speech_PowersetModelNative_nativeScan<'local>(
    mut env: JNIEnv<'local>,
    _this: JObject<'local>,
    handle: jlong,
    wav: JString<'local>,
    threshold: jfloat,
    min_sec: jdouble,
    out: JString<'local>,
    progress: JObject<'local>,
) -> jboolean {
    let args = ScanArgs {
        wav: &wav,
        threshold,
        min_sec,
        out: &out,
        progress: &progress,
    };
    match scan(&mut env, handle, &args) {
        Ok(true) => JNI_TRUE,
        Ok(false) => JNI_FALSE,
        Err(message) => {
            throw(&mut env, &message);
            JNI_FALSE
        }
    }
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_demo_nixyon_speech_PowersetModelNative_nativeClose<'local>(
    _env: JNIEnv<'local>,
    _this: JObject<'local>,
    handle: jlong,
) {
    let ptr = std::ptr::with_exposed_provenance_mut::<PyannotePowerset>(handle as usize);
    if ptr.is_null() {
        return;
    }
    // SAFETY: ненулевой handle получен из nativeOpen через Box::into_raw, и Kotlin
    // освобождает его ровно одним nativeClose (контракт PowersetModelNative.kt).
    drop(unsafe { Box::from_raw(ptr) });
}
