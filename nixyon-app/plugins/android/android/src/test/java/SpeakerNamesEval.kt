package dev.demo.nixyon.speech

import org.junit.Assert.assertEquals
import org.junit.Assume.assumeTrue
import org.junit.Test
import java.net.HttpURLConnection
import java.net.URI

// Подсказка имён на тестовых встречах. Синтетические разговоры, написаны для замера.
//
// Без модели проверяется только код вокруг неё: правильный ответ проходит проверки, а
// ловушки (кто упомянут в третьем лице, чьё имя звучит в обращении самого участника) не проходят.
//
// С моделью: запустить llama-server с той же моделью, что на телефоне, и указать его адрес:
//   llama-server -m qwen3.5-4b-q3_k_m.gguf --port 8099 -c 4096 -ngl 0 --jinja
//   cd nixyon-app/src-tauri/gen/android
//   NIXYON_LLAMA_SERVER=http://127.0.0.1:8099 ./gradlew :nixyon-android:testDebugUnitTest --tests '*SpeakerNamesEval*'
// Запросы и ответы модели идут в stderr, таблица итогов в stdout теста (отчёт Gradle,
// build/reports/tests). Без переменной замер пропускается.
class SpeakerNamesEval {
    private class Meeting(
        val name: String,
        val turns: List<NamingTurn>,
        val speakers: List<NamingSpeaker>,
        // Кого как зовут, если это ясно из разговора; остальные должны остаться без имени.
        val expected: Map<Int, String>,
        // Неверные ответы, которые проверки обязаны отсеять.
        val traps: String = "",
    )

    private fun meeting(name: String, expected: Map<Int, String>, traps: String, vararg lines: Pair<Int, String>): Meeting {
        val turns = lines.mapIndexed { i, (speaker, text) -> NamingTurn(i + 1, speaker, text) }
        val speakers = turns.map { it.speaker }.distinct().sorted().map { NamingSpeaker(it, null) }
        return Meeting(name, turns, speakers, expected, traps)
    }

    private val meetings = listOf(
        // scripts/data/llm-meetings/meeting-1.txt без имён: Анна, Борис и Спикер 3.
        meeting(
            "planning", mapOf(2 to "Борис"), "Спикер 1: Борис\nСпикер 3: Ольга",
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
            3 to "Кто-нибудь пробовал новую кофемашину на третьем этаже? Говорят, капучино там отличный.",
            2 to "Пробовал, ничего особенного. Ладно, давайте дальше.",
            1 to "Последнее: релиз. Раз импорт сломан, предлагаю перенести выпуск с пятого на двенадцатое ноября. Возражения есть?",
            2 to "Нет, так спокойнее.",
            3 to "Согласен.",
            1 to "Тогда решили: релиз двенадцатого ноября. Спасибо всем, созвонимся в пятницу.",
        ),
        // scripts/data/llm-meetings/meeting-2.txt без имён: Мария и Игорь.
        meeting(
            "supplier", mapOf(2 to "Игорь"), "Спикер 1: Игорь",
            1 to "Игорь, добрый день. Я по поставке датчиков, мы договаривались созвониться.",
            2 to "Да, здравствуйте. Слушаю.",
            1 to "Нам нужно двести сорок штук вместо двухсот, заказ вырос. Сможете отгрузить до двадцатого ноября?",
            2 to "Двести сорок сможем, на складе есть. По цене: была тысяча восемьсот пятьдесят рублей за штуку, но при таком объёме я могу предложить тысячу семьсот девяносто.",
            1 to "Хорошо. А доставка?",
            2 to "Доставка за наш счёт, если заказ от двухсот штук.",
            1 to "Отлично. Тогда я отправлю вам договор с новыми объёмами до понедельника.",
            2 to "Договорились. А я согласую скидку с нашей бухгалтерией, они обычно отвечают за день-два, и пришлю подтверждение письмом.",
            1 to "Спасибо. Тогда созвонимся в четверг в пятнадцать часов, чтобы всё подтвердить.",
            2 to "Да, в четверг в три. Всего доброго.",
        ),
        // scripts/data/llm-meetings/meeting-3.txt: имён нет, модель не нужна.
        meeting(
            "office", emptyMap(), "",
            1 to "Так, ну все подключились? Начинаем. Э, тема одна: переезд в новый офис.",
            2 to "Ну, по срокам. Арендодатель сказал, что ключи отдадут первого декабря.",
            1 to "Значит, переезжаем в выходные шестого и седьмого декабря. Кто за что?",
            3 to "Я могу заняться грузчиками. Найду компанию и закажу машину на шестое.",
            1 to "Отлично, спасибо. Сеть и интернет в новом офисе?",
            4 to "Это на мне. Я уже говорил с провайдером, надо подписать заявку. Подпишу до пятнадцатого ноября, они подключают за две недели.",
            2 to "Ну и мебель. Старые столы, э, половина сломана. Может, купить новые?",
            1 to "Было бы хорошо когда-нибудь обновить всю мебель, но сейчас денег нет. Столы пока везём старые, решили.",
            3 to "А коробки для вещей?",
            2 to "Коробки закажу я, штук пятьдесят хватит.",
            1 to "Хорошо. И ещё: каждый до конца ноября разбирает свой стол и выбрасывает лишнее. Это всех касается.",
        ),
        // Знакомство по кругу, обращения, упомянутые, но отсутствующие люди, компания и город.
        meeting(
            "kickoff", mapOf(1 to "Руслан", 2 to "Светлана", 3 to "Дмитрий"), "Спикер 1: Нина Сергеевна\nСпикер 3: Светлана",
            1 to "Коллеги, добрый день. Давайте коротко познакомимся, у нас новые люди. Меня зовут Руслан, я руковожу проектом.",
            2 to "Я Светлана, аналитик, пришла на прошлой неделе из Яндекса.",
            3 to "Дмитрий, бэкенд. Работаю удалённо из Казани.",
            1 to "Отлично. Светлана, расскажешь, что с требованиями от заказчика?",
            2 to "Да. Требования пришли в пятницу, там двенадцать пунктов, три из них спорные. Я бы обсудила их с Ниной Сергеевной, она вела этого заказчика раньше.",
            1 to "Нина Сергеевна сейчас на больничном, вернётся в понедельник. Давай подождём её.",
            3 to "А по API есть ясность? Мне нужно знать формат до среды.",
            1 to "Дим, это к Светлане. Светлана, успеешь до среды?",
            2 to "Успею, пришлю во вторник.",
            1 to "Хорошо. Спасибо, Дмитрий, спасибо, Светлана. На сегодня всё.",
        ),
        // Звонок: представление через «это», имя с отчеством, упомянутый третий человек.
        meeting(
            "call", mapOf(1 to "Наталья", 2 to "Сергей Петрович"), "Спикер 1: Иван Андреевич\nСпикер 2: Наталья",
            1 to "Алло, Сергей Петрович? Добрый день, это Наталья из «Альфа-Логистики».",
            2 to "Да, Наталья, здравствуйте. Слушаю вас.",
            1 to "Звоню по поводу задержки фуры из Новосибирска. Водитель сообщил, что машина будет только в четверг.",
            2 to "Неприятно. У нас склад в Подольске ждёт её в среду. Можно как-то ускорить?",
            1 to "Могу предложить перегрузить часть товара на нашу машину в Екатеринбурге, тогда половина приедет в среду.",
            2 to "Давайте так. Я предупрежу Ивана Андреевича, чтобы склад принял две поставки.",
            1 to "Хорошо, Сергей Петрович, тогда я пришлю вам новый график сегодня до шести.",
            2 to "Договорились, спасибо.",
        ),
        // Планёрка: слово передают по имени, имя звучит в разных формах, один участник безымянный.
        meeting(
            "standup", mapOf(2 to "Марина", 4 to "Кирилл"), "Спикер 1: Марина\nСпикер 3: Тимур",
            1 to "Начинаем планёрку. Первым пусть отчитается отдел продаж. Марина, тебе слово.",
            2 to "Спасибо. За октябрь закрыли сорок две сделки, это на десять процентов больше сентября. Основной рост дала новая линейка.",
            1 to "Отлично. Вопросы к Марине есть?",
            3 to "Есть один. Марин, а сколько из них повторные клиенты?",
            2 to "Примерно треть. Точную цифру пришлю после планёрки.",
            1 to "Хорошо. Теперь разработка. Кирилл?",
            4 to "У нас релиз переехал на следующую неделю: нашли ошибку в оплате. Исправим к среде.",
            3 to "Это та ошибка, о которой писал Тимур?",
            4 to "Да, она. Тимур уже проверил исправление на тестовом стенде.",
            1 to "Понятно. Спасибо, Кирилл. На этом всё.",
        ),
    )

    // Без модели: верный ответ проходит проверки целиком, ловушки не проходят.
    @Test
    fun theRightAnswerPassesTheChecksAndTrapsDoNot() {
        for (m in meetings) {
            val prompt = SpeakerNames.prompt(m.speakers, m.turns)
            if (m.expected.isEmpty()) {
                assertEquals(m.name, null, prompt)
                continue
            }
            prompt!!
            val right = prompt.asked.joinToString("\n") { "Спикер $it: ${m.expected[it] ?: "неизвестно"}" }
            val found = SpeakerNames.parse(right, prompt, m.speakers, m.turns).associate { it.speaker to it.name }
            assertEquals(m.name, m.expected, found)
            assertEquals(m.name, emptyMap<Int, String>(), SpeakerNames.parse(m.traps, prompt, m.speakers, m.turns).associate { it.speaker to it.name })
        }
    }

    @Test
    fun withTheModel() {
        val server = System.getenv("NIXYON_LLAMA_SERVER")
        assumeTrue("NIXYON_LLAMA_SERVER не задан", !server.isNullOrBlank())
        var right = 0
        var wrong = 0
        var missed = 0
        println("| Встреча | Токенов запроса | Ответ, с | Верно | Неверно | Пропущено |")
        println("|---|---|---|---|---|---|")
        for (m in meetings) {
            var tokens = 0
            var seconds = 0.0
            val found = SpeakerNamer(SpeechLanguage.RU) { system, user, maxTokens ->
                val reply = chat(server!!, system, user, maxTokens)
                tokens = Regex(""""prompt_tokens"\s*:\s*(\d+)""").find(reply)?.groupValues?.get(1)?.toInt() ?: 0
                val ms = Regex(""""(prompt_ms|predicted_ms)"\s*:\s*([\d.]+)""").findAll(reply).sumOf { it.groupValues[2].toDouble() }
                seconds = ms / 1000
                val answer = content(reply)
                System.err.println("--- ${m.name}\n$user\n>>>\n$answer")
                answer
            }.suggest(m.speakers, m.turns).associate { it.speaker to it.name }
            // Неверно: имя не то или имя тому, чьё имя из разговора не ясно. Пропущено: имя
            // ясно, а подсказки нет.
            val r = found.count { (id, name) -> m.expected[id] == name }
            val w = found.size - r
            val miss = m.expected.keys.count { it !in found }
            right += r
            wrong += w
            missed += miss
            println("| ${m.name} | $tokens | ${"%.1f".format(seconds)} | $r | $w | $miss |")
        }
        println("Итого: верно $right, неверно $wrong, пропущено $missed")
    }

    // Запрос как у приложения: системная инструкция и текст, жадный выбор, без рассуждений.
    private fun chat(server: String, system: String, user: String, maxTokens: Int): String {
        val body = """{"messages":[{"role":"system","content":${json(system)}},{"role":"user","content":${json(user)}}],""" +
            """"temperature":0,"max_tokens":$maxTokens,"cache_prompt":false,"chat_template_kwargs":{"enable_thinking":false}}"""
        val connection = URI(server.trimEnd('/') + "/v1/chat/completions").toURL().openConnection() as HttpURLConnection
        connection.requestMethod = "POST"
        connection.doOutput = true
        connection.setRequestProperty("Content-Type", "application/json")
        connection.outputStream.use { it.write(body.toByteArray(Charsets.UTF_8)) }
        return connection.inputStream.use { String(it.readBytes(), Charsets.UTF_8) }
    }

    private fun json(text: String): String = buildString {
        append('"')
        for (c in text) {
            when {
                c == '"' -> append("\\\"")
                c == '\\' -> append("\\\\")
                c == '\n' -> append("\\n")
                c < ' ' -> append("\\u%04x".format(c.code))
                else -> append(c)
            }
        }
        append('"')
    }

    // Текст ответа из JSON llama-server без библиотеки JSON: в тестах JVM org.json нет.
    private fun content(reply: String): String {
        val start = reply.indexOf("\"content\"").let { reply.indexOf('"', reply.indexOf(':', it) + 1) } + 1
        val out = StringBuilder()
        var i = start
        while (i < reply.length && reply[i] != '"') {
            if (reply[i] == '\\') {
                when (val e = reply[++i]) {
                    'n' -> out.append('\n')
                    't' -> out.append('\t')
                    'u' -> {
                        out.append(reply.substring(i + 1, i + 5).toInt(16).toChar())
                        i += 4
                    }
                    else -> out.append(e)
                }
            } else {
                out.append(reply[i])
            }
            i++
        }
        return out.toString()
    }
}
