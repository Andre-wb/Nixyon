package dev.demo.nixyon.speech

import android.content.Context
import android.util.Log

// Протокол встречи языковой моделью; той же моделью SpeakerNamer подсказывает имена
// участников. Где лежит модель, решает LlmLocator. Модель грузится на один запрос и сразу
// выгружается: 2,3 ГБ в памяти между запросами держать незачем. Запрос один за раз.
// Протокол длинной встречи идёт по частям (LongSummary): модель грузится один раз на все
// части и сведение. Если модели не хватает памяти, freeMemory отпускает модели речи
// (SpeechEngine.unload, если движок свободен), и память проверяется ещё раз.
class MeetingSummarizer(private val context: Context, private val freeMemory: () -> Boolean) {
    private val locator = LlmLocator(context)
    private val lock = Any()
    private var running = false
    private var handle = 0L
    private var cancelled = false

    fun isAvailable(): Boolean = locator.isAvailable()

    // Один ответ модели: подсказка имён.
    fun summarize(system: String, user: String, maxTokens: Int, progress: LlmProgress): String =
        withModel { opened -> generate(opened, system, user, maxTokens, progress) }

    // Протокол встречи любой длины. parts: сколько частей понадобилось (1: встреча целиком).
    fun summarizeMeeting(system: String, user: String, maxTokens: Int, progress: LlmProgress): MeetingSummary =
        withModel { opened ->
            val long = LongSummary { text, p -> generate(opened, system, text, maxTokens, p) }
            val text = long.run(user, progress)
            MeetingSummary(text, long.parts.coerceAtLeast(1), long.combines)
        }

    private fun <T> withModel(block: (handle: Long) -> T): T {
        synchronized(lock) {
            if (running) throw LlmException(LlmException.BUSY, "модель уже отвечает на другой запрос")
            running = true
            cancelled = false
        }
        try {
            val opened = locator.locate().use { model ->
                checkMemory(model.size)
                MeetingLlmNative.nativeOpen(model.fd, model.offset, CONTEXT_TOKENS, threads())
            }
            synchronized(lock) {
                handle = opened
                if (cancelled) MeetingLlmNative.nativeCancel(opened)
            }
            try {
                return block(opened)
            } finally {
                synchronized(lock) { handle = 0L }
                MeetingLlmNative.nativeClose(opened)
            }
        } finally {
            synchronized(lock) { running = false }
        }
    }

    // Отмена между частями длинной встречи срабатывает до следующего запроса к модели.
    private fun generate(opened: Long, system: String, user: String, maxTokens: Int, progress: LlmProgress): String {
        if (synchronized(lock) { cancelled }) throw LlmException(LlmException.CANCELLED, "отменено")
        val bytes = MeetingLlmNative.nativeGenerate(opened, system, user, maxTokens, progress)
        return String(bytes, Charsets.UTF_8)
    }

    // Отмена до загрузки модели сработает сразу после неё.
    fun cancel() {
        synchronized(lock) {
            cancelled = true
            if (handle != 0L) MeetingLlmNative.nativeCancel(handle)
        }
    }

    private fun checkMemory(modelBytes: Long) {
        val enough = { memory: MemorySnapshot -> LlmMemory.isEnough(memory.availableBytes, memory.lowMemory, modelBytes) }
        val (memory, freed) = DeviceMemory.check({ DeviceMemory.read(context) }, enough, freeMemory)
        if (freed) Log.i(TAG, "для языковой модели отпущены модели речи: свободно ${memory.availableBytes shr 20} МБ")
        if (!enough(memory)) {
            throw LlmException(
                LlmException.NO_MEMORY,
                "свободно ${memory.availableBytes shr 20} МБ, нужно ${(modelBytes + LlmMemory.HEADROOM_BYTES) shr 20} МБ",
            )
        }
    }

    private fun threads(): Int = Runtime.getRuntime().availableProcessors().coerceIn(1, MAX_THREADS)

    companion object {
        private const val TAG = "NixyonSpeech"
        // Контекст 4096 токенов: встреча и ответ должны уложиться в него вместе.
        const val CONTEXT_TOKENS = 4096
        // Не больше 4 потоков, как в примере llama.cpp для Android: у телефонов часть ядер медленные.
        private const val MAX_THREADS = 4
    }
}

class MeetingSummary(val text: String, val parts: Int, val combines: Int)
