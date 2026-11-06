package dev.demo.nixyon.speech

// code уходит в Rust и дальше в интерфейс; message только для лога. Коды из C++
// (meeting_llm.cpp) совпадают с константами ниже.
// Конструктор из двух строк вызывает C++, поэтому @JvmOverloads.
class LlmException @JvmOverloads constructor(val code: String, message: String, cause: Throwable? = null) :
    Exception(message, cause) {
    companion object {
        const val NOT_FOUND = "llm_not_found"
        const val BROKEN = "llm_broken"
        const val NO_MEMORY = "llm_no_memory"
        const val BUSY = "llm_busy"
        const val LOAD_FAILED = "llm_load_failed"
        const val TOO_LONG = "llm_too_long"
        const val CANCELLED = "llm_cancelled"
        const val FAILED = "llm_failed"
    }
}
