//! Модель сегментации pyannote для Kotlin-части: Kotlin зовёт эти функции через JNI,
//! перекрытия по всей записи ищет nixyon-ort на том же ONNX Runtime, что и sherpa-onnx.

#[allow(unsafe_code)]
mod jni_exports;
