package dev.demo.nixyon.speech

import org.junit.Assert.assertEquals
import org.junit.Test

class PlaybackNoticeTest {
    @Test
    fun pausingARecordThatNeverPlayedShowsNoNotification() {
        assertEquals(PlaybackNotice.NONE, PlaybackNotice.choose(active = false, playing = false))
    }

    @Test
    fun serviceStartsWhenSoundStartsAndThenOnlyUpdates() {
        assertEquals(PlaybackNotice.START, PlaybackNotice.choose(active = false, playing = true))
        assertEquals(PlaybackNotice.UPDATE, PlaybackNotice.choose(active = true, playing = false))
        assertEquals(PlaybackNotice.UPDATE, PlaybackNotice.choose(active = true, playing = true))
    }
}
