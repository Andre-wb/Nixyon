package dev.demo.nixyon.speech

// code уходит в Rust и дальше в интерфейс; message только для лога.
class ModelsException(val code: String, message: String, cause: Throwable? = null) :
    Exception(message, cause) {
    companion object {
        const val NOT_FOUND = "models_not_found"
        const val BAD_MANIFEST = "models_bad_manifest"
        const val INCOMPLETE = "models_incomplete"
        const val CORRUPT = "models_corrupt"
        const val LOAD_FAILED = "models_load_failed"
    }
}
