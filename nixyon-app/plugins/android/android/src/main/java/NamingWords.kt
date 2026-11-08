package dev.demo.nixyon.speech

// Слова языка встречи, по которым SpeakerNames ищет имена и разбирает ответ модели.
// nameEndings: на что кончается имя в начале предложения с запятой; null: проверки нет,
// английские имена кончаются на что угодно. inflected: имя склоняется («Анну», «Петра»),
// иначе сравнивается целиком, без притяжательного «'s».
class NamingWords(
    val system: String,
    val speakerLabel: String,
    val fragmentsHeader: String,
    val speakersHeader: String,
    val notNames: Set<String>,
    val nameEndings: String?,
    val notNamesMidSentence: Boolean,
    val reflexiveEndings: List<String>,
    val notIntro: Set<String>,
    val imperativeEndings: List<String>,
    val handOver: Set<String>,
    val selfIntro: Set<String>,
    val thanks: Set<String>,
    val refusals: Set<String>,
    val inflected: Boolean,
) {
    companion object {
        // Инструкция по-русски, как у протокола: модель замерялась на русских запросах.
        const val SYSTEM_RU =
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

        // Перевод русской инструкции для английских встреч.
        const val SYSTEM_EN =
            "You identify the names of meeting participants from excerpts of its transcript. Each line: who is " +
                "speaking, a colon, what they said. Participants whose names are unknown are labelled \"Speaker N\"; " +
                "\"…\" marks skipped parts of the conversation.\n" +
                "A participant's name is known only if they introduced themselves (\"my name is Anna\", \"I'm Anna\") " +
                "or were addressed by name (\"Anna, can you?\", \"Thanks, Anna\") and the neighbouring lines make it " +
                "clear that they were the one addressed. A name a participant uses to address someone else is not " +
                "their own. Someone talked about in the third person may not be in the meeting. Do not guess: if the " +
                "name does not follow from the excerpts unambiguously, write \"unknown\".\n" +
                "Answer strictly with one line per speaker from the list, without explanations: \"Speaker N: Name\" " +
                "or \"Speaker N: unknown\". Write the name as the participant is called in the conversation."

        // Русские слова, которыми часто начинают реплику с запятой после них: «Да, давай»,
        // «Слушай, …». С большой буквы и с запятой они похожи на обращение по имени.
        private val RU = NamingWords(
            system = SYSTEM_RU,
            speakerLabel = "Спикер",
            fragmentsHeader = "Фрагменты расшифровки:",
            speakersHeader = "Спикеры:",
            notNames = setOf(
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
            ),
            nameEndings = "бвгджзйклмнпрстфхцчшщьая",
            notNamesMidSentence = false,
            reflexiveEndings = listOf("ся", "сь"),
            notIntro = setOf(
                "добрый", "доброе", "доброй", "здравствуй", "здравствуйте", "привет", "приветствую", "спасибо",
                "благодарю", "пожалуйста", "извини", "извините", "слушай", "слушайте", "давай", "давайте", "ты", "вы",
                "тебе", "вам", "тебя", "вас", "твой", "ваш", "твоя", "ваша",
            ),
            imperativeEndings = listOf("ись", "ите", "йте"),
            handOver = setOf("слово", "передаю"),
            selfIntro = setOf("зовут", "звать", "я", "это"),
            thanks = setOf("спасибо", "благодарю"),
            refusals = setOf("неизвестно", "неизвестен", "неизвестна", "нет", "никто", "не", "спикер", "участник", "имя"),
            inflected = true,
        )

        // Английские слова с большой буквы, которые не имена: начало реплики с запятой
        // («Yes, sure», «Well, …»), а в середине предложения «I'm», дни и месяцы.
        private val EN = NamingWords(
            system = SYSTEM_EN,
            speakerLabel = "Speaker",
            fragmentsHeader = "Transcript excerpts:",
            speakersHeader = "Speakers:",
            notNames = setOf(
                "yes", "yeah", "yep", "no", "nope", "ok", "okay", "well", "so", "now", "right", "sure", "fine",
                "great", "good", "cool", "nice", "thanks", "thank", "please", "sorry", "hi", "hello", "hey", "guys",
                "everyone", "folks", "team", "listen", "look", "actually", "basically", "anyway", "also", "and",
                "but", "or", "then", "first", "second", "third", "finally", "honestly", "really", "maybe", "perhaps",
                "alright", "absolutely", "exactly", "indeed", "um", "uh", "hmm", "oh", "ah", "wow", "today",
                "tomorrow", "yesterday", "what", "who", "why", "where", "when", "how", "which", "if", "the", "a",
                "i'm", "i’m", "i'll", "i’ll", "i've", "i’ve", "i'd", "i’d", "monday", "tuesday", "wednesday",
                "thursday", "friday", "saturday", "sunday", "january", "february", "march", "april", "may", "june",
                "july", "august", "september", "october", "november", "december", "english", "russian",
            ),
            nameEndings = null,
            notNamesMidSentence = true,
            reflexiveEndings = emptyList(),
            notIntro = setOf(
                "hi", "hello", "hey", "good", "morning", "afternoon", "evening", "thanks", "thank", "please", "sorry",
                "you", "your", "can", "could", "would", "will",
            ),
            imperativeEndings = emptyList(),
            handOver = emptySet(),
            selfIntro = setOf("is", "i'm", "i’m", "im", "me", "name's", "name’s"),
            thanks = setOf("thanks", "thank"),
            refusals = setOf("unknown", "none", "no", "nobody", "not", "speaker", "participant", "name"),
            inflected = false,
        )

        fun of(language: SpeechLanguage): NamingWords = when (language) {
            SpeechLanguage.RU -> RU
            SpeechLanguage.EN -> EN
        }
    }
}
