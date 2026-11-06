package dev.demo.nixyon.speech

// Распознавание группами кусков VAD. Кусок, распознанный отдельно, кончается для модели
// концом записи: GigaAM с пунктуацией ставит в конце точку, а следующий кусок начинает с
// заглавной буквы, и фраза, разрезанная паузой, рвётся на «предложения»; слово на стыке
// модель тоже слышит без соседей. Поэтому соседние куски с короткой паузой распознаются
// вместе, одним отрезком звука вместе с паузой между ними, а слова потом раздаются обратно
// по кускам. Куски для эмбеддингов голоса, окон и нейросегментации остаются прежними:
// меняется только то, сколько звука слышит распознаватель.
// Помогает там, где VAD находит паузы внутри фраз. Куски, которые VAD режет
// принудительно по maxSpeechDuration (около 20 с), в группу уже не помещаются и
// распознаются как раньше, по одному.

// Группы подряд идущих кусков [начало, конец) в сэмплах. В группе пауза между соседними
// кусками не длиннее maxGap, а вся группа, от начала первого куска до конца последнего,
// не длиннее maxLength. Кусок длиннее maxLength идёт один, как раньше.
fun asrGroups(bounds: List<Pair<Long, Long>>, maxLength: Long, maxGap: Long): List<List<Pair<Long, Long>>> {
    val groups = mutableListOf<MutableList<Pair<Long, Long>>>()
    for (piece in bounds) {
        val group = groups.lastOrNull()
        if (group != null &&
            piece.first - group.last().second <= maxGap &&
            piece.second - group.first().first <= maxLength
        ) {
            group.add(piece)
        } else {
            groups.add(mutableListOf(piece))
        }
    }
    return groups
}

// Что распознаётся одним отрезком звука: группы кусков или, если группы выключены, каждый
// кусок отдельно.
fun recognitionGroups(
    bounds: List<Pair<Long, Long>>,
    grouped: Boolean,
    maxLength: Long,
    maxGap: Long,
): List<List<Pair<Long, Long>>> =
    if (grouped) asrGroups(bounds, maxLength, maxGap) else bounds.map { listOf(it) }

// Текст одного куска речи; times: время токенов в секундах от начала куска.
class PieceText(val text: String, val tokens: Array<String>, val times: FloatArray)

// Раздаёт токены распознанной группы по её кускам. times: время токенов в секундах от
// начала группы; pieces: куски группы (начало, конец) в секундах от начала группы.
// Слово уходит в кусок целиком, по времени своего первого токена; граница между соседними
// кусками посередине паузы между ними. Слова выделяются по тем же правилам, что у
// tokens_to_words в nixyon-pipeline: закрывающий знак остаётся при слове перед ним,
// открывающий идёт со словом после него. null, если у токенов нет времени: резать нечем.
fun splitByPieces(tokens: Array<String>, times: FloatArray, pieces: List<Pair<Double, Double>>): List<PieceText>? {
    if (pieces.isEmpty() || tokens.size != times.size) return null
    val owner = IntArray(tokens.size)
    var piece = 0
    var boundary = true
    var opening = false
    var anyWord = false
    for (i in tokens.indices) {
        val token = tokens[i]
        val body = token.trimStart(WORD_MARK, ' ')
        // Начинается ли с этого токена новое слово (или открывающий знак перед словом).
        val startsWord = when {
            body.isEmpty() -> {
                boundary = true
                false
            }
            anyWord && body.all { it in CLOSING } -> {
                boundary = false
                false
            }
            body.all { it in OPENING } -> {
                val first = !opening
                opening = true
                boundary = true
                first
            }
            body.all { it in DASHES } -> {
                val first = !opening
                anyWord = true
                boundary = true
                first
            }
            else -> {
                val starts = boundary || body.length != token.length || !anyWord
                // После открывающего знака слово уже начато на нём.
                val first = starts && !opening
                boundary = false
                opening = false
                anyWord = true
                first
            }
        }
        if (startsWord) {
            while (piece + 1 < pieces.size && times[i] >= (pieces[piece].second + pieces[piece + 1].first) / 2) {
                piece += 1
            }
        }
        owner[i] = piece
    }
    return pieces.mapIndexed { k, (start, end) ->
        val picked = tokens.indices.filter { owner[it] == k }
        PieceText(
            text = picked.joinToString("") { tokens[it] }.replace(WORD_MARK, ' ').trim(),
            tokens = Array(picked.size) { tokens[picked[it]] },
            times = FloatArray(picked.size) { (times[picked[it]] - start).coerceIn(0.0, end - start).toFloat() },
        )
    }
}

// Записаны кодами, чтобы в исходнике не было типографских символов.
private const val WORD_MARK = '▁'
private val CLOSING = setOf('.', ',', '!', '?', ':', ';', ')', ']', '»', '…')
private val OPENING = setOf('(', '[', '«', '"', '„')
private val DASHES = setOf('\u2014', '–')
