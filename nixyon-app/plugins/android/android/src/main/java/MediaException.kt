package dev.demo.nixyon.speech

// code уходит в Rust и дальше в интерфейс; message только для лога.
class MediaException(val code: String, message: String, cause: Throwable? = null) :
    Exception(message, cause) {
    companion object {
        const val MIC_DENIED = "mic_denied"
        const val MIC_UNAVAILABLE = "mic_unavailable"
        const val ALREADY_RECORDING = "already_recording"
        const val NOT_RECORDING = "not_recording"
        const val STORAGE_LOW = "storage_low"
        const val TOO_LONG = "too_long"
        const val NO_AUDIO_TRACK = "no_audio_track"
        const val UNSUPPORTED_AUDIO = "unsupported_audio"
        const val IMPORT_FAILED = "import_failed"
        const val CANCELLED = "cancelled"
        const val BUSY = "busy"
        const val PLAYBACK_FAILED = "playback_failed"
        const val NOT_PLAYING = "not_playing"
    }
}
