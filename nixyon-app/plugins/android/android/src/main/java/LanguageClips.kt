package dev.demo.nixyon.speech

// Отрезки для определения языка и решение по ним; то же правило, что у движка на ноутбуке
// (nixyon-core: language_clips.rs и language_vote.rs).
object LanguageClips {
    // Сколько отрезков записи слушает определение языка.
    const val CLIPS = 5
    // Отрезок не длиннее 30 с при 16 кГц: дальше Whisper не слушает.
    const val MAX_CLIP_SAMPLES = 480_000L

    // Запись делится на CLIPS равных частей по времени, из каждой берётся самый длинный кусок
    // речи (кусок относится к части по своей середине), обрезанный до MAX_CLIP_SAMPLES. Так
    // один разговорчивый участник в начале записи не решает за всю встречу. Часть без речи
    // отрезка не даёт. bounds: куски речи в сэмплах, total: длина записи в сэмплах.
    fun pick(bounds: List<Pair<Long, Long>>, total: Long): List<Pair<Long, Long>> {
        if (total <= 0) return emptyList()
        val best = arrayOfNulls<Pair<Long, Long>>(CLIPS)
        for ((from, to) in bounds) {
            if (to <= from) continue
            val middle = from + (to - from) / 2
            val part = (minOf(middle, total - 1) * CLIPS / total).toInt().coerceAtMost(CLIPS - 1)
            val current = best[part]
            if (current == null || to - from > current.second - current.first) best[part] = from to to
        }
        return best.filterNotNull().map { (from, to) -> from to minOf(to, from + MAX_CLIP_SAMPLES) }
    }

    // Английский, только если за него больше половины отрезков. Иначе, в том числе без
    // отрезков и при ничьей, русский: основной язык приложения, и его модель на чужой речи
    // даёт пометку неразборчивой речи, а не догадку. null в votes: ни русский, ни английский.
    fun vote(votes: List<SpeechLanguage?>): SpeechLanguage {
        val english = votes.count { it == SpeechLanguage.EN }
        return if (english * 2 > votes.size) SpeechLanguage.EN else SpeechLanguage.RU
    }
}
