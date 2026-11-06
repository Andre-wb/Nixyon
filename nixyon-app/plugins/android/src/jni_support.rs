//! Общее для границ JNI моделей: исключение в Kotlin и чтение байтов модели из
//! дескриптора, который Kotlin держит открытым только на время вызова.

use std::fs::File;
use std::os::fd::BorrowedFd;

use jni::JNIEnv;
use jni::sys::{jint, jlong};

use crate::model_slice::read_model_slice;

const ERROR_CLASS: &str = "java/lang/IllegalStateException";

/// Бросает исключение в Kotlin. Если исключение уже висит (сбой самого JNI), новое не
/// бросается: JNI запрещает бросать поверх, Kotlin получит уже висящее. Если бросить не
/// вышло совсем, Kotlin всё равно видит ошибку: 0 или null вместо результата.
pub fn throw(env: &mut JNIEnv<'_>, message: &str) {
    if env.exception_check().unwrap_or(true) {
        return;
    }
    let _ = env.throw_new(ERROR_CLASS, message);
}

/// Байты модели: `length` байт с `offset` в открытом файле `fd` (отдельный файл или APK).
pub fn model_bytes(fd: jint, offset: jlong, length: jlong) -> Result<Vec<u8>, String> {
    if fd < 0 {
        return Err("нет дескриптора файла модели".into());
    }
    let offset = u64::try_from(offset).map_err(|_| "отрицательное смещение модели")?;
    let length = u64::try_from(length).map_err(|_| "отрицательная длина модели")?;
    // SAFETY: Kotlin держит дескриптор открытым до возврата из nativeOpen (контракт
    // VoiceModelNative.kt и AgeModelNative.kt), а здесь он используется только внутри
    // вызова: сразу дублируется и больше не нужен.
    let borrowed = unsafe { BorrowedFd::borrow_raw(fd) };
    let file = File::from(
        borrowed
            .try_clone_to_owned()
            .map_err(|e| format!("дескриптор модели не дублируется: {e}"))?,
    );
    read_model_slice(&file, offset, length).map_err(|e| format!("файл модели не прочитан: {e}"))
}
