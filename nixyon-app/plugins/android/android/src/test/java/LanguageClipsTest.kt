package dev.demo.nixyon.speech

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

// Те же примеры, что в nixyon-core (language_clips.rs, language_vote.rs): правило у ноутбука и
// телефона одно.
class LanguageClipsTest {
    @Test
    fun takesTheLongestPieceOfEachFifth() {
        val bounds = listOf(0L to 5L, 6L to 18L, 25L to 30L, 41L to 59L, 90L to 99L)
        assertEquals(listOf(6L to 18L, 25L to 30L, 41L to 59L, 90L to 99L), LanguageClips.pick(bounds, 100))
    }

    @Test
    fun pieceBelongsToThePartOfItsMiddle() {
        assertEquals(listOf(15L to 35L), LanguageClips.pick(listOf(15L to 35L), 100))
        assertEquals(listOf(15L to 35L), LanguageClips.pick(listOf(15L to 35L, 21L to 39L), 100))
    }

    @Test
    fun longPieceIsCutToWhisperWindow() {
        val max = LanguageClips.MAX_CLIP_SAMPLES
        assertEquals(listOf(100L to 100L + max), LanguageClips.pick(listOf(100L to 100L + 2 * max), 10 * max))
    }

    @Test
    fun noSpeechGivesNoClips() {
        assertTrue(LanguageClips.pick(emptyList(), 100).isEmpty())
        assertTrue(LanguageClips.pick(listOf(3L to 3L), 100).isEmpty())
        assertTrue(LanguageClips.pick(listOf(0L to 5L), 0).isEmpty())
    }

    @Test
    fun piecePastTheEndGoesToTheLastPart() {
        assertEquals(listOf(98L to 120L), LanguageClips.pick(listOf(98L to 120L), 100))
    }

    @Test
    fun englishNeedsMoreThanHalfAndUnclearIsRussian() {
        val en = SpeechLanguage.EN
        val ru = SpeechLanguage.RU
        assertEquals(en, LanguageClips.vote(listOf(en, en, en, ru, ru)))
        assertEquals(ru, LanguageClips.vote(listOf(en, en, null, ru, ru)))
        assertEquals(ru, LanguageClips.vote(listOf(en, ru)))
        assertEquals(en, LanguageClips.vote(listOf(en)))
        assertEquals(ru, LanguageClips.vote(emptyList()))
        assertEquals(ru, LanguageClips.vote(listOf(null, null, null)))
    }

    @Test
    fun languageCodesMatchTheCore() {
        assertEquals(SpeechLanguage.EN, SpeechLanguage.fromCode("en"))
        assertEquals(SpeechLanguage.RU, SpeechLanguage.fromCode("ru"))
        assertNull(SpeechLanguage.fromCode("de"))
        assertNull(SpeechLanguage.fromCode(null))
    }

    @Test
    fun onlyRussianUsesHotwords() {
        assertTrue(SpeechLanguage.RU.hotwords)
        assertFalse(SpeechLanguage.EN.hotwords)
    }
}
