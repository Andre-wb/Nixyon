package dev.demo.nixyon.speech

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

class DownloadJournalTest {
    @get:Rule
    val dir = TemporaryFolder()

    @Test
    fun aWrittenJournalIsReadBackAndClearedJournalIsEmpty() {
        val journal = DownloadJournal(File(dir.root, "j.json"))
        journal.write(DownloadJournal.Entry(setOf(ModelPack.SPEECH, ModelPack.LLM), true, mapOf("models/vad.onnx.part" to setOf(0, 2))))
        val read = journal.read()!!
        assertEquals(setOf(ModelPack.SPEECH, ModelPack.LLM), read.packs)
        assertEquals(true, read.allowMetered)
        assertEquals(mapOf("models/vad.onnx.part" to setOf(0, 2)), read.chunks)
        journal.clear()
        assertNull(journal.read())
    }

    @Test
    fun aBrokenJournalCountsAsNoDownload() {
        val file = File(dir.root, "j.json")
        file.writeText("{not json")
        assertNull(DownloadJournal(file).read())
    }
}
