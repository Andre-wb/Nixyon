package dev.demo.nixyon.speech

// Текст встречи так, как его даёт ядро для протокола (TXT-экспорт, nixyon-render render_txt):
// шапка (название, длительность, участники), затем реплики через пустую строку, у каждой
// первая строка «кто, когда», вторая текст. Часть встречи: та же шапка и часть реплик.
class MeetingText(val header: String, val turns: List<String>) {
    fun text(part: List<String> = turns): String {
        val body = part.joinToString("\n\n")
        return if (header.isEmpty()) "$body\n" else "$header\n\n$body\n"
    }

    companion object {
        fun parse(text: String): MeetingText {
            val blocks = text.replace("\r\n", "\n").trim('\n').split("\n\n").map { it.trim('\n') }.filter { it.isNotEmpty() }
            if (blocks.size < 2) return MeetingText("", blocks)
            return MeetingText(blocks[0], blocks.drop(1))
        }
    }
}

object MeetingParts {
    // Короче этого реплика не режется: кусок без контекста модели не поможет.
    const val MIN_SPLIT_CHARS = 400

    // Две половины примерно поровну по символам: по границе реплик, а одна длинная
    // реплика режется по пробелу у середины, первая строка («кто, когда») идёт в обе.
    // null: резать нечего.
    fun halve(turns: List<String>): Pair<List<String>, List<String>>? {
        if (turns.size >= 2) {
            val total = turns.sumOf { it.length }
            var best = 1
            var bestGap = Int.MAX_VALUE
            var prefix = 0
            for (k in 1 until turns.size) {
                prefix += turns[k - 1].length
                val gap = Math.abs(2 * prefix - total)
                if (gap < bestGap) {
                    best = k
                    bestGap = gap
                }
            }
            return turns.subList(0, best) to turns.subList(best, turns.size)
        }
        val turn = turns.singleOrNull() ?: return null
        val newline = turn.indexOf('\n')
        val head = if (newline >= 0) turn.substring(0, newline + 1) else ""
        val body = turn.substring(head.length)
        if (body.length < MIN_SPLIT_CHARS) return null
        val middle = body.length / 2
        val left = body.lastIndexOf(' ', middle)
        val right = body.indexOf(' ', middle)
        val at = when {
            left <= 0 && right < 0 -> return null
            left <= 0 -> right
            right < 0 -> left
            middle - left <= right - middle -> left
            else -> right
        }
        return listOf(head + body.substring(0, at).trimEnd()) to listOf(head + body.substring(at).trimStart())
    }

    // Запрос на сведение протоколов частей в один. Инструкция и вид ответа остаются от
    // протокола (system из ядра), здесь только что делать с частями.
    fun combineRequest(header: String, protocols: List<String>): String {
        val sb = StringBuilder()
        if (header.isNotEmpty()) sb.append(header).append("\n\n")
        sb.append(
            "Встреча длинная, поэтому протокол составлен по частям. Ниже протоколы частей по порядку. " +
                "Составь из них один протокол всей встречи в том же виде: краткое содержание всей встречи, " +
                "все решения и все задачи без повторов.",
        )
        protocols.forEachIndexed { i, protocol ->
            sb.append("\n\nПротокол части ").append(i + 1).append(" из ").append(protocols.size).append(":\n").append(protocol)
        }
        return sb.append('\n').toString()
    }

    // Ответ без блока рассуждений, если модель его написала, и без пустых краёв, как
    // clean_answer в ядре.
    fun cleanAnswer(raw: String): String = raw.substringAfterLast("</think>").trim()
}
