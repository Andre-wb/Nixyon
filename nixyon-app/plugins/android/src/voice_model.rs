//! Модель голоса ReDimNet2 для Kotlin-части: Kotlin зовёт эти функции через JNI,
//! эмбеддинг считает nixyon-ort на том же ONNX Runtime, что и sherpa-onnx.

#[cfg(target_os = "android")]
#[allow(unsafe_code)]
mod jni_exports;
