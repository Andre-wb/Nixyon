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
        const val DOWNLOAD_FAILED = "models_download_failed"
        const val DOWNLOAD_CORRUPT = "models_download_corrupt"
        const val STORAGE_LOW = "models_storage_low"
        const val CANCELLED = "cancelled"
        const val BUSY = "busy"
    }
}
