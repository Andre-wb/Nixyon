package dev.demo.nixyon.speech

// Реплика и участник последней расшифровки, как их присылает ядро. name: имя, которое уже
// дал пользователь; null, если участник подписан «Спикер N».
class NamingTurn(val id: Int, val speaker: Int, val text: String)

class NamingSpeaker(val id: Int, val name: String?)

// Подсказка: участника speaker, похоже, зовут name. turn: реплика, из которой это видно,
// чтобы интерфейс показал её рядом с подсказкой.
data class NameSuggestion(val speaker: Int, val name: String, val turn: Int)

// Запрос к языковой модели. asked: про каких участников спрашиваем.
class NamesPrompt(val system: String, val user: String, val maxTokens: Int, val asked: List<Int>)

// Имена участников из самого разговора: кто представился («меня зовут Анна») или к кому
// обратились по имени («Руслан, сможешь?»), а он ответил. Модель читает не всю встречу, а
// только реплики со словами, похожими на имена, и соседние с ними: запрос короткий, и на
// телефоне это секунды, а не минуты, как у протокола. Нет таких слов: модель не грузится.
// Ответ модели проверяется: имя звучит в разговоре; это самопредставление участника или
// обращение к нему в соседней с его ответом реплике; одно имя не достаётся двоим и не
// совпадает с уже данным. Подсказки только предлагаются, имя даёт пользователь.
class SpeakerNamer(private val generate: (system: String, user: String, maxTokens: Int) -> String) {
    fun suggest(speakers: List<NamingSpeaker>, turns: List<NamingTurn>): List<NameSuggestion> {
        val prompt = SpeakerNames.prompt(speakers, turns) ?: return emptyList()
        return SpeakerNames.parse(generate(prompt.system, prompt.user, prompt.maxTokens), prompt, speakers, turns)
    }
}

object SpeakerNames {
    // Инструкция по-русски, как у протокола: модель замерялась на русских запросах.
    const val SYSTEM =
        "Ты определяешь имена участников встречи по фрагментам её расшифровки. Каждая строка: кто говорит, " +
            "двоеточие, что он сказал. Участники, чьи имена неизвестны, подписаны «Спикер N»; «…» отмечает " +
            "пропущенные места разговора.\n" +
            "Имя участника известно, только если он сам представился («меня зовут Анна», «я Анна») или к нему " +
            "обратились по имени («Анна, сможешь?», «Спасибо, Анна») и по соседним репликам ясно, что обращались " +
            "именно к нему. Имя, которым участник обращается к другому, принадлежит не ему. Тот, о ком говорят " +
            "в третьем лице, может и не участвовать во встрече. Не угадывай: если имя не следует из фрагментов " +
            "однозначно, пиши «неизвестно».\n" +
            "Ответ строго по строке на каждого спикера из списка, без пояснений: «Спикер N: Имя» или " +
            "«Спикер N: неизвестно». Имя пиши в именительном падеже, как участника называют в разговоре."

    // Предел фрагментов в символах: около 1 500 токенов, вместе с инструкцией и ответом
    // запрос остаётся вдвое меньше контекста модели (MeetingSummarizer.CONTEXT_TOKENS).
    const val MAX_EXCERPT_CHARS = 4_000
    // Длинное предложение с именем режется вокруг имени, соседнее с ним режется по краю.
    const val MAX_SENTENCE_CHARS = 240
    const val CONTEXT_CHARS = 160
    // Больше этого от одной реплики не берётся, сколько бы имён в ней ни было.
    const val MAX_TURN_CHARS = 600
    const val SPEAKER_LABEL = "Спикер"

    private val WORD = Regex("""\p{L}+(?:[-'’]\p{L}+)*""")
    private val SENTENCE_BREAK = Regex("""(?<=[.!?…])\s+""")
    private val NAME_WORD = Regex("""\p{Lu}\p{Ll}+(?:-\p{Lu}?\p{Ll}+)*""")
    private val ANSWER_LINE = Regex("""^\s*(?:[-–•]\s*)?спикер\s*(\d+)\s*[:：\u2014–-]\s*(.*)$""", RegexOption.IGNORE_CASE)
    private val NAME_END = Regex("""\s[\u2014–-]\s|[(,;.!?\[]""")

    // Слова, которыми часто начинают реплику с запятой после них: «Да, давай», «Слушай, …».
    // С большой буквы и с запятой они похожи на обращение по имени, но это не имена.
    private val NOT_NAMES = setOf(
        "ага", "алло", "ах", "безусловно", "блин", "вот", "впрочем", "всем", "вообще", "все", "всё",
        "давай", "давайте", "действительно", "договорились", "друзья", "думаю", "естественно", "здравствуйте",
        "значит", "или", "извини", "извините", "итак", "итого", "кажется", "коллеги", "конечно",
        "короче", "кстати", "круто", "ладно", "мм", "минус", "может", "наверное", "например", "народ", "нет",
        "ну", "ой", "окей", "ок", "отлично", "плюс", "погоди", "подожди", "пожалуйста", "поехали",
        "понял", "поняла", "понятно", "похоже", "потом", "правда", "привет", "принято", "простите", "прости",
        "ребята", "разумеется", "сейчас", "сначала", "слушай", "слушайте", "смотри", "смотрите", "согласен",
        "согласна", "спасибо", "так", "тогда", "точно", "угу", "уважаемые", "хм", "хорошо", "честно", "это",
        "да", "эх", "ясно", "супер", "класс", "секунду", "минуту", "видимо", "в-общем", "во-первых",
        "во-вторых", "в-третьих", "кроме", "однако", "зато", "причём", "причем", "хотя", "если", "когда",
        "теперь", "здесь", "тут", "там", "сегодня", "завтра", "вчера", "утром", "вечером", "интересно",
        "странно", "верно", "возможно", "товарищи", "господа", "команда", "всё-таки", "как", "зачем", "куда",
        "откуда", "какой", "какая", "чей", "чья", "кто", "что", "сколько", "серьёзно", "серьезно",
    )
    private const val NAME_ENDINGS = "бвгджзйклмнпрстфхцчшщьая"
    private val NOT_INTRO = setOf(
        "добрый", "доброе", "доброй", "здравствуй", "здравствуйте", "привет", "приветствую", "спасибо", "благодарю",
        "пожалуйста", "извини", "извините", "слушай", "слушайте", "давай", "давайте", "ты", "вы", "тебе", "вам",
        "тебя", "вас", "твой", "ваш", "твоя", "ваша",
    )
    private val HAND_OVER = setOf("слово", "передаю")
    private val SELF_INTRO = setOf("зовут", "звать", "я", "это")
    private val REFUSALS = setOf("неизвестно", "неизвестен", "неизвестна", "нет", "никто", "не", "спикер", "участник", "имя")

    fun prompt(speakers: List<NamingSpeaker>, turns: List<NamingTurn>): NamesPrompt? {
        val names = given(speakers)
        if (speakers.none { names[it.id] == null }) return null
        val parsed = turns.map { Turn(it) }
        val keep = parsed.map { sortedMapOf<Int, Keep>() }
        for (i in parsed.indices) {
            val found = parsed[i].sentences.withIndex().filter { (_, s) -> s.words.any { it.candidate } }
            if (found.isEmpty()) continue
            for ((k, _) in found) keep[i][k] = Keep.NAME
            for (j in before(parsed, i)) {
                if (parsed[j].sentences.isNotEmpty()) keep[j].putIfAbsent(parsed[j].sentences.lastIndex, Keep.TAIL)
            }
            for (j in after(parsed, i)) {
                if (parsed[j].sentences.isNotEmpty()) keep[j].putIfAbsent(0, Keep.HEAD)
            }
        }

        val lines = mutableListOf<String>()
        val asked = sortedSetOf<Int>()
        var size = 0
        var previous = -2
        // Реплики по порядку, пока не кончится предел; пропуски между ними отмечены «…».
        for (i in keep.indices) {
            if (keep[i].isEmpty()) continue
            val line = "${label(turns[i].speaker, names)}: ${excerpt(parsed[i], keep[i])}"
            val gap = previous >= 0 && i > previous + 1
            val add = line.length + 1 + if (gap) 2 else 0
            if (size + add > MAX_EXCERPT_CHARS) break
            if (gap) lines += "…"
            lines += line
            size += add
            previous = i
            if (names[turns[i].speaker] == null) asked += turns[i].speaker
        }
        if (asked.isEmpty()) return null
        val user = "Фрагменты расшифровки:\n" + lines.joinToString("\n") +
            "\n\nСпикеры: " + asked.joinToString(", ") { "$SPEAKER_LABEL $it" } + "."
        return NamesPrompt(SYSTEM, user, minOf(320, 48 + 24 * asked.size), asked.toList())
    }

    fun parse(answer: String, prompt: NamesPrompt, speakers: List<NamingSpeaker>, turns: List<NamingTurn>): List<NameSuggestion> {
        val proposed = linkedMapOf<Int, String?>()
        for (raw in answer.lines()) {
            val line = raw.replace(Regex("""[*_«»"“”]"""), "")
            val match = ANSWER_LINE.find(line) ?: continue
            val id = match.groupValues[1].toIntOrNull() ?: continue
            if (id !in prompt.asked || id in proposed) continue
            proposed[id] = cleanName(match.groupValues[2])
        }

        val parsed = turns.map { Turn(it) }
        val taken = given(speakers).values.filterNotNull().map { key(it.split(' ').first()) }.toSet()
        val supported = proposed.mapNotNull { (id, name) ->
            if (name == null || key(name.split(' ').first()) in taken) return@mapNotNull null
            val words = name.split(' ')
            // Каждое слово имени должно звучать в разговоре: фамилию модель не придумывает.
            if (words.any { part -> parsed.none { t -> t.words.any { it.nameLike && sameName(part, it.text) } } }) {
                return@mapNotNull null
            }
            // Полное имя могло прозвучать по частям: тогда подтверждением служит первое слово.
            val turn = evidence(parsed, id, words) ?: if (words.size > 1) evidence(parsed, id, words.take(1)) else null
            turn?.let { NameSuggestion(id, name, it) }
        }
        val twice = supported.groupBy { key(it.name.split(' ').first()) }.filterValues { it.size > 1 }.keys
        return supported.filter { key(it.name.split(' ').first()) !in twice }.sortedBy { it.speaker }
    }

    // Имя из строки ответа: до скобки, тире или знака препинания, от одного до трёх слов
    // с большой буквы. «Неизвестно» и всё, что не похоже на имя, даёт null.
    private fun cleanName(value: String): String? {
        val head = NAME_END.find(value)?.let { value.substring(0, it.range.first) } ?: value
        val words = head.trim().split(Regex("""\s+""")).filter { it.isNotEmpty() }
        if (words.isEmpty() || words.size > 3) return null
        if (words.first().lowercase() in REFUSALS) return null
        if (words.any { !NAME_WORD.matches(it) }) return null
        return words.joinToString(" ")
    }

    // Реплика, подтверждающая имя участника id: он сам представился, или к нему обратились
    // по имени в реплике рядом с его собственной. Обращение адресовано следующему, кто
    // заговорит, а предыдущему только в первом предложении реплики («Да, Наталья,
    // здравствуйте.») или с благодарностью («…Спасибо, Кирилл.»): в «Дим, это к Светлане.
    // Светлана, успеешь?» Светлана: та, кто ответит, а не тот, кто спрашивал. null:
    // подтверждения нет.
    private fun evidence(parsed: List<Turn>, id: Int, name: List<String>): Int? {
        for ((i, turn) in parsed.withIndex()) {
            for (mention in mentions(turn, name)) {
                val self = turn.speaker == id && mention.selfIntro
                val near = turn.speaker != id && mention.address &&
                    (after(parsed, i).any { parsed[it].speaker == id } ||
                        (mention.backward && before(parsed, i).any { parsed[it].speaker == id }))
                if (self || near) return turn.id
            }
        }
        return null
    }

    private class Mention(val selfIntro: Boolean, val address: Boolean, val backward: Boolean)

    // Места, где в реплике звучит имя (все его слова подряд), и как оно звучит.
    private fun mentions(turn: Turn, name: List<String>): List<Mention> {
        val words = turn.words
        val out = mutableListOf<Mention>()
        for (k in words.indices) {
            if (k + name.size > words.size) break
            if (name.indices.any { n -> !words[k + n].nameLike || !sameName(name[n], words[k + n].text) }) continue
            val first = words[k]
            val last = words[k + name.size - 1]
            val selfIntro = (k > 0 && words[k - 1].text.lowercase() in SELF_INTRO) || (k == 0 && roundIntro(turn, last))
            val after = turn.text.substring(last.end).trimStart().firstOrNull()
            val before = turn.text.substring(0, first.start).trimEnd().lastOrNull()
            // Обращение: имя отделено знаками с обеих сторон: «Анна, …», «…, Анна?»,
            // «Спасибо, Анна.», «Кирилл?»; или слово передают: «Передаю слово Ольге.».
            // «А Ольга не может помочь?» и «обсудить с Ниной Сергеевной, она…» не обращения.
            val apart = (before == null || before in ",.!?…:\u2014–") && (after == null || after in ",.!?…")
            val handOver = k > 0 && words[k - 1].text.lowercase() in HAND_OVER
            val sentence = turn.sentences[first.sentence]
            val backward = first.sentence == 0 ||
                sentence.words.any { it.text.lowercase() == "спасибо" || it.text.lowercase() == "благодарю" }
            out += Mention(selfIntro, apart || handOver, backward && !handOver)
        }
        return out
    }

    // Знакомство по кругу: реплика начинается с имени, а за ним коротко и не вопросом,
    // «Дмитрий, бэкенд.», «Анна» и тире перед «маркетинг.». Приветствие, благодарность, «ты», «вы» или
    // повелительный глагол значат обращение к другому: «Игорь, добрый день.», «Борис,
    // займись.», «Борис, возьмёшь?».
    private fun roundIntro(turn: Turn, name: Word): Boolean {
        val sentence = turn.sentences.first()
        if (sentence.words.size > 4 || sentence.text.trimEnd().endsWith("?")) return false
        if (turn.text.substring(name.end).trimStart().firstOrNull() !in listOf(',', '.', '\u2014', '–', null)) return false
        return sentence.words.none { w ->
            val word = w.text.lowercase()
            word in NOT_INTRO || word.endsWith("ись") || word.endsWith("ите") || word.endsWith("йте")
        }
    }

    // Одно ли это имя в разных падежах: «Анна» и «Анну», «Пётр» и «Петра», «Павел» и
    // «Павла», «Саша» и «Саш». Совпадает начало слова без последних одной-двух букв.
    fun sameName(name: String, word: String): Boolean {
        val a = key(name)
        val b = key(word)
        if (a == b) return true
        if (kotlin.math.abs(a.length - b.length) > 3) return false
        val need = maxOf(2, if (a.length <= 4) a.length - 1 else a.length - 2)
        return a.commonPrefixWith(b).length >= minOf(need, a.length)
    }

    private fun key(word: String) = word.lowercase().replace('ё', 'е')

    private fun given(speakers: List<NamingSpeaker>): Map<Int, String?> =
        speakers.associate { it.id to it.name?.trim()?.takeIf(String::isNotEmpty) }

    private fun label(speaker: Int, names: Map<Int, String?>) = names[speaker] ?: "$SPEAKER_LABEL $speaker"

    // Соседние реплики другого участника: ответ на обращение и то, на что отвечают. Если
    // соседняя реплика того же участника (речь разрезана), берётся и следующая за ней.
    private fun after(parsed: List<Turn>, i: Int): List<Int> = neighbours(parsed, i, +1)

    private fun before(parsed: List<Turn>, i: Int): List<Int> = neighbours(parsed, i, -1)

    private fun neighbours(parsed: List<Turn>, i: Int, step: Int): List<Int> {
        val out = mutableListOf<Int>()
        var j = i + step
        while (j in parsed.indices && out.size < 2) {
            out += j
            if (parsed[j].speaker != parsed[i].speaker) break
            j += step
        }
        return if (out.isNotEmpty() && parsed[out.last()].speaker != parsed[i].speaker) out else emptyList()
    }

    private enum class Keep { NAME, HEAD, TAIL }

    // Нужные предложения реплики по порядку; пропуски отмечены «…».
    private fun excerpt(turn: Turn, keep: Map<Int, Keep>): String {
        val out = StringBuilder()
        var last = -1
        for ((k, how) in keep) {
            val sentence = turn.sentences[k]
            val text = when (how) {
                Keep.NAME -> around(sentence)
                Keep.HEAD -> head(sentence.text, CONTEXT_CHARS)
                Keep.TAIL -> tail(sentence.text, CONTEXT_CHARS)
            }
            val gap = if (k > last + 1) "… " else ""
            val piece = (if (out.isNotEmpty()) " " else "") + gap + text
            if (out.isNotEmpty() && out.length + piece.length > MAX_TURN_CHARS) break
            out.append(piece)
            last = k
        }
        if (last < turn.sentences.lastIndex) out.append(" …")
        return out.toString()
    }

    private fun around(sentence: Sentence): String {
        val text = sentence.text
        if (text.length <= MAX_SENTENCE_CHARS) return text
        val at = sentence.words.first { it.candidate }.start
        val from = maxOf(0, minOf(at - MAX_SENTENCE_CHARS / 3, text.length - MAX_SENTENCE_CHARS))
        val start = if (from == 0) 0 else text.indexOf(' ', from).let { if (it in from until at) it + 1 else from }
        val end = minOf(text.length, start + MAX_SENTENCE_CHARS)
        val cut = if (end == text.length) end else text.lastIndexOf(' ', end).let { if (it > at) it else end }
        return (if (start > 0) "… " else "") + text.substring(start, cut) + (if (cut < text.length) " …" else "")
    }

    private fun head(text: String, max: Int): String {
        if (text.length <= max) return text
        val cut = text.lastIndexOf(' ', max).let { if (it > 0) it else max }
        return text.substring(0, cut) + " …"
    }

    private fun tail(text: String, max: Int): String {
        if (text.length <= max) return text
        val from = text.length - max
        val cut = text.indexOf(' ', from).let { if (it in from until text.length - 1) it + 1 else from }
        return "… " + text.substring(cut)
    }

    private class Word(
        val text: String,
        val start: Int,
        val end: Int,
        val nameLike: Boolean,
        val candidate: Boolean,
        val sentence: Int = 0,
    )

    private class Sentence(val text: String, val words: List<Word>)

    // Реплика, разобранная на предложения и слова. Слово похоже на имя, если начинается
    // с большой буквы и дальше в нём есть строчные (не аббревиатура). Кандидат в имена:
    // такое слово не в начале предложения, или в начале, но с запятой после него, как
    // обращение («Борис, возьмёшь?»). В начале предложения с запятой бывает и «Да,»,
    // «Слушай,», «Возьму,», «Закажи,»: такие слова отсеивает список NOT_NAMES, а глаголы
    // и наречия отсеивает окончание. Русские имена и обращения («Саш») кончаются на согласную,
    // «а», «я», «й» или «ь», а «Закажи», «Возьму», «Хорошо», «Давайте» нет.
    private class Turn(source: NamingTurn) {
        val id = source.id
        val speaker = source.speaker
        val text = source.text
        val sentences: List<Sentence>
        val words: List<Word>

        init {
            val all = mutableListOf<Word>()
            val list = mutableListOf<Sentence>()
            var from = 0
            for (piece in SENTENCE_BREAK.split(text)) {
                val at = text.indexOf(piece, from)
                from = at + piece.length
                if (piece.isBlank()) continue
                val words = WORD.findAll(piece).mapIndexed { k, m ->
                    val w = m.value
                    val nameLike = w.length >= 2 && w[0].isUpperCase() && w.drop(1).any { it.isLowerCase() }
                    val next = piece.substring(m.range.last + 1).trimStart().firstOrNull()
                    // «Кирилл?» отдельным предложением тоже обращение, «Воспроизводится?» нет.
                    val alone = k == 0 && (next == '?' || next == '!') && WORD.findAll(piece).count() == 1 &&
                        !w.endsWith("ся") && !w.endsWith("сь")
                    val candidate = nameLike && (k > 0 || ((next == ',' || alone) &&
                        w.last().lowercaseChar() in NAME_ENDINGS && w.lowercase() !in NOT_NAMES))
                    Word(w, m.range.first, m.range.last + 1, nameLike, candidate)
                }.toList()
                list += Sentence(piece, words)
                words.mapTo(all) { Word(it.text, it.start + at, it.end + at, it.nameLike, it.candidate, list.lastIndex) }
            }
            sentences = list
            words = all
        }
    }
}
