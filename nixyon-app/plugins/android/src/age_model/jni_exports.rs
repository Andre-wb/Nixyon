//! Граница JNI (AgeModelNative.kt):
//! - nativeOpen читает модель из открытого дескриптора с заданного смещения и длины
//!   (модель внутри APK), nativeOpenFile открывает её прямо с диска по пути: так в
//!   памяти нет второй копии файла на время загрузки. Оба проверяют вшитый классификатор
//!   и возвращают указатель как long; 0 не возвращается никогда, ошибка приходит
//!   исключением IllegalStateException. Дескриптор дублируется только на время чтения,
//!   вызывающий закрывает свой сам.
//! - Владелец указателя Kotlin: ровно один вызов nativeClose освобождает модель, после
//!   этого указатель не используется. nativeSpeakerVoice не пересекается с nativeClose.
//! - Звук и признаки голоса в лог и в текст ошибок не попадают.

use jni::JNIEnv;
use jni::objects::{JFloatArray, JObject, JObjectArray, JString};
use jni::sys::{jint, jintArray, jlong};
use nixyon_ort::{AgeGender, ChildAdult, bundled_child_adult, speaker_voice};

use crate::jni_support::{model_bytes, throw};

/// Окно модели, как при обучении классификатора.
const WINDOW_SEC: f64 = 10.0;
const SAMPLE_RATE: f64 = 16_000.0;
// Ответ из двух чисел: [ребёнок, пол]. Ребёнок: 1 да, 0 взрослый; пол: 1 женский,
// 0 мужской; -1 ответа нет.
const YES: jint = 1;
const NO: jint = 0;
const UNKNOWN: jint = -1;

fn answer(value: Option<bool>) -> jint {
    match value {
        Some(true) => YES,
        Some(false) => NO,
        None => UNKNOWN,
    }
}

struct AgeModel {
    model: AgeGender,
    classifier: ChildAdult,
}

fn open(fd: jint, offset: jlong, length: jlong, threads: jint) -> Result<AgeModel, String> {
    // Классификатор проверяется до чтения 363 МБ модели: испорченные веса видны сразу.
    let classifier = bundled_child_adult().map_err(|e| e.to_string())?;
    let threads = usize::try_from(threads).unwrap_or(1).max(1);
    let bytes = model_bytes(fd, offset, length)?;
    let model = AgeGender::from_bytes(&bytes, threads, WINDOW_SEC).map_err(|e| e.to_string())?;
    Ok(AgeModel { model, classifier })
}

fn open_file(env: &mut JNIEnv<'_>, path: &JString<'_>, threads: jint) -> Result<AgeModel, String> {
    let classifier = bundled_child_adult().map_err(|e| e.to_string())?;
    let threads = usize::try_from(threads).unwrap_or(1).max(1);
    let path: String = env.get_string(path).map_err(|e| e.to_string())?.into();
    let model = AgeGender::load_pool(std::path::Path::new(&path), threads, 1, WINDOW_SEC)
        .map_err(|e| e.to_string())?;
    Ok(AgeModel { model, classifier })
}

/// Модель по указателю из nativeOpen; 0 даёт None.
fn model<'a>(handle: jlong) -> Option<&'a AgeModel> {
    let ptr = std::ptr::with_exposed_provenance::<AgeModel>(handle as usize);
    // SAFETY: ненулевой handle получен из nativeOpen и ещё не освобождён nativeClose
    // (контракт AgeModelNative.kt), поэтому указывает на живую модель.
    unsafe { ptr.as_ref() }
}

fn voice(
    env: &mut JNIEnv<'_>,
    handle: jlong,
    pieces: &JObjectArray<'_>,
) -> Result<[jint; 2], String> {
    let age = model(handle).ok_or("модель audEERING не открыта")?;
    let count = env.get_array_length(pieces).map_err(|e| e.to_string())?;
    let mut analyses = Vec::new();
    for i in 0..count {
        let piece = JFloatArray::from(
            env.get_object_array_element(pieces, i)
                .map_err(|e| e.to_string())?,
        );
        let length = env.get_array_length(&piece).map_err(|e| e.to_string())?;
        let mut samples = vec![0f32; usize::try_from(length).unwrap_or(0)];
        env.get_float_array_region(&piece, 0, &mut samples)
            .map_err(|e| e.to_string())?;
        env.delete_local_ref(piece).map_err(|e| e.to_string())?;
        if let Some(a) = age.model.analyze(&samples).map_err(|e| e.to_string())? {
            analyses.push((a, samples.len() as f64 / SAMPLE_RATE));
        }
    }
    let voice = speaker_voice(&age.classifier, &analyses).map_err(|e| e.to_string())?;
    Ok([answer(voice.child), answer(voice.female)])
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_demo_nixyon_speech_AgeModelNative_nativeOpen<'local>(
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
pub extern "system" fn Java_dev_demo_nixyon_speech_AgeModelNative_nativeOpenFile<'local>(
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

/// Куски речи одного спикера, 16 кГц: [ребёнок, пол], см. YES, NO и UNKNOWN; null при
/// ошибке.
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_demo_nixyon_speech_AgeModelNative_nativeSpeakerVoice<'local>(
    mut env: JNIEnv<'local>,
    _this: JObject<'local>,
    handle: jlong,
    pieces: JObjectArray<'local>,
) -> jintArray {
    let result = voice(&mut env, handle, &pieces).and_then(|v| {
        let out = env.new_int_array(2).map_err(|e| e.to_string())?;
        env.set_int_array_region(&out, 0, &v)
            .map_err(|e| e.to_string())?;
        Ok(out)
    });
    match result {
        Ok(out) => out.into_raw(),
        Err(message) => {
            throw(&mut env, &message);
            std::ptr::null_mut()
        }
    }
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_demo_nixyon_speech_AgeModelNative_nativeClose<'local>(
    _env: JNIEnv<'local>,
    _this: JObject<'local>,
    handle: jlong,
) {
    let ptr = std::ptr::with_exposed_provenance_mut::<AgeModel>(handle as usize);
    if ptr.is_null() {
        return;
    }
    // SAFETY: ненулевой handle получен из nativeOpen через Box::into_raw, и Kotlin
    // освобождает его ровно одним nativeClose (контракт AgeModelNative.kt).
    drop(unsafe { Box::from_raw(ptr) });
}
