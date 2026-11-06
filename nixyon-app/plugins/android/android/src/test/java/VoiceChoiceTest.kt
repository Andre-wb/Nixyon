package dev.demo.nixyon.speech

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class VoiceChoiceTest {
    @Test
    fun voiceModelIsARequiredFileOfTheModelSet() {
        assertFalse(VoiceChoice.ROLE.optional)
    }

    @Test
    fun voiceModelIsNotTheOneTheDiarizerNeeds() {
        assertTrue(VoiceChoice.ROLE != ModelRole.EMBEDDING)
    }

    @Test
    fun sameSpeakerThresholdIsACosine() {
        assertTrue(VoiceChoice.SAME_SPEAKER_COSINE in -1.0..1.0)
    }
}
