package dev.demo.nixyon.speech

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class SpeakerNamesTest {
    // Реплики по порядку: (спикер, текст); id реплики: её номер с 1.
    private fun turns(vararg lines: Pair<Int, String>) =
        lines.mapIndexed { i, (speaker, text) -> NamingTurn(i + 1, speaker, text) }

    private fun unnamed(vararg ids: Int) = ids.map { NamingSpeaker(it, null) }

    // Начало встречи из scripts/data/llm-meetings/meeting-1.txt без имён.
    private val meeting = turns(
        1 to "Доброе утро. Давайте быстро по планам на неделю, у нас полчаса.",
        2 to "Доброе. Сразу плохая новость: вчера двое пользователей написали, что приложение падает при импорте файлов m4a с айфона.",
        1 to "Воспроизводится?",
        2 to "Да, на втором тестовом телефоне падает каждый раз. Похоже, декодер не понимает файлы с переменным битрейтом.",
        1 to "Это критично. Борис, возьмёшь? Нужно исправить до пятницы, иначе релиз не выпускаем.",
        2 to "Возьму, до пятницы успею.",
        3 to "Кстати, про второй телефон. Он один на всю команду, и Борис его постоянно забирает. Может, купить ещё один?",
        1 to "Да, давай. Закажи, пожалуйста, ещё один такой же, бюджет до сорока тысяч.",
        3 to "Хорошо, закажу сегодня.",
        1 to "Теперь демо. Показ для заказчика в четверг в три часа. Я сделаю слайды к среде, чтобы было время прогнать.",
        2 to "А Ольга не может помочь со слайдами? Она же делала прошлую презентацию.",
        1 to "Ольга в отпуске до конца месяца, так что нет, не будем её трогать. Сделаю сама.",
    )

    private fun suggest(answer: String, speakers: List<NamingSpeaker>, turns: List<NamingTurn>): List<NameSuggestion> {
        val prompt = SpeakerNames.prompt(speakers, turns)!!
        return SpeakerNames.parse(answer, prompt, speakers, turns)
    }

    @Test
    fun theModelReadsOnlyTurnsWithNamesAndTheirNeighbours() {
        val prompt = SpeakerNames.prompt(unnamed(1, 2, 3), meeting)!!
        assertEquals(
            "Фрагменты расшифровки:\n" +
                "Спикер 2: … Похоже, декодер не понимает файлы с переменным битрейтом.\n" +
                "Спикер 1: … Борис, возьмёшь? …\n" +
                "Спикер 2: Возьму, до пятницы успею.\n" +
                "Спикер 3: … Он один на всю команду, и Борис его постоянно забирает. …\n" +
                "Спикер 1: Да, давай. …\n" +
                "…\n" +
                "Спикер 1: … Я сделаю слайды к среде, чтобы было время прогнать.\n" +
                "Спикер 2: А Ольга не может помочь со слайдами? …\n" +
                "Спикер 1: Ольга в отпуске до конца месяца, так что нет, не будем её трогать. …\n" +
                "\nСпикеры: Спикер 1, Спикер 2, Спикер 3.",
            prompt.user,
        )
        assertEquals(listOf(1, 2, 3), prompt.asked)
        assertEquals(48 + 24 * 3, prompt.maxTokens)
        assertEquals(SpeakerNames.SYSTEM, prompt.system)
    }

    @Test
    fun skippedPartsOfTheMeetingAreMarked() {
        val t = turns(
            1 to "Начнём.",
            2 to "Давайте.",
            1 to "Первый вопрос про бюджет.",
            2 to "Бюджет утвердили.",
            1 to "Хорошо.",
            2 to "Руслан, ты сможешь прислать смету?",
            3 to "Смогу.",
        )
        val user = SpeakerNames.prompt(unnamed(1, 2, 3), t)!!.user
        assertTrue(user, user.startsWith("Фрагменты расшифровки:\nСпикер 1: Хорошо.\nСпикер 2: Руслан, ты сможешь прислать смету?\nСпикер 3: Смогу.\n"))
        val gapped = turns(
            1 to "Анна, начнёшь?",
            2 to "Да.",
            1 to "Отлично.",
            2 to "Про бюджет.",
            1 to "Хорошо.",
            2 to "Руслан, ты сможешь?",
            3 to "Смогу.",
        )
        val lines = SpeakerNames.prompt(unnamed(1, 2, 3), gapped)!!.user.lines()
        assertEquals(listOf("Спикер 1: Анна, начнёшь?", "Спикер 2: Да.", "…", "Спикер 1: Хорошо."), lines.subList(1, 5))
        val skipped = turns(1 to "Да. Про бюджет. Пётр, начнёшь? Время идёт. Ладно.", 2 to "Начну. Спасибо, Анна.")
        assertEquals(
            "Спикер 1: … Пётр, начнёшь? … Ладно.",
            SpeakerNames.prompt(unnamed(1, 2), turns(2 to "Ну.", *skipped.map { it.speaker to it.text }.toTypedArray()))!!.user.lines()[2],
        )
    }

    @Test
    fun withoutAnythingLikeANameTheModelIsNotAsked() {
        val t = turns(
            1 to "Так, ну все подключились? Начинаем.",
            2 to "Да, здравствуйте. Слушаю.",
            1 to "Хорошо, спасибо. Сеть и интернет в новом офисе?",
            3 to "Это на мне, подпишу до пятнадцатого ноября. Купим API ключ для ВТБ.",
        )
        assertNull(SpeakerNames.prompt(unnamed(1, 2, 3), t))
        var called = false
        val found = SpeakerNamer { _, _, _ -> called = true; "" }.suggest(unnamed(1, 2, 3), t)
        assertTrue(found.isEmpty())
        assertFalse(called)
    }

    @Test
    fun whenEveryoneHasANameTheModelIsNotAsked() {
        val speakers = listOf(NamingSpeaker(1, "Анна"), NamingSpeaker(2, "Борис"), NamingSpeaker(3, "Ира"))
        assertNull(SpeakerNames.prompt(speakers, meeting))
        // Пустое имя или пробелы: это «Спикер N».
        val blank = listOf(NamingSpeaker(1, "Анна"), NamingSpeaker(2, " "), NamingSpeaker(3, "Ира"))
        assertEquals(listOf(2), SpeakerNames.prompt(blank, meeting)!!.asked)
    }

    @Test
    fun namedSpeakersAppearUnderTheirNamesAndAreNotAskedAbout() {
        val speakers = listOf(NamingSpeaker(1, "Анна"), NamingSpeaker(2, null), NamingSpeaker(3, null))
        val prompt = SpeakerNames.prompt(speakers, meeting)!!
        assertTrue(prompt.user, prompt.user.contains("\nАнна: … Борис, возьмёшь? …\nСпикер 2: Возьму"))
        assertEquals(listOf(2, 3), prompt.asked)
    }

    @Test
    fun aLongTurnIsCutAroundTheName() {
        val filler = "Мы обсуждали сроки и долго не могли договориться о порядке работ на следующий месяц"
        val long = "$filler, потому что $filler, и в итоге Руслан предложил перенести всё, а $filler и $filler."
        val t = turns(1 to "Начнём.", 2 to long, 1 to "Ясно. $filler. $filler. $filler. $filler.")
        val lines = SpeakerNames.prompt(unnamed(1, 2), t)!!.user.lines()
        val cut = lines[2]
        assertTrue(cut, cut.startsWith("Спикер 2: … ") && cut.endsWith(" …"))
        assertTrue(cut, cut.contains("Руслан предложил"))
        assertTrue(cut, cut.length <= "Спикер 2: ".length + SpeakerNames.MAX_SENTENCE_CHARS + 4)
        val next = lines[3]
        assertTrue(next, next.startsWith("Спикер 1: Ясно. …"))
    }

    @Test
    fun theExcerptStaysWithinItsBudget() {
        val lines = (1..400).flatMap { n ->
            listOf(1 to "Руслан, по пункту $n что скажешь? Нужно решить сегодня.", 2 to "По пункту $n всё готово.")
        }
        val prompt = SpeakerNames.prompt(unnamed(1, 2), turns(*lines.toTypedArray()))!!
        val excerpt = prompt.user.substringAfter("Фрагменты расшифровки:\n").substringBefore("\n\nСпикеры:")
        assertTrue(excerpt.length <= SpeakerNames.MAX_EXCERPT_CHARS)
        assertTrue(excerpt.length > SpeakerNames.MAX_EXCERPT_CHARS - 200)
        assertTrue(excerpt.startsWith("Спикер 1: Руслан, по пункту 1 что скажешь?"))
    }

    @Test
    fun aNameGivenToTheOneWhoAnsweredAnAddressIsSuggested() {
        val found = suggest("Спикер 1: неизвестно\nСпикер 2: Борис\nСпикер 3: неизвестно", unnamed(1, 2, 3), meeting)
        assertEquals(listOf(NameSuggestion(2, "Борис", 5)), found)
    }

    @Test
    fun theNameSomeoneUsesForAnotherIsNotTheirs() {
        // Спикер 1 сам обращается к Борису; Спикер 3 говорит о Борисе в третьем лице.
        assertTrue(suggest("Спикер 1: Борис\nСпикер 3: Борис", unnamed(1, 2, 3), meeting).isEmpty())
    }

    @Test
    fun someoneOnlyTalkedAboutIsNotSuggested() {
        // «А Ольга не может помочь?» и «Ольга в отпуске» не обращения.
        assertTrue(suggest("Спикер 1: Ольга\nСпикер 3: Ольга", unnamed(1, 3), meeting).isEmpty())
    }

    @Test
    fun aNameThatIsNotInTheConversationIsDropped() {
        assertTrue(suggest("Спикер 1: Анна\nСпикер 2: Борис Петров", unnamed(1, 2, 3), meeting).isEmpty())
    }

    @Test
    fun selfIntroductionsCountInTheSpeakersOwnTurn() {
        val t = turns(
            1 to "Добрый день, меня зовут Руслан, я из отдела закупок.",
            2 to "Здравствуйте. Я Мария Петровна, бухгалтер.",
            1 to "Мария Петровна, вам слово.",
            2 to "Спасибо.",
        )
        val found = suggest("Спикер 1: Руслан\nСпикер 2: Мария Петровна", unnamed(1, 2), t)
        assertEquals(listOf(NameSuggestion(1, "Руслан", 1), NameSuggestion(2, "Мария Петровна", 2)), found)
    }

    @Test
    fun aRoundOfIntroductionsCountsButGreetingsAndRequestsDoNot() {
        val t = turns(
            1 to "Давайте познакомимся. Меня зовут Руслан.",
            2 to "Анна \u2014 маркетинг.",
            3 to "Дмитрий, бэкенд. Работаю из Казани.",
            1 to "Игорь, добрый день.",
            4 to "Здравствуйте.",
            1 to "Борис, займись этим.",
            5 to "Хорошо.",
        )
        val speakers = unnamed(1, 2, 3, 4, 5)
        val found = suggest("Спикер 1: Игорь\nСпикер 2: Анна\nСпикер 3: Дмитрий\nСпикер 4: Игорь\nСпикер 5: Борис", speakers, t)
        // «Игорь, добрый день.» подтверждает Игоря тому, кто ответил, а не тому, кто поздоровался.
        assertEquals(
            listOf(NameSuggestion(2, "Анна", 2), NameSuggestion(3, "Дмитрий", 3), NameSuggestion(4, "Игорь", 4), NameSuggestion(5, "Борис", 6)),
            found,
        )
        assertTrue(suggest("Спикер 1: Игорь", speakers, t).isEmpty())
        assertTrue(suggest("Спикер 1: Борис", speakers, t).isEmpty())
    }

    @Test
    fun aNameAloneAsAQuestionIsAnAddress() {
        val t = turns(1 to "Теперь разработка. Кирилл?", 2 to "Релиз переехал на неделю.", 1 to "Воспроизводится?", 2 to "Да.")
        val prompt = SpeakerNames.prompt(unnamed(1, 2), t)!!
        assertTrue(prompt.user, prompt.user.contains("Спикер 1: … Кирилл?\nСпикер 2: Релиз переехал на неделю.\n\n"))
        assertEquals(listOf(NameSuggestion(2, "Кирилл", 1)), SpeakerNames.parse("Спикер 2: Кирилл", prompt, unnamed(1, 2), t))
    }

    @Test
    fun namesInOtherCasesAndHandOversAreRecognised() {
        val t = turns(
            1 to "С цифрами всё. Передаю слово Ольге.",
            2 to "Спасибо. Тогда про маркетинг.",
            1 to "Хорошо, Саш, а что с бюджетом?",
            3 to "Бюджет согласовали.",
        )
        val found = suggest("Спикер 2: Ольга\nСпикер 3: Саша", unnamed(1, 2, 3), t)
        assertEquals(listOf(NameSuggestion(2, "Ольга", 1), NameSuggestion(3, "Саша", 3)), found)
    }

    @Test
    fun thanksGoToThePreviousSpeaker() {
        val t = turns(1 to "Отчёт готов, отправил всем.", 2 to "Спасибо, Игорь. Идём дальше.")
        assertEquals(listOf(NameSuggestion(1, "Игорь", 2)), suggest("Спикер 1: Игорь", unnamed(1, 2), t))
    }

    @Test
    fun oneNameForTwoSpeakersOrAnAlreadyGivenNameIsDropped() {
        val t = turns(
            1 to "Анна, начнёшь?",
            2 to "Да. Анна, а ты что думаешь?",
            3 to "Согласна.",
        )
        assertTrue(suggest("Спикер 2: Анна\nСпикер 3: Анна", unnamed(1, 2, 3), t).isEmpty())
        val speakers = listOf(NamingSpeaker(1, "Анна"), NamingSpeaker(2, null), NamingSpeaker(3, null))
        assertTrue(suggest("Спикер 2: Анна", speakers, t).isEmpty())
    }

    @Test
    fun looseAnswerFormatsAreRead() {
        val answer = """
            Вот что получилось:
            - **Спикер 2**: Борис (к нему обратились по имени).
            спикер 3 ${'\u2014'} Неизвестно
            Спикер 2: Ольга
            Спикер 7: Пётр
            Спикер 1: «неизвестно»
        """.trimIndent()
        assertEquals(listOf(NameSuggestion(2, "Борис", 5)), suggest(answer, unnamed(1, 2, 3), meeting))
        assertTrue(suggest("Спикер 2: это Борис", unnamed(1, 2, 3), meeting).isEmpty())
        assertTrue(suggest("Спикер 2: Спикер 5", unnamed(1, 2, 3), meeting).isEmpty())
        assertTrue(suggest("", unnamed(1, 2, 3), meeting).isEmpty())
    }

    @Test
    fun theModelGetsThePromptAndItsAnswerIsChecked() {
        var asked: Triple<String, String, Int>? = null
        val found = SpeakerNamer { system, user, maxTokens ->
            asked = Triple(system, user, maxTokens)
            "Спикер 1: неизвестно\nСпикер 2: Борис\nСпикер 3: Ольга"
        }.suggest(unnamed(1, 2, 3), meeting)
        assertEquals(listOf(NameSuggestion(2, "Борис", 5)), found)
        val prompt = SpeakerNames.prompt(unnamed(1, 2, 3), meeting)!!
        assertEquals(Triple(prompt.system, prompt.user, prompt.maxTokens), asked)
    }

    @Test
    fun formsOfOneNameMatch() {
        for ((name, word) in listOf("Анна" to "Анну", "Пётр" to "Петра", "Павел" to "Павла", "Саша" to "Саш",
            "Ольга" to "Ольге", "Дмитрий" to "Дмитрия", "Илья" to "Илью", "Игорь" to "Игорю", "Ян" to "Яна")) {
            assertTrue("$name $word", SpeakerNames.sameName(name, word))
        }
        for ((name, word) in listOf("Анна" to "Антон", "Ольга" to "Олег", "Ян" to "Юля", "Игорь" to "Игорьевичами")) {
            assertFalse("$name $word", SpeakerNames.sameName(name, word))
        }
    }

    @Test
    fun emptyTurnsAndOddTextDoNotBreakAnything() {
        val t = turns(1 to "", 2 to "   ", 1 to "Руслан, ты тут?", 2 to "", 3 to "…", 2 to "Тут.")
        val prompt = SpeakerNames.prompt(unnamed(1, 2, 3), t)!!
        assertTrue(prompt.user, prompt.user.contains("Спикер 1: Руслан, ты тут?"))
        // Пустых реплик во фрагментах нет, поэтому про Спикера 2 модель не спрашивали.
        assertEquals(listOf(1), prompt.asked)
        assertEquals(emptyList<NameSuggestion>(), SpeakerNames.parse("Спикер 2: Руслан", prompt, unnamed(1, 2, 3), t))
    }
}
