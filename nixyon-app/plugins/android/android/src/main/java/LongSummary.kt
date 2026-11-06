package dev.demo.nixyon.speech

// Протокол встречи, которая не помещается в контекст модели (MeetingSummarizer.CONTEXT_TOKENS:
// около 10 минут разговора вместе с ответом). Сначала встреча целиком; если модель ответила
// llm_too_long, встреча делится пополам по репликам, пока каждая часть не поместится, у
// каждой части свой протокол, затем протоколы частей сводятся в один тем же запросом
// (system ядра: вид ответа тот же). Если и протоколы частей не помещаются вместе, они
// сводятся по половинам. Проверка длины в C++ идёт до чтения встречи, поэтому лишняя
// попытка стоит только разбивки на токены.
//
// generate: один ответ модели на текст встречи (user) с ходом работы; бросает LlmException.
// Ход работы для интерфейса: пока идут части и сведение читает их, фаза READING от 0 до
// SCALE по всей встрече; ответ сведения как обычно, фаза WRITING.
class LongSummary(private val generate: (user: String, progress: LlmProgress) -> String) {
    // Сколько частей и сведений понадобилось: для лога.
    var parts = 0
        private set
    var combines = 0
        private set

    fun run(user: String, progress: LlmProgress): String {
        val whole = try {
            return generate(user, progress)
        } catch (e: LlmException) {
            if (e.code != LlmException.TOO_LONG) throw e
            e
        }
        val meeting = MeetingText.parse(user)
        val total = meeting.turns.sumOf { it.length }.coerceAtLeast(1)
        val protocols = ArrayList<String>()
        var offset = 0

        fun summarize(turns: List<String>) {
            val size = turns.sumOf { it.length }
            val tooLong = try {
                val answer = generate(meeting.text(turns), partProgress(progress, offset, size, total))
                parts++
                MeetingParts.cleanAnswer(answer).takeIf { it.isNotEmpty() }?.let(protocols::add)
                offset += size
                return
            } catch (e: LlmException) {
                if (e.code != LlmException.TOO_LONG) throw e
                e
            }
            val (first, second) = MeetingParts.halve(turns) ?: throw tooLong
            summarize(first)
            summarize(second)
        }

        val (first, second) = MeetingParts.halve(meeting.turns) ?: throw whole
        summarize(first)
        summarize(second)
        return when (protocols.size) {
            0 -> ""
            1 -> protocols[0]
            else -> combine(meeting.header, protocols, progress, isFinal = true, depth = 0)
        }
    }

    private fun combine(header: String, protocols: List<String>, progress: LlmProgress, isFinal: Boolean, depth: Int): String {
        val tooLong = try {
            val answer = generate(MeetingParts.combineRequest(header, protocols), if (isFinal) combineProgress(progress) else waiting(progress))
            combines++
            return MeetingParts.cleanAnswer(answer)
        } catch (e: LlmException) {
            if (e.code != LlmException.TOO_LONG) throw e
            e
        }
        if (protocols.size < 2 || depth >= MAX_COMBINE_DEPTH) throw tooLong
        val half = protocols.size / 2
        val first = combine(header, protocols.subList(0, half), progress, isFinal = false, depth = depth + 1)
        val second = combine(header, protocols.subList(half, protocols.size), progress, isFinal = false, depth = depth + 1)
        return combine(header, listOf(first, second), progress, isFinal, depth + 1)
    }

    companion object {
        const val SCALE = 1000
        // Доля шкалы на части; остаток на чтение протоколов частей при сведении.
        const val PARTS_SHARE = 950
        private const val MAX_COMBINE_DEPTH = 8

        // Часть с offset символов от начала, size символов из total: чтение части первая
        // половина её доли, ответ вторая.
        fun partProgress(progress: LlmProgress, offset: Int, size: Int, total: Int) = LlmProgress { phase, done, of ->
            val share = if (of > 0) done.toDouble() / of else 0.0
            val within = if (phase == LlmProgress.READING) share / 2 else 0.5 + share / 2
            val value = (PARTS_SHARE * (offset + size * within) / total).toInt().coerceIn(0, PARTS_SHARE)
            progress.onProgress(LlmProgress.READING, value, SCALE)
        }

        fun combineProgress(progress: LlmProgress) = LlmProgress { phase, done, of ->
            if (phase == LlmProgress.READING) {
                val share = if (of > 0) done.toDouble() / of else 0.0
                progress.onProgress(LlmProgress.READING, PARTS_SHARE + ((SCALE - PARTS_SHARE) * share).toInt(), SCALE)
            } else {
                progress.onProgress(phase, done, of)
            }
        }

        // Промежуточное сведение: шкала стоит в конце частей.
        fun waiting(progress: LlmProgress) = LlmProgress { _, _, _ ->
            progress.onProgress(LlmProgress.READING, PARTS_SHARE, SCALE)
        }
    }
}
