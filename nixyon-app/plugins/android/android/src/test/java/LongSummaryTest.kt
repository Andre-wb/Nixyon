package dev.demo.nixyon.speech

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test

// Протокол длинной встречи по частям (LongSummary, MeetingParts). Модель подменена: запрос
// длиннее limit символов даёт llm_too_long, как проверка длины в meeting_llm.cpp.
class LongSummaryTest {
    private val header = "Встреча\nДлительность: 01:00:00\nУчастников: 2"

    private fun turn(i: Int, words: Int = 30) =
        "Спикер ${i % 2 + 1}, 00:${(i / 60).toString().padStart(2, '0')}:${(i % 60).toString().padStart(2, '0')}\n" +
            (1..words).joinToString(" ") { "слово$i-$it" } + "."

    private fun meeting(turns: List<String>) = MeetingText(header, turns).text()

    private class FakeModel(val limit: Int) {
        val requests = mutableListOf<String>()
        val progress = mutableListOf<Triple<Int, Int, Int>>()

        fun generate(user: String, p: LlmProgress): String {
            requests += user
            if (user.length > limit) throw LlmException(LlmException.TOO_LONG, "длинно")
            p.onProgress(LlmProgress.READING, 0, 10)
            p.onProgress(LlmProgress.READING, 10, 10)
            p.onProgress(LlmProgress.WRITING, 5, 20)
            p.onProgress(LlmProgress.WRITING, 20, 20)
            return if (user.contains("Протокол части")) "<think>x</think>\nИтог ${requests.size}" else "Часть ${requests.size}\n"
        }
    }

    private fun run(model: FakeModel, user: String): Pair<String, LongSummary> {
        val long = LongSummary(model::generate)
        val text = long.run(user) { phase, done, total -> model.progress += Triple(phase, done, total) }
        return text to long
    }

    @Test
    fun aMeetingThatFitsIsOneRequestWithItsOwnProgress() {
        val model = FakeModel(100_000)
        val user = meeting((0 until 5).map { turn(it) })
        val (text, long) = run(model, user)
        assertEquals(listOf(user), model.requests)
        assertEquals("Часть 1\n", text)
        assertEquals(0, long.parts)
        assertEquals(Triple(LlmProgress.WRITING, 20, 20), model.progress.last())
    }

    @Test
    fun aLongMeetingIsSplitByTurnsAndThePartsAreCombined() {
        val turns = (0 until 40).map { turn(it) }
        val model = FakeModel(3_000)
        val (text, long) = run(model, meeting(turns))
        val parts = model.requests.drop(1).filter { it.length <= 3_000 && !it.contains("Протокол части") }
        assertEquals(long.parts, parts.size)
        assertTrue(long.parts >= 4)
        // Каждая часть с шапкой, все реплики по разу и по порядку.
        val partTurns = parts.map { MeetingText.parse(it) }
        partTurns.forEach { assertEquals(header, it.header) }
        assertEquals(turns, partTurns.flatMap { it.turns })
        val combine = model.requests.last()
        assertTrue(combine.startsWith(header))
        assertTrue(combine.contains("Протокол части ${long.parts} из ${long.parts}:"))
        assertEquals("Итог ${model.requests.size}", text)
        assertEquals(1, long.combines)
    }

    @Test
    fun progressGrowsToTheEndOfThePartsAndTheCombinedAnswerIsWritten() {
        val model = FakeModel(3_000)
        run(model, meeting((0 until 40).map { turn(it) }))
        val reading = model.progress.filter { it.first == LlmProgress.READING }.map { it.second }
        assertEquals(reading.sorted(), reading)
        assertTrue(reading.all { it in 0..LongSummary.SCALE })
        assertEquals(LongSummary.SCALE, reading.last())
        assertTrue(model.progress.all { it.first == LlmProgress.WRITING || it.third == LongSummary.SCALE })
        assertEquals(Triple(LlmProgress.WRITING, 20, 20), model.progress.last())
    }

    @Test
    fun oneLongTurnIsCutAtSpacesAndKeepsItsSpeaker() {
        val long = turn(7, words = 600)
        val (first, second) = MeetingParts.halve(listOf(long))!!
        val head = long.substringBefore('\n')
        assertEquals(head, first.single().substringBefore('\n'))
        assertEquals(head, second.single().substringBefore('\n'))
        assertEquals(long.substringAfter('\n'), first.single().substringAfter('\n') + " " + second.single().substringAfter('\n'))
        assertNull(MeetingParts.halve(listOf(turn(1, words = 5))))
        assertNull(MeetingParts.halve(emptyList()))

        val model = FakeModel(3_000)
        val (text, summary) = run(model, meeting(listOf(long)))
        assertTrue(summary.parts >= 4)
        assertTrue(text.startsWith("Итог"))
    }

    @Test
    fun whatCannotBeSplitStaysTooLong() {
        val model = FakeModel(50)
        try {
            run(model, meeting(listOf(turn(1, words = 5))))
            fail("протокол составлен")
        } catch (e: LlmException) {
            assertEquals(LlmException.TOO_LONG, e.code)
        }
    }

    @Test
    fun tooManyPartProtocolsAreCombinedByHalves() {
        val turns = (0 until 60).map { turn(it) }
        // Сведение всех частей сразу не помещается, сведение половин помещается.
        val model = object {
            val requests = mutableListOf<String>()
            fun generate(user: String, p: LlmProgress): String {
                requests += user
                val limit = if (user.contains("Протокол части")) 400 else 3_000
                if (user.length > limit) throw LlmException(LlmException.TOO_LONG, "длинно")
                return if (user.contains("Протокол части")) "Свод" else "Часть"
            }
        }
        val long = LongSummary(model::generate)
        assertEquals("Свод", long.run(meeting(turns)) { _, _, _ -> })
        assertTrue(long.combines >= 3)
    }

    @Test
    fun otherErrorsStopAtOnce() {
        val requests = mutableListOf<String>()
        val long = LongSummary { user, _ ->
            requests += user
            if (requests.size == 1) throw LlmException(LlmException.TOO_LONG, "длинно")
            throw LlmException(LlmException.CANCELLED, "отменено")
        }
        try {
            long.run(meeting((0 until 40).map { turn(it) })) { _, _, _ -> }
            fail("протокол составлен")
        } catch (e: LlmException) {
            assertEquals(LlmException.CANCELLED, e.code)
        }
        assertEquals(2, requests.size)
    }

    @Test
    fun theMeetingTextRoundTrips() {
        val text = meeting((0 until 3).map { turn(it) })
        val parsed = MeetingText.parse(text)
        assertEquals(header, parsed.header)
        assertEquals(3, parsed.turns.size)
        assertEquals(text, parsed.text())
        assertEquals("Итог", MeetingParts.cleanAnswer("<think>\nдумаю\n</think>\n Итог \n"))
    }
}
