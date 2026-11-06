package dev.demo.nixyon.speech

// code уходит в Rust и дальше в интерфейс; message только для лога.
class AnalysisException(val code: String, message: String, cause: Throwable? = null) :
    Exception(message, cause) {
    companion object {
        const val BAD_AUDIO = "bad_audio"
        const val CANCELLED = "cancelled"
        const val BUSY = "busy"
        const val ENGINE_FAILED = "engine_failed"
        const val SAMPLE_TOO_SHORT = "sample_too_short"
    }
}
