package dev.demo.nixyon.speech

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.nio.file.Files

class RecordingTargetsTest {
    private val files = Files.createTempDirectory("nixyon-targets").toFile()

    private fun record(target: RecordingTarget, prefix: String) =
        target.keep(target.newPart(prefix).also { it.writeText(prefix) })

    @Test
    fun voiceSampleRecordedAfterAMeetingLeavesTheMeetingRecordingInPlace() {
        val meeting = record(RecordingsDir(files), "rec")
        val sample = record(SamplesDir(files), "smp")
        assertTrue(meeting.exists())
        assertTrue(sample.exists())
        assertEquals(listOf(meeting), RecordingsDir(files).dir.listFiles()!!.toList())
    }

    @Test
    fun newMeetingReplacesThePreviousOneAndNewSampleReplacesThePreviousSample() {
        val first = record(RecordingsDir(files), "rec")
        Thread.sleep(2)
        val second = record(RecordingsDir(files), "rec")
        val oldSample = record(SamplesDir(files), "smp")
        Thread.sleep(2)
        val newSample = record(SamplesDir(files), "smp")
        assertFalse(first.exists())
        assertTrue(second.exists())
        assertFalse(oldSample.exists())
        assertTrue(newSample.exists())
    }
}
